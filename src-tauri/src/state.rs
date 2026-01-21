use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use sscontrol::config::Config;

/// 应用状态
#[derive(Clone)]
pub struct AppState {
    /// 配置
    pub config: Arc<RwLock<Config>>,
    /// 连接状态
    pub connection_state: Arc<Mutex<ConnectionState>>,
}

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

impl ConnectionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectionState::Disconnected => "disconnected",
            ConnectionState::Connecting => "connecting",
            ConnectionState::Connected => "connected",
            ConnectionState::Reconnecting => "reconnecting",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "connecting" => ConnectionState::Connecting,
            "connected" => ConnectionState::Connected,
            "reconnecting" => ConnectionState::Reconnecting,
            _ => ConnectionState::Disconnected,
        }
    }
}

/// 连接统计信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionStats {
    /// 连接状态
    pub status: String,
    /// 已发送帧数
    pub frames_sent: u64,
    /// 当前 FPS
    pub fps: f64,
    /// 已发送字节数
    pub bytes_sent: u64,
    /// 连接时长（秒）
    pub uptime: u64,
    /// 延迟（毫秒）
    pub latency: u64,
    /// 重连次数
    pub reconnect_count: u64,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        ConnectionStats {
            status: "disconnected".to_string(),
            frames_sent: 0,
            fps: 0.0,
            bytes_sent: 0,
            uptime: 0,
            latency: 0,
            reconnect_count: 0,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        // 加载配置
        let config_path = Config::get_config_path(None);
        let config = Config::load(&config_path).unwrap_or_default();

        AppState {
            config: Arc::new(RwLock::new(config)),
            connection_state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
