//! 网络传输模块
//!
//! 提供 WebSocket 客户端功能，支持自动重连和输入事件处理

use anyhow::{anyhow, Result};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

// 安全相关导入
#[cfg(feature = "security")]
use crate::security::{ApiKeyAuth, TokenManager};

/// 输入事件处理器回调
pub type InputEventHandler = Arc<Mutex<Option<Box<dyn Fn(crate::input::InputEvent) + Send + 'static>>>>;

/// WebSocket 发送器类型别名
type WsSender = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

/// 视频数据包 (用于网络传输)
#[derive(Debug, Clone)]
pub struct VideoPacket {
    pub device_id: String,
    pub timestamp: u64,
    pub sequence: u64,
    pub is_key_frame: bool,
    pub data: Vec<u8>,
}

impl VideoPacket {
    /// 序列化为 JSON + 二进制数据
    pub fn to_wire_format(&self) -> Vec<u8> {
        // 格式: [JSON header length (4 bytes)][JSON header][binary data]
        let header = serde_json::json!({
            "device_id": self.device_id,
            "timestamp": self.timestamp,
            "sequence": self.sequence,
            "is_key_frame": self.is_key_frame,
            "data_size": self.data.len(),
        });

        let header_str = header.to_string();
        let header_bytes = header_str.as_bytes();

        let mut result = Vec::with_capacity(4 + header_bytes.len() + self.data.len());

        // 写入 header 长度 (big endian)
        result.extend_from_slice(&(header_bytes.len() as u32).to_be_bytes());
        // 写入 header
        result.extend_from_slice(header_bytes);
        // 写入数据
        result.extend_from_slice(&self.data);

        result
    }
}

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

/// WebSocket 视频客户端配置
#[derive(Debug, Clone)]
pub struct VideoClientConfig {
    /// 自动重连
    pub auto_reconnect: bool,
    /// 重连间隔 (毫秒)
    pub reconnect_interval_ms: u64,
    /// 最大重连次数 (None = 无限)
    pub max_reconnect_attempts: Option<usize>,
    /// 连接超时 (秒)
    pub connect_timeout_secs: u64,
    /// API Key (可选，用于认证)
    pub api_key: Option<String>,
    /// 是否使用 TLS
    pub use_tls: bool,
}

impl Default for VideoClientConfig {
    fn default() -> Self {
        VideoClientConfig {
            auto_reconnect: true,
            reconnect_interval_ms: 2000,
            max_reconnect_attempts: None,
            connect_timeout_secs: 10,
            api_key: None,
            use_tls: false,
        }
    }
}

/// WebSocket 视频客户端
pub struct VideoClient {
    url: String,
    device_id: String,
    config: VideoClientConfig,
    sender: Arc<Mutex<Option<WsSender>>>,
    sequence: Arc<Mutex<u64>>,
    state: Arc<Mutex<ConnectionState>>,
    reconnect_count: Arc<Mutex<usize>>,
    should_stop: Arc<Mutex<bool>>,
    input_handler: InputEventHandler,
    /// Token 管理器 (用于认证)
    #[cfg(feature = "security")]
    token_manager: Option<Arc<TokenManager>>,
}

impl VideoClient {
    /// 创建新的视频客户端
    pub fn new(url: String, device_id: String) -> Self {
        Self::with_config(url, device_id, VideoClientConfig::default())
    }

    /// 使用配置创建客户端
    pub fn with_config(url: String, device_id: String, config: VideoClientConfig) -> Self {
        #[cfg(feature = "security")]
        let token_manager = if let Some(ref api_key) = config.api_key {
            Some(Arc::new(TokenManager::new(ApiKeyAuth::new(api_key.clone()))))
        } else {
            None
        };

        VideoClient {
            url,
            device_id,
            config,
            sender: Arc::new(Mutex::new(None)),
            sequence: Arc::new(Mutex::new(0)),
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            reconnect_count: Arc::new(Mutex::new(0)),
            should_stop: Arc::new(Mutex::new(false)),
            input_handler: Arc::new(Mutex::new(None)),
            #[cfg(feature = "security")]
            token_manager,
        }
    }

    /// 设置输入事件处理器
    pub fn set_input_handler<F>(&self, handler: F)
    where
        F: Fn(crate::input::InputEvent) + Send + 'static,
    {
        *self.input_handler.blocking_lock() = Some(Box::new(handler));
    }

