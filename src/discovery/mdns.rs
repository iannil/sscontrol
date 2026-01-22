//! mDNS 设备发现模块
//!
//! 提供局域网内设备的自动发现功能：
//! - 被控端：广播服务
//! - 控制端：发现服务

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info};

/// mDNS 服务类型
pub const SERVICE_TYPE: &str = "_sscontrol._tcp.local.";

/// 服务实例名称前缀
pub const SERVICE_NAME_PREFIX: &str = "sscontrol";

/// 发现的设备信息
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    /// 设备 ID
    pub device_id: String,

    /// IP 地址
    pub ip_address: IpAddr,

    /// 端口
    pub port: u16,

    /// 主机名
    pub hostname: String,

    /// 会话 ID (用于匹配连接码)
    pub session_id: Option<String>,

    /// 公钥指纹
    pub fingerprint: Option<String>,

    /// 最后发现时间
    pub last_seen: Instant,
}

/// mDNS 服务 (被控端使用)
pub struct MdnsService {
    daemon: ServiceDaemon,
    service_fullname: Option<String>,
    device_id: String,
    port: u16,
}

impl MdnsService {
    /// 创建新的 mDNS 服务
    pub fn new(device_id: &str, port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new()?;

        Ok(Self {
            daemon,
            service_fullname: None,
            device_id: device_id.to_string(),
            port,
        })
    }

    /// 注册服务 (开始广播)
    pub fn register(&mut self, session_id: Option<&str>, fingerprint: Option<&str>) -> Result<()> {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let instance_name = format!("{}-{}", SERVICE_NAME_PREFIX, &self.device_id[..8]);

        // 构建 TXT 记录
        let mut properties = vec![
            ("device_id".to_string(), self.device_id.clone()),
            ("hostname".to_string(), hostname),
            ("version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
        ];

        if let Some(sid) = session_id {
            properties.push(("session_id".to_string(), sid.to_string()));
        }

        if let Some(fp) = fingerprint {
            properties.push(("fingerprint".to_string(), fp.to_string()));
        }

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &format!("{}.local.", instance_name),
            "",
            self.port,
            properties.as_slice(),
        )?;

        let fullname = service_info.get_fullname().to_string();
        self.daemon.register(service_info)?;
        self.service_fullname = Some(fullname.clone());

        info!("mDNS service registered: {} on port {}", fullname, self.port);
        Ok(())
    }

    /// 更新 session_id (生成新连接码时调用)
    pub fn update_session_id(&mut self, session_id: &str) -> Result<()> {
        // 先注销再重新注册
        self.unregister()?;
        self.register(Some(session_id), None)
    }

    /// 注销服务
    pub fn unregister(&mut self) -> Result<()> {
        if let Some(fullname) = self.service_fullname.take() {
            self.daemon.unregister(&fullname)?;
            info!("mDNS service unregistered: {}", fullname);
        }
        Ok(())
    }
}

impl Drop for MdnsService {
    fn drop(&mut self) {
        let _ = self.unregister();
        let _ = self.daemon.shutdown();
    }
}

/// mDNS 发现服务 (控制端使用)
pub struct MdnsDiscovery {
    daemon: ServiceDaemon,
    discovered: Arc<Mutex<HashMap<String, DiscoveredPeer>>>,
    is_browsing: bool,
}

impl MdnsDiscovery {
    /// 创建新的发现服务
    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new()?;

        Ok(Self {
            daemon,
            discovered: Arc::new(Mutex::new(HashMap::new())),
            is_browsing: false,
        })
    }

    /// 开始发现设备
    pub fn start(&mut self) -> Result<mpsc::Receiver<DiscoveredPeer>> {
        let receiver = self.daemon.browse(SERVICE_TYPE)?;
        let discovered = self.discovered.clone();

        let (tx, rx) = mpsc::channel(32);

        // 启动后台任务处理发现事件
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let peer = Self::parse_service_info(&info);
                        if let Some(peer) = peer {
                            debug!("Discovered peer: {:?}", peer);

                            // 更新已发现列表
                            {
                                let mut map = discovered.lock().unwrap();
                                map.insert(peer.device_id.clone(), peer.clone());
                            }

                            // 通知
                            let _ = tx.blocking_send(peer);
                        }
                    }
                    ServiceEvent::ServiceRemoved(_type_name, fullname) => {
                        debug!("Service removed: {}", fullname);
                        // 从列表中移除
                        let mut map = discovered.lock().unwrap();
                        map.retain(|_, p| !fullname.contains(&p.device_id));
                    }
                    _ => {}
                }
            }
        });

        self.is_browsing = true;
        Ok(rx)
    }

    /// 获取所有已发现的设备
    pub fn get_peers(&self) -> Vec<DiscoveredPeer> {
        let map = self.discovered.lock().unwrap();
        map.values()
            .filter(|p| p.last_seen.elapsed() < Duration::from_secs(60))
            .cloned()
            .collect()
    }

    /// 通过 session_id 查找设备
    pub fn find_by_session_id(&self, session_id: &str) -> Option<DiscoveredPeer> {
        let map = self.discovered.lock().unwrap();
        map.values()
            .find(|p| p.session_id.as_deref() == Some(session_id))
            .cloned()
    }

    /// 通过 device_id 查找设备
    pub fn find_by_device_id(&self, device_id: &str) -> Option<DiscoveredPeer> {
        let map = self.discovered.lock().unwrap();
        map.get(device_id).cloned()
    }

    /// 解析服务信息
    fn parse_service_info(info: &ServiceInfo) -> Option<DiscoveredPeer> {
        let addresses: Vec<_> = info.get_addresses().iter().collect();
        let ip_address = addresses.first().copied()?;

        let properties = info.get_properties();

        let device_id = properties.get("device_id")?.val_str().to_string();
        let hostname = properties
            .get("hostname")
            .map(|p| p.val_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let session_id = properties.get("session_id").map(|p| p.val_str().to_string());
        let fingerprint = properties.get("fingerprint").map(|p| p.val_str().to_string());

        Some(DiscoveredPeer {
            device_id,
            ip_address: *ip_address,
            port: info.get_port(),
            hostname,
            session_id,
            fingerprint,
            last_seen: Instant::now(),
        })
    }

    /// 停止发现
    pub fn stop(&mut self) -> Result<()> {
        if self.is_browsing {
            self.daemon.stop_browse(SERVICE_TYPE)?;
            self.is_browsing = false;
        }
        Ok(())
    }
}

impl Drop for MdnsDiscovery {
    fn drop(&mut self) {
        let _ = self.stop();
        let _ = self.daemon.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_type() {
        assert!(SERVICE_TYPE.ends_with(".local."));
        assert!(SERVICE_TYPE.starts_with("_"));
    }
}
