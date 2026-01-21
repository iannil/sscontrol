//! Token 管理
//!
//! 提供基于时间戳和 nonce 的 token 生成和验证

use anyhow::{anyhow, Result};
use rand::Rng;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::auth::ApiKeyAuth;

/// Token 管理器
///
/// 负责生成和验证认证 token，防止重放攻击
pub struct TokenManager {
    auth: ApiKeyAuth,
    /// 已使用的 nonce，用于防止重放攻击
    used_nonces: Arc<Mutex<HashSet<String>>>,
    /// nonce 清理间隔（秒）
    nonce_ttl: u64,
}

impl TokenManager {
    /// 创建新的 Token 管理器
    pub fn new(auth: ApiKeyAuth) -> Self {
        Self {
            auth,
            used_nonces: Arc::new(Mutex::new(HashSet::new())),
            nonce_ttl: 300, // 5 分钟
        }
    }

    /// 设置 nonce 过期时间
    pub fn with_nonce_ttl(mut self, ttl: u64) -> Self {
        self.nonce_ttl = ttl;
        self
    }

    /// 生成认证 token
    ///
    /// # 参数
    /// - `payload`: 要签名的数据
    ///
    /// # 返回
    /// 包含 timestamp、nonce 和 token 的元组
    pub fn generate_auth_token(&self, payload: &str) -> (u64, String, String) {
        let timestamp = ApiKeyAuth::current_timestamp();
        let nonce = self.generate_nonce();

        let data = format!("{}:{}:{}", payload, timestamp, nonce);
        let token = self.auth.generate_token(&data);

        (timestamp, nonce, token)
    }

    /// 验证认证 token
    ///
    /// # 参数
    /// - `payload`: 原始数据
    /// - `timestamp`: Unix 时间戳
    /// - `nonce`: 随机数
    /// - `token`: 要验证的 token
    ///
    /// # 返回
    /// 验证成功返回 Ok(())，失败返回错误
    pub async fn verify_auth_token(
        &self,
        payload: &str,
        timestamp: u64,
        nonce: &str,
        token: &str,
    ) -> Result<()> {
        // 检查时间戳
        let now = ApiKeyAuth::current_timestamp();
        if timestamp > now {
            return Err(anyhow!("时间戳在未来"));
        }

        let max_age = self.nonce_ttl;
        if now.saturating_sub(timestamp) > max_age {
            return Err(anyhow!("时间戳过期"));
        }

        // 检查 nonce 是否已使用（防重放攻击）
        {
            let mut used = self.used_nonces.lock().await;
            if used.contains(nonce) {
                return Err(anyhow!("nonce 已使用"));
            }
            used.insert(nonce.to_string());

            // 清理过期的 nonce
            if used.len() > 10000 {
                self.cleanup_old_nonces(&mut used, now).await;
            }
        }

        // 验证 token
        let data = format!("{}:{}:{}", payload, timestamp, nonce);
        if !self.auth.verify_token(&data, token) {
            return Err(anyhow!("token 验证失败"));
        }

        Ok(())
    }

    /// 生成随机 nonce
    fn generate_nonce(&self) -> String {
        let mut rng = rand::thread_rng();
        let nonce: u64 = rng.gen();
        format!("{:x}", nonce)
    }

    /// 清理过期的 nonce
    async fn cleanup_old_nonces(&self, used: &mut HashSet<String>, _now: u64) {
        // 简单的清理策略：当集合过大时清空
        // 生产环境应该使用更精细的时间窗口清理
        if used.len() > 10000 {
            used.clear();
        }
    }

    /// 获取内部认证器的引用
    pub fn auth(&self) -> &ApiKeyAuth {
        &self.auth
    }
}

impl Clone for TokenManager {
    fn clone(&self) -> Self {
        Self {
            auth: ApiKeyAuth::new(self.auth.api_key().to_string()),
            used_nonces: self.used_nonces.clone(),
            nonce_ttl: self.nonce_ttl,
        }
    }
}

/// 认证请求
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthRequest {
    pub api_key: String,
    pub timestamp: u64,
    pub nonce: String,
    pub token: String,
}

impl AuthRequest {
    /// 创建新的认证请求
    pub fn new(api_key: String, timestamp: u64, nonce: String, token: String) -> Self {
        Self {
            api_key,
            timestamp,
            nonce,
            token,
        }
    }

    /// 使用 TokenManager 创建认证请求
    pub fn from_manager(manager: &TokenManager, payload: &str, api_key: String) -> Self {
        let (timestamp, nonce, token) = manager.generate_auth_token(payload);
        Self {
            api_key,
            timestamp,
            nonce,
            token,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_manager_generate_and_verify() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        let manager = TokenManager::new(auth);

        let (timestamp, nonce, token) = manager.generate_auth_token("test-payload");

        // 验证应该成功
        assert!(manager
            .verify_auth_token("test-payload", timestamp, &nonce, &token)
            .await
            .is_ok());

        // 相同的 nonce 不应重复使用
        assert!(manager
            .verify_auth_token("test-payload", timestamp, &nonce, &token)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_token_manager_wrong_payload() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        let manager = TokenManager::new(auth);

        let (timestamp, nonce, token) = manager.generate_auth_token("test-payload");

        // 错误的 payload 应该验证失败
        assert!(manager
            .verify_auth_token("wrong-payload", timestamp, &nonce, &token)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_token_manager_expired_timestamp() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        let manager = TokenManager::new(auth);

        let old_timestamp = ApiKeyAuth::current_timestamp().saturating_sub(400);
        let nonce = manager.generate_nonce();
        let data = format!("test-payload:{}:{}", old_timestamp, nonce);
        let token = manager.auth().generate_token(&data);

        // 过期的时间戳应该验证失败
        assert!(manager
            .verify_auth_token("test-payload", old_timestamp, &nonce, &token)
            .await
            .is_err());
    }

    #[test]
    fn test_auth_request() {
        let auth = ApiKeyAuth::new("test-key".to_string());
        let manager = TokenManager::new(auth);

        let request = AuthRequest::from_manager(&manager, "test-payload", "my-api-key".to_string());

        assert_eq!(request.api_key, "my-api-key");
        assert!(!request.nonce.is_empty());
        assert!(!request.token.is_empty());
    }
}