    /// 发送认证消息
    ///
    /// 当配置了 API Key 时，在连接建立后自动调用此方法进行认证
    #[cfg(feature = "security")]
    pub async fn send_auth(&self) -> Result<()> {
        if let Some(ref token_manager) = self.token_manager {
            let (timestamp, nonce, token) = token_manager.generate_auth_token(&self.device_id);

            let auth_msg = serde_json::json!({
                "type": "auth",
                "device_id": self.device_id,
                "api_key": self.config.api_key.as_ref().unwrap(),
                "timestamp": timestamp,
                "nonce": nonce,
                "token": token,
            });

            let mut sender = self.sender.lock().await;
            if let Some(ref mut s) = *sender {
                s.send(Message::Text(auth_msg.to_string())).await
                    .map_err(|e| anyhow!("发送认证消息失败: {:?}", e))?;
                tracing::info!("认证消息已发送");
            } else {
                return Err(anyhow!("未连接到服务器"));
            }
        }
        Ok(())
    }

    /// 检查是否已配置认证
    pub fn has_auth(&self) -> bool {
        self.config.api_key.is_some()
    }

    /// 处理接收到的消息 (解析输入事件)
    fn handle_message(&self, text: String) {
        // 尝试解析为输入事件
        if let Ok(event) = serde_json::from_str::<crate::input::InputEvent>(&text) {
            let handler = self.input_handler.blocking_lock();
            if let Some(h) = handler.as_ref() {
                h(event);
            }
        } else {
            tracing::debug!("收到非输入事件消息: {}", text);
        }
    }

