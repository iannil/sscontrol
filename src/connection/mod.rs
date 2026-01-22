//! 统一连接管理模块
//!
//! 提供局域网 + 跨网络的统一连接管理：
//! - 被控端：生成连接码，等待连接
//! - 控制端：解析连接码，建立连接

use crate::discovery::{ConnectionCode, DiscoveredPeer, MdnsDiscovery, MdnsService};
use crate::signaling::{CloudflareSignaling, IceCandidate, SignalingError};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// 连接模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    /// 被控端模式
    Host,
    /// 控制端模式
    Client,
}

/// 连接状态
#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    /// 等待连接
    Waiting,
    /// 正在连接
    Connecting,
    /// 已连接
    Connected,
    /// 连接失败
    Failed(String),
    /// 已断开
    Disconnected,
}

/// 连接配置
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// 信令服务 URL
    pub signaling_url: Option<String>,

    /// 连接码有效期 (秒)
    pub code_ttl: u64,

    /// 连接超时 (秒)
    pub connect_timeout: u64,

    /// 是否启用 mDNS 发现
    pub mdns_enabled: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            signaling_url: None,
            code_ttl: 300,
            connect_timeout: 60,
            mdns_enabled: true,
        }
    }
}

/// 被控端连接信息
#[derive(Debug, Clone)]
pub struct HostConnectionInfo {
    /// 连接码
    pub code: ConnectionCode,

    /// 格式化的连接码字符串
    pub code_string: String,

    /// PIN 码
    pub pin: String,

    /// 过期时间戳
    pub expires_at: u64,
}

/// 连接管理器
pub struct ConnectionManager {
    config: ConnectionConfig,
    mode: ConnectionMode,
    status: Arc<Mutex<ConnectionStatus>>,
    signaling: CloudflareSignaling,
    mdns_service: Option<MdnsService>,
    mdns_discovery: Option<MdnsDiscovery>,
    device_id: String,
}

impl ConnectionManager {
    /// 创建被控端连接管理器
    pub fn new_host(device_id: &str, config: ConnectionConfig) -> Result<Self> {
        let signaling = CloudflareSignaling::new(config.signaling_url.as_deref());

        // 创建 mDNS 服务
        let mdns_service = if config.mdns_enabled {
            Some(MdnsService::new(device_id, 0)?) // 端口后续设置
        } else {
            None
        };

        Ok(Self {
            config,
            mode: ConnectionMode::Host,
            status: Arc::new(Mutex::new(ConnectionStatus::Waiting)),
            signaling,
            mdns_service,
            mdns_discovery: None,
            device_id: device_id.to_string(),
        })
    }

    /// 创建控制端连接管理器
    pub fn new_client(device_id: &str, config: ConnectionConfig) -> Result<Self> {
        let signaling = CloudflareSignaling::new(config.signaling_url.as_deref());

        // 创建 mDNS 发现服务
        let mdns_discovery = if config.mdns_enabled {
            Some(MdnsDiscovery::new()?)
        } else {
            None
        };

        Ok(Self {
            config,
            mode: ConnectionMode::Client,
            status: Arc::new(Mutex::new(ConnectionStatus::Waiting)),
            signaling,
            mdns_service: None,
            mdns_discovery,
            device_id: device_id.to_string(),
        })
    }

    /// 被控端：生成连接码并开始等待连接
    pub async fn host_start(&mut self, sdp_offer: &str, ice_candidates: Vec<IceCandidate>) -> Result<HostConnectionInfo> {
        if self.mode != ConnectionMode::Host {
            return Err(anyhow!("Not in host mode"));
        }

        // 生成连接码
        let code = ConnectionCode::generate_with_ttl(self.config.code_ttl);
        let code_string = code.encode();
        let pin = format!("{:04}", code.pin);
        let session_id = code.session_id_hex();

        info!("Generated connection code: {}", code_string);
        info!("PIN: {}", pin);

        // 注册到 mDNS
        if let Some(ref mut mdns) = self.mdns_service {
            if let Err(e) = mdns.register(Some(&session_id), None) {
                warn!("Failed to register mDNS service: {}", e);
            }
        }

        // 注册到公共信令服务
        let pin_hash = self.hash_pin(&pin);
        let expires_at = self
            .signaling
            .create_session(
                &session_id,
                sdp_offer,
                ice_candidates,
                None, // public_key
                Some(&pin_hash),
                self.config.code_ttl,
            )
            .await
            .map_err(|e| anyhow!("Failed to create session: {}", e))?;

        *self.status.lock().await = ConnectionStatus::Waiting;

        Ok(HostConnectionInfo {
            code: code.clone(),
            code_string,
            pin,
            expires_at,
        })
    }

    /// 被控端：等待控制端连接
    pub async fn host_wait_for_connection(&self, session_id: &str) -> Result<(String, Vec<IceCandidate>)> {
        let timeout = Duration::from_secs(self.config.code_ttl);
        let interval = Duration::from_secs(2);

        *self.status.lock().await = ConnectionStatus::Waiting;

        let result = self
            .signaling
            .poll_for_answer(session_id, timeout, interval)
            .await
            .map_err(|e| anyhow!("Failed to receive answer: {}", e))?;

        *self.status.lock().await = ConnectionStatus::Connecting;

        Ok(result)
    }

