//! API Key 认证实现
//!
//! 提供基于共享密钥的认证机制，支持 HMAC-SHA256 token 生成和验证

use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// API Key 认证器
pub struct ApiKeyAuth {
    api_key: String,
}

impl ApiKeyAuth {
    /// 从环境变量创建
    ///
    /// 环境变量: `SSCONTROL_API_KEY`
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("SSCONTROL_API_KEY")
            .map_err(|_| anyhow!("SSCONTROL_API_KEY 环境变量未设置"))?;
        Ok(Self { api_key })
    }

    /// 从字符串创建
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    /// 获取 API Key 的引用
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// 验证 API Key
    ///
    /// 使用常量时间比较以防止时序攻击
    pub fn verify(&self, key: &str) -> bool {
        constant_time_eq(&self.api_key, key)
    }

    /// 生成认证 token (HMAC-SHA256)
    ///
    /// # 参数
    /// - `payload`: 要签名的数据
    ///
    /// # 返回
    /// 十六进制编码的 HMAC-SHA256 签名
    pub fn generate_token(&self, payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.api_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// 生成带时间戳的认证 token
    ///
    /// # 参数
    /// - `payload`: 要签名的数据
    /// - `timestamp`: Unix 时间戳
    pub fn generate_token_with_timestamp(&self, payload: &str, timestamp: u64) -> String {
        let data = format!("{}:{}", payload, timestamp);
        self.generate_token(&data)
    }

    /// 验证 token
    ///
    /// # 参数
    /// - `payload`: 原始数据
    /// - `token`: 要验证的 token
    pub fn verify_token(&self, payload: &str, token: &str) -> bool {
        let expected = self.generate_token(payload);
        constant_time_eq(&expected, token)
    }

    /// 验证带时间戳的 token
    ///
    /// # 参数
    /// - `payload`: 原始数据
    /// - `timestamp`: Unix 时间戳
    /// - `token`: 要验证的 token
    /// - `max_age`: 最大允许的时间差（秒）
    pub fn verify_token_with_timestamp(
        &self,
        payload: &str,
        timestamp: u64,
        token: &str,
        max_age: u64,
    ) -> bool {
        // 检查时间戳是否在允许范围内
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0); // 系统时间异常时返回 0，验证将失败

        if timestamp > now || now.saturating_sub(timestamp) > max_age {
            return false;
        }

        let data = format!("{}:{}", payload, timestamp);
        self.verify_token(&data, token)
    }

    /// 生成当前时间戳
    pub fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// 常量时间比较，防止时序攻击
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).all(|(x, y)| x == y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_auth_from_string() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        assert_eq!(auth.api_key(), "test-key");
    }

    #[test]
    fn test_api_key_verify() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        assert!(auth.verify("test-key"));
        assert!(!auth.verify("wrong-key"));
        assert!(!auth.verify("test-ke"));  // 不同长度
    }

    #[test]
    fn test_generate_token() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        let token1 = auth.generate_token("test-payload");
        let token2 = auth.generate_token("test-payload");
        let token3 = auth.generate_token("different-payload");

        // 相同输入产生相同 token
        assert_eq!(token1, token2);
        // 不同输入产生不同 token
        assert_ne!(token1, token3);
    }

    #[test]
    fn test_verify_token() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        let token = auth.generate_token("test-payload");

        assert!(auth.verify_token("test-payload", &token));
        assert!(!auth.verify_token("test-payload", "wrong-token"));
        assert!(!auth.verify_token("wrong-payload", &token));
    }

    #[test]
    fn test_token_with_timestamp() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        let timestamp = ApiKeyAuth::current_timestamp();
        let token = auth.generate_token_with_timestamp("test-payload", timestamp);

        // 验证正确的时间戳 token
        assert!(auth.verify_token_with_timestamp("test-payload", timestamp, &token, 300));

        // 验证过期的 token
        let old_timestamp = timestamp.saturating_sub(400);
        assert!(!auth.verify_token_with_timestamp("test-payload", old_timestamp, &token, 300));
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq("same", "same"));
        assert!(!constant_time_eq("same", "different"));
        assert!(!constant_time_eq("short", "longer"));
    }
}
