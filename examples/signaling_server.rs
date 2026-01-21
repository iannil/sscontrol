//! WebRTC 信令服务器
//!
//! 用于在对等端之间交换 SDP 和 ICE 候选
//!
//! 运行: cargo run --example signaling_server --features security

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::accept_async;
use tungstenite::protocol::Message;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

// 安全相关导入 (当启用 security feature 时)
#[cfg(feature = "security")]
use sscontrol::security::{ApiKeyAuth, TokenManager};

/// 认证状态
#[derive(Debug, Clone, PartialEq)]
enum AuthStatus {
    /// 未认证
    Unauthenticated,
    /// 认证成功
    Authenticated(String),  // device_id
    /// 认证失败
    Failed,
}

/// 信令消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    /// 认证消息
    #[serde(rename = "auth")]
    Auth {
        device_id: String,
        api_key: String,
        timestamp: u64,
        nonce: String,
        token: String,
    },
    /// 客户端加入
    #[serde(rename = "join")]
    Join { room_id: String },
    /// 房间内现有客户端列表
    #[serde(rename = "peers")]
    Peers { peers: Vec<PeerInfo> },
    /// 新客户端加入房间
    #[serde(rename = "new_peer")]
    NewPeer { peer_id: String },
    /// 客户端离开
    #[serde(rename = "peer_left")]
    PeerLeft { peer_id: String },
    /// SDP Offer
    #[serde(rename = "offer")]
    Offer { from: String, to: String, sdp: String },
    /// SDP Answer
    #[serde(rename = "answer")]
    Answer { from: String, to: String, sdp: String },
    /// ICE Candidate
    #[serde(rename = "ice")]
    Ice { from: String, to: String, candidate: String, sdp_mid: String, sdp_mline_index: u16 },
    /// 错误
    #[serde(rename = "error")]
    Error { message: String },
    /// 认证成功响应
    #[serde(rename = "auth_success")]
    AuthSuccess { message: String },
}

/// 客户端信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: String,
}

/// 发送器包装
struct Sender {
    peer_id: String,
    sender: tokio::sync::mpsc::UnboundedSender<String>,
}

/// 房间
struct Room {
    clients: Vec<String>,
}

/// 服务器配置
struct ServerConfig {
    /// 是否需要认证
    require_auth: bool,
    /// Token 管理器 (可选)
    #[cfg(feature = "security")]
    token_manager: Option<TokenManager>,
}

/// 服务器状态
struct ServerState {
    rooms: HashMap<String, Room>,
    clients: HashMap<String, Sender>,
    config: ServerConfig,
}

impl ServerState {
    fn new() -> Self {
        // 检查是否设置了 API Key 环境变量
        let require_auth = std::env::var("SSCONTROL_API_KEY").is_ok();

        #[cfg(feature = "security")]
        let token_manager = if let Ok(api_key) = std::env::var("SSCONTROL_API_KEY") {
            Some(TokenManager::new(ApiKeyAuth::new(api_key)))
        } else {
            None
        };

        Self {
            rooms: HashMap::new(),
            clients: HashMap::new(),
            config: ServerConfig {
                require_auth,
                #[cfg(feature = "security")]
                token_manager,
            },
        }
    }

    /// 验证认证消息
    async fn verify_auth(&self, device_id: &str, timestamp: u64, nonce: &str, token: &str) -> bool {
        if !self.config.require_auth {
            // 不需要认证时，直接通过
            return true;
        }

        #[cfg(feature = "security")]
        {
            if let Some(ref manager) = self.config.token_manager {
                return manager.verify_auth_token(device_id, timestamp, nonce, token).await.is_ok();
            }
            false
        }

        #[cfg(not(feature = "security"))]
        {
            // 没有 security feature 时，如果设置了 require_auth 但没有 token_manager，拒绝
            false
        }
    }

    /// 客户端加入房间
    fn join_room(&mut self, peer_id: String, room_id: String) -> Vec<String> {
        if !self.rooms.contains_key(&room_id) {
            self.rooms.insert(room_id.clone(), Room { clients: Vec::new() });
        }

        let room = self.rooms.get_mut(&room_id).unwrap();
        let existing_peers = room.clients.clone();
        room.clients.push(peer_id.clone());
        existing_peers
    }

