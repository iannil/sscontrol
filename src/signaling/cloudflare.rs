//! Cloudflare Workers 信令服务客户端
//!
//! 与 Cloudflare Workers 部署的信令服务进行通信

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info, warn};

/// 默认信令服务器 URL
pub const DEFAULT_WORKER_URL: &str = "https://sscontrol-signaling.workers.dev";

/// 信令服务错误
#[derive(Debug, Error)]
pub enum SignalingError {
    #[error("Session not found or expired")]
    SessionNotFound,

    #[error("Session expired")]
    SessionExpired,

    #[error("Too many attempts")]
    TooManyAttempts,

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Invalid response")]
    InvalidResponse,
}

/// ICE 候选
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidate {
    pub candidate: String,
    #[serde(rename = "sdpMid", skip_serializing_if = "Option::is_none")]
    pub sdp_mid: Option<String>,
    #[serde(rename = "sdpMLineIndex", skip_serializing_if = "Option::is_none")]
    pub sdp_m_line_index: Option<u32>,
}

/// 会话信息
#[derive(Debug, Clone, Deserialize)]
pub struct SessionInfo {
    pub offer: String,
    pub candidates: Vec<IceCandidate>,
    pub public_key: Option<String>,
    pub expires_at: u64,
    pub status: String,
    pub answer: Option<String>,
    pub client_candidates: Option<Vec<IceCandidate>>,
}

/// 创建会话请求
#[derive(Debug, Serialize)]
struct CreateSessionRequest {
    session_id: String,
    offer: String,
    candidates: Vec<IceCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pin_hash: Option<String>,
    ttl: u64,
}

/// 创建会话响应
#[derive(Debug, Deserialize)]
struct CreateSessionResponse {
    success: bool,
    session_id: String,
    expires_at: u64,
}

/// 发送 Answer 请求
#[derive(Debug, Serialize)]
struct PostAnswerRequest {
    answer: String,
    candidates: Vec<IceCandidate>,
}

/// 添加 ICE 候选请求
#[derive(Debug, Serialize)]
struct PostIceRequest {
    role: String,
    candidate: IceCandidate,
}

/// 通用响应
#[derive(Debug, Deserialize)]
struct GenericResponse {
    success: Option<bool>,
    error: Option<String>,
}

/// Cloudflare 信令客户端
pub struct CloudflareSignaling {
    client: Client,
    base_url: String,
}

impl CloudflareSignaling {
    /// 创建新的客户端
    pub fn new(worker_url: Option<&str>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: worker_url.unwrap_or(DEFAULT_WORKER_URL).to_string(),
        }
    }

    /// 创建会话 (被控端调用)
    pub async fn create_session(
        &self,
        session_id: &str,
        offer: &str,
        candidates: Vec<IceCandidate>,
        public_key: Option<&str>,
        pin_hash: Option<&str>,
        ttl: u64,
    ) -> Result<u64, SignalingError> {
        let url = format!("{}/api/session", self.base_url);

        let request = CreateSessionRequest {
            session_id: session_id.to_string(),
            offer: offer.to_string(),
            candidates,
            public_key: public_key.map(|s| s.to_string()),
            pin_hash: pin_hash.map(|s| s.to_string()),
            ttl,
        };

        debug!("Creating session: {}", session_id);

        let response = self.client.post(&url).json(&request).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error: GenericResponse = response.json().await?;
            return Err(SignalingError::ServerError(
                error.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        let result: CreateSessionResponse = response.json().await?;
        info!("Session created: {}, expires at: {}", result.session_id, result.expires_at);

        Ok(result.expires_at)
    }

    /// 获取会话信息 (控制端调用)
    pub async fn get_session(&self, session_id: &str) -> Result<SessionInfo, SignalingError> {
        let url = format!("{}/api/session/{}", self.base_url, session_id);

        debug!("Getting session: {}", session_id);

        let response = self.client.get(&url).send().await?;

        let status = response.status();
        match status.as_u16() {
            200 => {
                let info: SessionInfo = response.json().await?;
                Ok(info)
            }
            404 => Err(SignalingError::SessionNotFound),
            410 => Err(SignalingError::SessionExpired),
            429 => Err(SignalingError::TooManyAttempts),
            _ => {
                let error: GenericResponse = response.json().await?;
                Err(SignalingError::ServerError(
                    error.error.unwrap_or_else(|| "Unknown error".to_string()),
                ))
            }
        }
    }

    /// 发送 Answer (控制端调用)
    pub async fn post_answer(
        &self,
        session_id: &str,
        answer: &str,
        candidates: Vec<IceCandidate>,
    ) -> Result<(), SignalingError> {
        let url = format!("{}/api/session/{}/answer", self.base_url, session_id);

        let request = PostAnswerRequest {
            answer: answer.to_string(),
            candidates,
        };

        debug!("Posting answer to session: {}", session_id);

        let response = self.client.post(&url).json(&request).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error: GenericResponse = response.json().await?;
            return Err(SignalingError::ServerError(
                error.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        info!("Answer posted to session: {}", session_id);
        Ok(())
    }

    /// 添加 ICE 候选
    pub async fn post_ice_candidate(
        &self,
        session_id: &str,
        role: &str, // "host" or "client"
        candidate: IceCandidate,
    ) -> Result<(), SignalingError> {
        let url = format!("{}/api/session/{}/ice", self.base_url, session_id);

        let request = PostIceRequest {
            role: role.to_string(),
            candidate,
        };

        debug!("Posting ICE candidate to session: {} as {}", session_id, role);

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let error: GenericResponse = response.json().await?;
            return Err(SignalingError::ServerError(
                error.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        Ok(())
    }

    /// 删除会话
    pub async fn delete_session(&self, session_id: &str) -> Result<(), SignalingError> {
        let url = format!("{}/api/session/{}", self.base_url, session_id);

        debug!("Deleting session: {}", session_id);

        let response = self.client.delete(&url).send().await?;

        if !response.status().is_success() {
            warn!("Failed to delete session: {}", session_id);
        }

        Ok(())
    }

    /// 轮询等待 Answer (被控端调用)
    pub async fn poll_for_answer(
        &self,
        session_id: &str,
        timeout: Duration,
        interval: Duration,
    ) -> Result<(String, Vec<IceCandidate>), SignalingError> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            let info = self.get_session(session_id).await?;

            if let Some(answer) = info.answer {
                let candidates = info.client_candidates.unwrap_or_default();
                return Ok((answer, candidates));
            }

            tokio::time::sleep(interval).await;
        }

        Err(SignalingError::SessionExpired)
    }
}

impl Default for CloudflareSignaling {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ice_candidate_serialization() {
        let candidate = IceCandidate {
            candidate: "candidate:123 1 udp 456 192.168.1.1 5000 typ host".to_string(),
            sdp_mid: Some("0".to_string()),
            sdp_m_line_index: Some(0),
        };

        let json = serde_json::to_string(&candidate).unwrap();
        assert!(json.contains("sdpMid"));
        assert!(json.contains("sdpMLineIndex"));
    }
}
