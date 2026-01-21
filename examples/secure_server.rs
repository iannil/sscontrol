//! 安全校视频流服务器示例
//!
//! 展示如何使用安全特性运行 WebSocket 视频服务器
//!
//! 运行:
//! ```bash
//! # 设置环境变量
//! export SSCONTROL_API_KEY="your-secret-api-key"
//!
//! # 运行服务器
//! cargo run --example secure_server --features security
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::accept_async;
use tungstenite::protocol::Message;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

// 安全相关导入
use sscontrol::security::{ApiKeyAuth, TokenManager};

/// 认证状态
#[derive(Debug, Clone, PartialEq)]
enum AuthStatus {
    Unauthenticated,
    Authenticated(String),
    Failed,
}

/// 服务器消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// 认证
    #[serde(rename = "auth")]
    Auth {
        device_id: String,
        api_key: String,
        timestamp: u64,
        nonce: String,
        token: String,
    },
    /// 视频数据
    #[serde(rename = "video")]
    Video {
        device_id: String,
        sequence: u64,
        data: String,  // base64 编码
    },
    /// 心跳
    #[serde(rename = "ping")]
    Ping {},
    /// 错误
    #[serde(rename = "error")]
    Error { message: String },
    /// 认证成功
    #[serde(rename = "auth_success")]
    AuthSuccess { message: String },
}

/// 客户端信息
struct Client {
    device_id: Option<String>,
    sender: tokio::sync::mpsc::UnboundedSender<String>,
}

/// 服务器状态
struct ServerState {
    clients: HashMap<String, Client>,
    token_manager: Option<TokenManager>,
    require_auth: bool,
}

impl ServerState {
    fn new() -> Self {
        // 检查是否设置了 API Key
        let require_auth = std::env::var("SSCONTROL_API_KEY").is_ok();
        let token_manager = if let Ok(api_key) = std::env::var("SSCONTROL_API_KEY") {
            Some(TokenManager::new(ApiKeyAuth::new(api_key)))
        } else {
            None
        };

        tracing::info!("认证要求: {}", require_auth);

        Self {
            clients: HashMap::new(),
            token_manager,
            require_auth,
        }
    }

    /// 验证认证令牌
    async fn verify_auth(&self, device_id: &str, timestamp: u64, nonce: &str, token: &str) -> bool {
        if !self.require_auth {
            return true;
        }

        if let Some(ref manager) = self.token_manager {
            manager.verify_auth_token(device_id, timestamp, nonce, token).await.is_ok()
        } else {
            false
        }
    }
}

async fn handle_client(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    client_id: String,
    state: Arc<RwLock<ServerState>>,
) {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // 认证状态
    let auth_status = Arc::new(Mutex::new(AuthStatus::Unauthenticated));

    // 注册客户端
    {
        let mut state = state.write().await;
        state.clients.insert(client_id.clone(), Client {
            device_id: None,
            sender: tx,
        });
    }

    // 消息发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // 消息接收任务
    let state_clone = state.clone();
    let client_id_clone = client_id.clone();
    let auth_status_clone = auth_status.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                        handle_message(
                            server_msg,
                            client_id_clone.clone(),
                            state_clone.clone(),
                            auth_status_clone.clone(),
                        ).await;
                    }
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("客户端 {} 关闭连接", client_id_clone);
                    break;
                }
                Err(e) => {
                    tracing::error!("接收错误: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // 等待任一任务完成
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // 清理
    let mut state = state.write().await;
    state.clients.remove(&client_id);
    tracing::info!("客户端 {} 断开连接", client_id);
}

async fn handle_message(
    msg: ServerMessage,
    client_id: String,
    state: Arc<RwLock<ServerState>>,
    auth_status: Arc<Mutex<AuthStatus>>,
) {
    match msg {
        ServerMessage::Auth {
            device_id,
            api_key: _,
            timestamp,
            nonce,
            token,
        } => {
            // 处理认证
            let state_read = state.read().await;
            let verified = state_read.verify_auth(&device_id, timestamp, &nonce, &token).await;
            drop(state_read);

            let mut status = auth_status.lock().await;
            if verified {
                *status = AuthStatus::Authenticated(device_id.clone());
                tracing::info!("客户端 {} 认证成功", device_id);

                // 更新客户端信息
                let mut state = state.write().await;
                if let Some(client) = state.clients.get_mut(&client_id) {
                    client.device_id = Some(device_id.clone());
                }

                // 发送认证成功响应
                if let Ok(msg) = serde_json::to_string(&ServerMessage::AuthSuccess {
                    message: "认证成功".to_string(),
                }) {
                    if let Some(client) = state.clients.get(&client_id) {
                        let _ = client.sender.send(msg);
                    }
                }
            } else {
                *status = AuthStatus::Failed;
                tracing::warn!("客户端 {} 认证失败", device_id);

                // 发送认证失败响应
                if let Ok(msg) = serde_json::to_string(&ServerMessage::Error {
                    message: "认证失败".to_string(),
                }) {
                    let state = state.read().await;
                    if let Some(client) = state.clients.get(&client_id) {
                        let _ = client.sender.send(msg);
                    }
                }
            }
        }
        ServerMessage::Video { device_id, sequence, .. } => {
            // 检查认证状态
            {
                let status = auth_status.lock().await;
                let require_auth = state.read().await.require_auth;
                if require_auth && !matches!(*status, AuthStatus::Authenticated(_)) {
                    tracing::warn!("未认证的客户端尝试发送视频数据");
                    return;
                }
            }

            tracing::debug!("收到来自 {} 的视频帧，序列号: {}", device_id, sequence);
        }
        ServerMessage::Ping {} => {
            // 响应心跳
            let status = auth_status.lock().await;
            let is_authenticated = matches!(*status, AuthStatus::Authenticated(_));
            drop(status);

            if is_authenticated {
                tracing::debug!("收到来自 {} 的心跳", client_id);
            }
        }
        _ => {}
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .init();

    // 检查环境变量
    let api_key = std::env::var("SSCONTROL_API_KEY");
    if api_key.is_ok() {
        tracing::info!("API Key 已设置，启用认证");
    } else {
        tracing::warn!("未设置 API_KEY 环境变量，允许未认证连接");
    }

    let addr = "127.0.0.1:8443";
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("安全视频服务器监听: {}", addr);
    tracing::info!("WebSocket 端点: ws://{}", addr);
    tracing::info!("");
    tracing::info!("提示: 设置 SSCONTROL_API_KEY 环境变量以启用认证");
    tracing::info!("例如: export SSCONTROL_API_KEY=\"my-secret-key\"");

    let state = Arc::new(RwLock::new(ServerState::new()));

    while let Ok((stream, addr)) = listener.accept().await {
        let client_id = format!("client_{}", addr.port());
        let state = state.clone();

        tokio::spawn(async move {
            let ws_stream = accept_async(stream).await;

            match ws_stream {
                Ok(ws) => {
                    tracing::debug!("新连接: {}", client_id);
                    handle_client(ws, client_id, state).await;
                }
                Err(e) => {
                    tracing::error!("WebSocket 握手失败: {}", e);
                }
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::Auth {
            device_id: "test-device".to_string(),
            api_key: "test-key".to_string(),
            timestamp: 1234567890,
            nonce: "abc123".to_string(),
            token: "xyz789".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"auth\""));
    }

    #[test]
    fn test_server_state_new() {
        let state = ServerState::new();
        assert!(!state.require_auth || state.token_manager.is_some());
    }
}
