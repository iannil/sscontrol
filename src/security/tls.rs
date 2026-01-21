//! TLS 配置
//!
//! 提供 TLS 证书配置和验证功能

use anyhow::{anyhow, Result};
use std::path::Path;

/// TLS 配置
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

impl TlsConfig {
    /// 从环境变量加载
    ///
    /// 环境变量:
    /// - `SSCONTROL_TLS_CERT`: 证书文件路径
    /// - `SSCONTROL_TLS_KEY`: 私钥文件路径
    pub fn from_env() -> Option<Self> {
        let cert_path = std::env::var("SSCONTROL_TLS_CERT").ok()?;
        let key_path = std::env::var("SSCONTROL_TLS_KEY").ok()?;
        Some(Self { cert_path, key_path })
    }

    /// 从文件路径创建
    pub fn new(cert_path: String, key_path: String) -> Self {
        Self { cert_path, key_path }
    }

    /// 验证证书文件存在
    pub fn validate(&self) -> Result<()> {
        if !Path::new(&self.cert_path).exists() {
            return Err(anyhow!("证书文件不存在: {}", self.cert_path));
        }
        if !Path::new(&self.key_path).exists() {
            return Err(anyhow!("私钥文件不存在: {}", self.key_path));
        }
        Ok(())
    }

    /// 创建 TLS 连接器 (客户端)
    ///
    /// 当 security feature 启用时可用
    #[cfg(feature = "security")]
    pub fn create_client_connector(&self) -> Result<tokio_rustls::TlsConnector> {
        use tokio_rustls::rustls::ClientConfig;
        use tokio_rustls::rustls::RootCertStore;

        // 使用系统根证书
        let mut root_store = RootCertStore::empty();
        // 添加 Mozilla 根证书
        let certs = rustls_native_certs::load_native_certs()
            .map_err(|e| anyhow!("加载系统根证书失败: {:?}", e))?;
        for cert in certs {
            root_store.add(cert).ok();
        }

        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(tokio_rustls::TlsConnector::from(std::sync::Arc::new(config)))
    }

    /// 创建 TLS 接受器 (服务器)
    ///
    /// 当 security feature 启用时可用
    #[cfg(feature = "security")]
    pub fn create_server_config(&self) -> Result<std::sync::Arc<rustls::ServerConfig>> {
        use rustls::ServerConfig;
        use rustls_pemfile::{certs, private_key};
        use std::io::BufReader;

        // 读取证书
        let cert_file = std::fs::File::open(&self.cert_path)?;
        let mut cert_reader = BufReader::new(cert_file);
        let cert_chain = certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow!("读取证书失败: {:?}", e))?;

        // 读取私钥
        let key_file = std::fs::File::open(&self.key_path)?;
        let mut key_reader = BufReader::new(key_file);
        let key = private_key(&mut key_reader)
            .map_err(|e| anyhow!("读取私钥失败: {:?}", e))?
            .ok_or_else(|| anyhow!("未找到私钥"))?;

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)
            .map_err(|e| anyhow!("创建服务器配置失败: {:?}", e))?;

        Ok(std::sync::Arc::new(config))
    }
}

/// 从环境变量或默认路径加载 TLS 配置
impl Default for TlsConfig {
    fn default() -> Self {
        Self::new(
            "cert.pem".to_string(),
            "key.pem".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls_config_new() {
        let config = TlsConfig::new("cert.pem".to_string(), "key.pem".to_string());
        assert_eq!(config.cert_path, "cert.pem");
        assert_eq!(config.key_path, "key.pem");
    }

    #[test]
    fn test_tls_config_default() {
        let config = TlsConfig::default();
        assert_eq!(config.cert_path, "cert.pem");
        assert_eq!(config.key_path, "key.pem");
    }

    #[test]
    fn test_tls_config_validate_nonexistent() {
        let config = TlsConfig::new("/nonexistent/cert.pem".to_string(), "/nonexistent/key.pem".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_tls_config_from_env() {
        // 环境变量未设置时应返回 None
        std::env::remove_var("SSCONTROL_TLS_CERT");
        std::env::remove_var("SSCONTROL_TLS_KEY");
        assert!(TlsConfig::from_env().is_none());

        // 环境变量设置后应返回 Some
        std::env::set_var("SSCONTROL_TLS_CERT", "/path/to/cert.pem");
        std::env::set_var("SSCONTROL_TLS_KEY", "/path/to/key.pem");
        let config = TlsConfig::from_env();
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.cert_path, "/path/to/cert.pem");
        assert_eq!(config.key_path, "/path/to/key.pem");

        // 清理环境变量
        std::env::remove_var("SSCONTROL_TLS_CERT");
        std::env::remove_var("SSCONTROL_TLS_KEY");
    }
}