    /// 控制端：通过连接码连接
    pub async fn client_connect(
        &mut self,
        code_string: &str,
        pin: &str,
    ) -> Result<(String, Vec<IceCandidate>)> {
        if self.mode != ConnectionMode::Client {
            return Err(anyhow!("Not in client mode"));
        }

        // 解析连接码
        let code = ConnectionCode::decode(code_string)
            .map_err(|e| anyhow!("Invalid connection code: {}", e))?;

        // 验证 PIN
        let pin_value: u16 = pin.parse().map_err(|_| anyhow!("Invalid PIN format"))?;
        if !code.verify_pin(pin_value) {
            return Err(anyhow!("Incorrect PIN"));
        }

        // 检查是否过期
        if !code.is_valid() {
            return Err(anyhow!("Connection code has expired"));
        }

        let session_id = code.session_id_hex();
        info!("Connecting to session: {}", session_id);

        *self.status.lock().await = ConnectionStatus::Connecting;

        // 并行尝试局域网和公网连接
        let lan_result = self.try_lan_connect(&session_id);
        let wan_result = self.try_wan_connect(&session_id);

        // 使用先成功的结果
        tokio::select! {
            result = lan_result => {
                if let Ok(info) = result {
                    info!("Connected via LAN");
                    return Ok((info.0, info.1));
                }
            }
            result = wan_result => {
                if let Ok(info) = result {
                    info!("Connected via WAN");
                    return Ok((info.0, info.1));
                }
            }
        }

        // 如果并行失败，顺序尝试
        match self.try_wan_connect(&session_id).await {
            Ok(info) => {
                info!("Connected via WAN (fallback)");
                Ok(info)
            }
            Err(e) => {
                *self.status.lock().await = ConnectionStatus::Failed(e.to_string());
                Err(e)
            }
        }
    }

    /// 尝试局域网连接
    async fn try_lan_connect(&self, session_id: &str) -> Result<(String, Vec<IceCandidate>)> {
        if let Some(ref discovery) = self.mdns_discovery {
            // 查找带有该 session_id 的设备
            if let Some(peer) = discovery.find_by_session_id(session_id) {
                debug!("Found peer via mDNS: {:?}", peer);
                // TODO: 直接通过 LAN 建立 WebRTC 连接
                // 目前先返回错误，触发 WAN 连接
            }
        }

        Err(anyhow!("LAN connection not available"))
    }

    /// 尝试公网连接
    async fn try_wan_connect(&self, session_id: &str) -> Result<(String, Vec<IceCandidate>)> {
        // 从信令服务获取 offer
        let session_info = self
            .signaling
            .get_session(session_id)
            .await
            .map_err(|e| anyhow!("Failed to get session: {}", e))?;

        Ok((session_info.offer, session_info.candidates))
    }

    /// 控制端：发送 Answer
    pub async fn client_send_answer(
        &self,
        session_id: &str,
        answer: &str,
        candidates: Vec<IceCandidate>,
    ) -> Result<()> {
        self.signaling
            .post_answer(session_id, answer, candidates)
            .await
            .map_err(|e| anyhow!("Failed to send answer: {}", e))?;

        *self.status.lock().await = ConnectionStatus::Connected;
        Ok(())
    }

    /// 添加 ICE 候选
    pub async fn add_ice_candidate(&self, session_id: &str, candidate: IceCandidate) -> Result<()> {
        let role = match self.mode {
            ConnectionMode::Host => "host",
            ConnectionMode::Client => "client",
        };

        self.signaling
            .post_ice_candidate(session_id, role, candidate)
            .await
            .map_err(|e| anyhow!("Failed to add ICE candidate: {}", e))
    }

    /// 获取当前状态
    pub async fn status(&self) -> ConnectionStatus {
        self.status.lock().await.clone()
    }

    /// 设置连接成功
    pub async fn set_connected(&self) {
        *self.status.lock().await = ConnectionStatus::Connected;
    }

    /// 设置连接断开
    pub async fn set_disconnected(&self) {
        *self.status.lock().await = ConnectionStatus::Disconnected;
    }

    /// 计算 PIN 哈希
    fn hash_pin(&self, pin: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(pin.as_bytes());
        hasher.update(self.device_id.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// 启动 mDNS 发现 (控制端)
    pub fn start_discovery(&mut self) -> Result<tokio::sync::mpsc::Receiver<DiscoveredPeer>> {
        if let Some(ref mut discovery) = self.mdns_discovery {
            discovery.start()
        } else {
            Err(anyhow!("mDNS discovery not available"))
        }
    }

    /// 获取已发现的设备 (控制端)
    pub fn discovered_peers(&self) -> Vec<DiscoveredPeer> {
        if let Some(ref discovery) = self.mdns_discovery {
            discovery.get_peers()
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_config_default() {
        let config = ConnectionConfig::default();
        assert_eq!(config.code_ttl, 300);
        assert_eq!(config.connect_timeout, 60);
        assert!(config.mdns_enabled);
    }
}
