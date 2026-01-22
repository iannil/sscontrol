//! 配置管理模块
//!
//! 负责加载和管理应用程序配置

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::Result;
use uuid::Uuid;

/// 应用程序配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub capture: CaptureConfig,
    pub logging: LoggingConfig,
    /// 安全配置
    #[serde(default)]
    pub security: SecurityConfig,
    /// WebRTC 配置
    #[serde(default)]
    pub webrtc: WebRTCConfig,
}

/// 服务器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// WebSocket 服务器地址
    pub url: String,
    /// 设备 ID (自动生成或手动指定)
    #[serde(default = "default_device_id")]
    pub device_id: String,
}

/// 屏幕捕获配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CaptureConfig {
    /// 目标帧率
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// 屏幕索引 (None = 主显示器)
    #[serde(default)]
    pub screen_index: Option<u32>,
    /// 捕获宽度 (None = 原始宽度)
    #[serde(default)]
    pub width: Option<u32>,
    /// 捕获高度 (None = 原始高度)
    #[serde(default)]
    pub height: Option<u32>,
}

/// 日志配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// 日志级别: trace, debug, info, warn, error
    #[serde(default = "default_log_level")]
    pub level: String,
    /// 日志文件路径 (None = 仅控制台)
    #[serde(default)]
    pub file: Option<String>,
}

/// 安全配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// API Key (推荐通过环境变量设置: SSCONTROL_API_KEY)
    #[serde(default)]
    pub api_key: Option<String>,
    /// TLS 证书路径 (推荐通过环境变量设置: SSCONTROL_TLS_CERT)
    #[serde(default)]
    pub tls_cert: Option<String>,
    /// TLS 私钥路径 (推荐通过环境变量设置: SSCONTROL_TLS_KEY)
    #[serde(default)]
    pub tls_key: Option<String>,
    /// 是否强制使用 TLS
    #[serde(default)]
    pub require_tls: bool,
    /// Token 最大有效期（秒）
    #[serde(default = "default_token_ttl")]
    pub token_ttl: u64,
}

/// WebRTC 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebRTCConfig {
    /// STUN 服务器列表
    #[serde(default = "default_stun_servers")]
    pub stun_servers: Vec<String>,
    /// TURN 服务器配置
    #[serde(default)]
    pub turn_servers: Vec<TurnServerConfig>,
    /// ICE 传输策略: "all" 或 "relay"
    #[serde(default = "default_ice_transport_policy")]
    pub ice_transport_policy: String,
}

/// TURN 服务器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TurnServerConfig {
    /// TURN 服务器 URL (例如: turn:turn.example.com:3478)
    pub url: String,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                url: "ws://localhost:8080".to_string(),
                device_id: Uuid::new_v4().to_string(),
            },
            capture: CaptureConfig {
                fps: 30,
                screen_index: None,
                width: None,
                height: None,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: None,
            },
            security: SecurityConfig::default(),
            webrtc: WebRTCConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            url: "ws://localhost:8080".to_string(),
            device_id: Uuid::new_v4().to_string(),
        }
    }
}

impl Default for CaptureConfig {
    fn default() -> Self {
        CaptureConfig {
            fps: 30,
            screen_index: None,
            width: None,
            height: None,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            level: "info".to_string(),
            file: None,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        SecurityConfig {
            api_key: None,
            tls_cert: None,
            tls_key: None,
            require_tls: false,
            token_ttl: 300, // 5 分钟
        }
    }
}

impl Default for WebRTCConfig {
    fn default() -> Self {
        WebRTCConfig {
            stun_servers: default_stun_servers(),
            turn_servers: Vec::new(),
            ice_transport_policy: "all".to_string(),
        }
    }
}

fn default_device_id() -> String {
    Uuid::new_v4().to_string()
}

fn default_fps() -> u32 {
    30
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_token_ttl() -> u64 {
    300 // 5 分钟
}

fn default_stun_servers() -> Vec<String> {
    vec![
        "stun:stun.l.google.com:19302".to_string(),
        "stun:stun1.l.google.com:19302".to_string(),
    ]
}

fn default_ice_transport_policy() -> String {
    "all".to_string()
}

impl Config {
    /// 从文件加载配置
    ///
    /// 如果文件不存在或解析失败，返回默认配置
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            tracing::warn!("配置文件不存在: {:?}, 使用默认配置", path);
            return Ok(Config::default());
        }

        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("配置文件解析失败: {}", e))?;

        tracing::info!("配置加载成功: {:?}", path);
        Ok(config)
    }

    /// 保存配置到文件
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path.as_ref(), content)?;
        Ok(())
    }

    /// 获取配置文件路径
    ///
    /// 优先级: 命令行指定 > 当前目录 > 用户主目录
    pub fn get_config_path(cli_path: Option<&str>) -> String {
        if let Some(p) = cli_path {
            return p.to_string();
        }

        // 首先检查当前目录
        if Path::new("config.toml").exists() {
            return "config.toml".to_string();
        }

        // 然后检查用户配置目录
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = format!("{}/.config/sscontrol", home);
            let config_path = format!("{}/config.toml", config_dir);
            if Path::new(&config_path).exists() {
                return config_path;
            }
        }

        // 默认返回当前目录的配置文件路径
        "config.toml".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.capture.fps, 30);
        assert_eq!(config.logging.level, "info");
        assert!(!config.server.device_id.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let _parsed: Config = toml::from_str(&toml_str).unwrap();
    }
}