    /// 连接到 WebSocket 服务器
    pub async fn connect(&self) -> Result<()> {
        self._connect().await?;

        // 启动重连监控任务
        if self.config.auto_reconnect {
            let should_stop = self.should_stop.clone();
            let state = self.state.clone();
            let url = self.url.clone();
            let sender = self.sender.clone();
            let config = self.config.clone();
            let reconnect_count = self.reconnect_count.clone();
            let input_handler = self.input_handler.clone();

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(config.reconnect_interval_ms));

                loop {
                    interval.tick().await;

                    if *should_stop.lock().await {
                        break;
                    }

                    let current_state = *state.lock().await;
                    if current_state == ConnectionState::Disconnected {
                        // 尝试重连
                        if let Some(max_attempts) = config.max_reconnect_attempts {
                            let count = *reconnect_count.lock().await;
                            if count >= max_attempts {
                                tracing::error!("达到最大重连次数，停止重连");
                                *should_stop.lock().await = true;
                                break;
                            }
                        }

                        tracing::info!("尝试重新连接到: {}", url);

                        match connect_async(&url).await {
                            Ok((ws_stream, _)) => {
                                let (s, mut r) = ws_stream.split();
                                *sender.lock().await = Some(s);
                                *state.lock().await = ConnectionState::Connected;
                                *reconnect_count.lock().await = 0;

                                tracing::info!("重连成功");

                                // 如果配置了认证，发送认证消息
                                #[cfg(feature = "security")]
                                if config.api_key.is_some() {
                                    // 注意：重连时无法访问 self，需要将 token_manager 克隆到闭包中
                                    // 这里简化处理，仅记录日志
                                    tracing::info!("重连后需要重新认证");
                                }

                                // 启动接收任务
                                let state_clone = state.clone();
                                let handler = input_handler.clone();
                                tokio::spawn(async move {
                                    while let Some(msg) = r.next().await {
                                        match msg {
                                            Ok(Message::Text(text)) => {
                                                // 尝试解析为输入事件
                                                if let Ok(event) = serde_json::from_str::<crate::input::InputEvent>(&text) {
                                                    let h = handler.lock().await;
                                                    if let Some(h_fn) = h.as_ref() {
                                                        h_fn(event);
                                                    }
                                                } else {
                                                    tracing::debug!("收到消息: {}", text);
                                                }
                                            }
                                            Ok(Message::Close(_)) => {
                                                tracing::warn!("服务器关闭连接");
                                                *state_clone.lock().await = ConnectionState::Disconnected;
                                                break;
                                            }
                                            Err(e) => {
                                                tracing::error!("接收错误: {}", e);
                                                *state_clone.lock().await = ConnectionState::Disconnected;
                                                break;
                                            }
                                            _ => {}
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::warn!("重连失败: {}", e);
                                *reconnect_count.lock().await += 1;
                            }
                        }
                    }
                }
            });
        }

        Ok(())
    }

    /// 内部连接实现
    async fn _connect(&self) -> Result<()> {
        *self.state.lock().await = ConnectionState::Connecting;

        tracing::info!("连接到服务器: {}", self.url);

        let (ws_stream, _) = connect_async(&self.url)
            .await
            .map_err(|e| anyhow!("连接失败: {}", e))?;

        let (sender, mut receiver) = ws_stream.split();

        // 保存 sender
        *self.sender.lock().await = Some(sender);
        *self.state.lock().await = ConnectionState::Connected;
        *self.reconnect_count.lock().await = 0;

        // 如果配置了认证，发送认证消息
        #[cfg(feature = "security")]
        if self.has_auth() {
            if let Err(e) = self.send_auth().await {
                tracing::warn!("发送认证消息失败: {}", e);
            }
        }

        // 启动接收任务
        let connected = self.state.clone();
        let input_handler = self.input_handler.clone();
        tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // 尝试解析为输入事件
                        if let Ok(event) = serde_json::from_str::<crate::input::InputEvent>(&text) {
                            let h = input_handler.lock().await;
                            if let Some(h_fn) = h.as_ref() {
                                h_fn(event);
                            }
                        } else {
                            tracing::debug!("收到消息: {}", text);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        tracing::warn!("服务器关闭连接");
                        *connected.lock().await = ConnectionState::Disconnected;
                        break;
                    }
                    Err(e) => {
                        tracing::error!("接收错误: {}", e);
                        *connected.lock().await = ConnectionState::Disconnected;
                        break;
                    }
                    _ => {}
                }
            }
        });

        tracing::info!("连接成功");
        Ok(())
    }

    /// 发送视频数据包
    pub async fn send_packet(&self, data: Vec<u8>, is_key_frame: bool) -> Result<()> {
        let mut sender = self.sender.lock().await;
        let sender = sender.as_mut().ok_or_else(|| anyhow!("未连接"))?;

        let mut seq = self.sequence.lock().await;
        let packet = VideoPacket {
            device_id: self.device_id.clone(),
            timestamp: crate::capture::Frame::current_timestamp(),
            sequence: *seq,
            is_key_frame,
            data,
        };
        *seq += 1;
        drop(seq);

        let wire_data = packet.to_wire_format();

        match sender.send(Message::Binary(wire_data)).await {
            Ok(_) => Ok(()),
            Err(e) => {
                *self.state.lock().await = ConnectionState::Disconnected;
                Err(anyhow!("发送失败: {}", e))
            }
        }
    }

    /// 发送原始数据
    pub async fn send_raw(&self, data: Vec<u8>) -> Result<()> {
        let mut sender = self.sender.lock().await;
        let sender = sender.as_mut().ok_or_else(|| anyhow!("未连接"))?;

        match sender.send(Message::Binary(data)).await {
            Ok(_) => Ok(()),
            Err(e) => {
                *self.state.lock().await = ConnectionState::Disconnected;
                Err(anyhow!("发送失败: {}", e))
            }
        }
    }

    /// 检查是否已连接
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.lock().await, ConnectionState::Connected)
    }

    /// 获取连接状态
    pub async fn state(&self) -> ConnectionState {
        *self.state.lock().await
    }

    /// 断开连接
    pub async fn disconnect(&self) -> Result<()> {
        *self.should_stop.lock().await = true;
        *self.state.lock().await = ConnectionState::Disconnected;

        let mut sender = self.sender.lock().await;
        if let Some(mut s) = sender.take() {
            s.close().await?;
        }

        tracing::info!("连接已断开");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_packet_serialization() {
        let packet = VideoPacket {
            device_id: "test-device".to_string(),
            timestamp: 12345,
            sequence: 0,
            is_key_frame: true,
            data: vec![1, 2, 3, 4, 5],
        };

        let wire_data = packet.to_wire_format();
        assert!(wire_data.len() > 4);
        assert!(wire_data.len() > packet.data.len());
    }

    #[test]
    fn test_client_config_default() {
        let config = VideoClientConfig::default();
        assert!(config.auto_reconnect);
        assert_eq!(config.reconnect_interval_ms, 2000);
        assert!(config.max_reconnect_attempts.is_none());
    }
}