    /// 客户端离开房间
    fn leave_room(&mut self, peer_id: &str) -> Option<String> {
        for (room_id, room) in self.rooms.iter_mut() {
            if let Some(pos) = room.clients.iter().position(|id| id == peer_id) {
                room.clients.remove(pos);
                if room.clients.is_empty() {
                    return Some(room_id.clone());
                }
                return Some(room_id.clone());
            }
        }
        None
    }

    /// 获取房间内的客户端
    fn get_room_peers(&self, room_id: &str) -> Vec<String> {
        self.rooms
            .get(room_id)
            .map(|r| r.clients.clone())
            .unwrap_or_default()
    }

    /// 发送消息给指定客户端
    fn send_to(&self, peer_id: &str, msg: &str) -> bool {
        if let Some(sender) = self.clients.get(peer_id) {
            sender.sender.send(msg.to_string()).is_ok()
        } else {
            false
        }
    }

    /// 广播消息给房间内的所有客户端
    fn broadcast_to_room(&self, room_id: &str, msg: &str, exclude: Option<&str>) {
        if let Some(room) = self.rooms.get(room_id) {
            for peer_id in &room.clients {
                if let Some(exclude_id) = exclude {
                    if peer_id == exclude_id {
                        continue;
                    }
                }
                self.send_to(peer_id, msg);
            }
        }
    }
}

