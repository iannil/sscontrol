//! 安全模块
//!
//! 提供认证、TLS 加密和 token 管理功能

#![allow(dead_code)]

pub mod auth;
pub mod tls;
pub mod token;

pub use auth::ApiKeyAuth;
pub use tls::TlsConfig;
pub use token::TokenManager;

/// 安全配置
#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    pub api_key: Option<String>,
    pub tls_config: Option<TlsConfig>,
    pub require_tls: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert!(config.api_key.is_none());
        assert!(config.tls_config.is_none());
        assert!(!config.require_tls);
    }
}