async fn handle_client(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    peer_id: String,
    state: Arc<RwLock<ServerState>>,
) {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // 认证状态
    let auth_status = Arc::new(Mutex::new(AuthStatus::Unauthenticated));

    // 注册客户端
    {
        let mut state = state.write().await;
        state.clients.insert(peer_id.clone(), Sender {
            peer_id: peer_id.clone(),
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
    let peer_id_clone = peer_id.clone();
    let auth_status_clone = auth_status.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                        handle_signal(
                            signal,
                            peer_id_clone.clone(),
                            state_clone.clone(),
                            auth_status_clone.clone(),
                        ).await;
                    }
                }
                Ok(Message::Close(_)) => break,
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
    state.clients.remove(&peer_id);

    if let Some(room_id) = state.leave_room(&peer_id) {
        // 通知其他客户端
        if let Ok(msg) = serde_json::to_string(&SignalMessage::PeerLeft {
            peer_id: peer_id.clone(),
        }) {
            state.broadcast_to_room(&room_id, &msg, None);
        }

        // 如果房间为空，删除房间
        let peers = state.get_room_peers(&room_id);
        if peers.is_empty() {
            state.rooms.remove(&room_id);
        }
    }

    tracing::info!("客户端 {} 断开连接", peer_id);
}

async fn handle_signal(
    signal: SignalMessage,
    peer_id: String,
    state: Arc<RwLock<ServerState>>,
    auth_status: Arc<Mutex<AuthStatus>>,
) {
    match signal {
        SignalMessage::Auth {
            device_id,
            api_key: _,
            timestamp,
            nonce,
            token,
        } => {
            // 处理认证消息
            let state_read = state.read().await;
            let verified = state_read.verify_auth(&device_id, timestamp, &nonce, &token).await;
            drop(state_read); // 释放读锁

            let mut status = auth_status.lock().await;
            if verified {
                *status = AuthStatus::Authenticated(device_id.clone());
                tracing::info!("客户端 {} 认证成功", device_id);

                // 发送认证成功响应
                let state = state.read().await;
                if let Ok(msg) = serde_json::to_string(&SignalMessage::AuthSuccess {
                    message: "认证成功".to_string(),
                }) {
                    state.send_to(&peer_id, &msg);
                }
            } else {
                *status = AuthStatus::Failed;
                tracing::warn!("客户端 {} 认证失败", device_id);

                // 发送认证失败响应
                let state = state.read().await;
                if let Ok(msg) = serde_json::to_string(&SignalMessage::Error {
                    message: "认证失败".to_string(),
                }) {
                    state.send_to(&peer_id, &msg);
                }
            }
        }
        SignalMessage::Join { room_id } => {
            // 检查认证状态
            {
                let status = auth_status.lock().await;
                if state.read().await.config.require_auth {
                    if !matches!(*status, AuthStatus::Authenticated(_)) {
                        tracing::warn!("未认证的客户端 {} 尝试加入房间", peer_id);
                        let state = state.read().await;
                        if let Ok(msg) = serde_json::to_string(&SignalMessage::Error {
                            message: "需要先认证".to_string(),
                        }) {
                            state.send_to(&peer_id, &msg);
                        }
                        return;
                    }
                }
            }

            let mut state = state.write().await;
            let existing_peers = state.join_room(peer_id.clone(), room_id.clone());

            // 发送房间内现有客户端列表
            let peers: Vec<PeerInfo> = existing_peers
                .iter()
                .map(|id| PeerInfo { id: id.clone() })
                .collect();

            if let Ok(msg) = serde_json::to_string(&SignalMessage::Peers { peers }) {
                state.send_to(&peer_id, &msg);
            }

            // 通知其他客户端有新成员加入
            if let Ok(msg) = serde_json::to_string(&SignalMessage::NewPeer {
                peer_id: peer_id.clone(),
            }) {
                state.broadcast_to_room(&room_id, &msg, Some(&peer_id));
            }

            tracing::info!("客户端 {} 加入房间 {}", peer_id, room_id);
        }
        SignalMessage::Offer { to, sdp, .. } => {
            // 检查认证状态
            {
                let status = auth_status.lock().await;
                if state.read().await.config.require_auth {
                    if !matches!(*status, AuthStatus::Authenticated(_)) {
                        return;
                    }
                }
            }

            let state = state.read().await;
            if let Ok(msg) = serde_json::to_string(&SignalMessage::Offer {
                from: peer_id,
                to: to.clone(),
                sdp,
            }) {
                state.send_to(&to, &msg);
            }
        }
        SignalMessage::Answer { to, sdp, .. } => {
            // 检查认证状态
            {
                let status = auth_status.lock().await;
                if state.read().await.config.require_auth {
                    if !matches!(*status, AuthStatus::Authenticated(_)) {
                        return;
                    }
                }
            }

            let state = state.read().await;
            if let Ok(msg) = serde_json::to_string(&SignalMessage::Answer {
                from: peer_id,
                to: to.clone(),
                sdp,
            }) {
                state.send_to(&to, &msg);
            }
        }
        SignalMessage::Ice { to, candidate, sdp_mid, sdp_mline_index, .. } => {
            // 检查认证状态
            {
                let status = auth_status.lock().await;
                if state.read().await.config.require_auth {
                    if !matches!(*status, AuthStatus::Authenticated(_)) {
                        return;
                    }
                }
            }

            let state = state.read().await;
            if let Ok(msg) = serde_json::to_string(&SignalMessage::Ice {
                from: peer_id,
                to: to.clone(),
                candidate,
                sdp_mid,
                sdp_mline_index,
            }) {
                state.send_to(&to, &msg);
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

    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("信令服务器监听: {}", addr);
    tracing::info!("WebSocket 端点: ws://{}", addr);

    let state = Arc::new(RwLock::new(ServerState::new()));

    while let Ok((stream, addr)) = listener.accept().await {
        let peer_id = format!("peer_{}", addr.port());
        let state = state.clone();

        tokio::spawn(async move {
            let ws_stream = accept_async(stream).await;

            match ws_stream {
                Ok(ws) => {
                    tracing::debug!("新连接: {}", peer_id);
                    handle_client(ws, peer_id, state).await;
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
    fn test_signal_message_serialization() {
        let msg = SignalMessage::Join {
            room_id: "test_room".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"join\""));

        let msg2 = SignalMessage::Offer {
            from: "peer1".to_string(),
            to: "peer2".to_string(),
            sdp: "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\n".to_string(),
        };
        let json2 = serde_json::to_string(&msg2).unwrap();
        assert!(json2.contains("\"offer\""));
    }
}
