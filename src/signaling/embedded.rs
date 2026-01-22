//! 内嵌信令服务器模块
//!
//! 使用 axum 实现，支持 HTTP 反向代理 (如 Cloudflare Tunnel)

use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::Method,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tower_http::cors::{Any, CorsLayer};

/// 信令消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    /// 加入房间
    #[serde(rename = "join")]
    Join { room_id: String },
    /// 房间内现有成员
    #[serde(rename = "peers")]
    Peers { peers: Vec<PeerInfo> },
    /// 新成员加入
    #[serde(rename = "new_peer")]
    NewPeer { peer_id: String },
    /// 成员离开
    #[serde(rename = "peer_left")]
    PeerLeft { peer_id: String },
    /// SDP Offer
    #[serde(rename = "offer")]
    Offer { from: String, to: String, sdp: String },
    /// SDP Answer
    #[serde(rename = "answer")]
    Answer { from: String, to: String, sdp: String },
    /// ICE 候选
    #[serde(rename = "ice")]
    Ice {
        from: String,
        to: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    },
    /// 错误
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: String,
}

/// Host 收到的信令事件
#[derive(Debug, Clone)]
pub enum HostSignalEvent {
    /// 新的 Viewer 连接
    ViewerJoined { peer_id: String },
    /// Viewer 断开
    ViewerLeft { peer_id: String },
    /// 收到 Offer
    Offer { from: String, sdp: String },
    /// 收到 ICE 候选
    Ice {
        from: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    },
}

/// 客户端发送器
struct ClientSender {
    sender: mpsc::UnboundedSender<String>,
}

/// 房间状态
struct Room {
    clients: Vec<String>,
}

/// 服务器状态
struct ServerState {
    rooms: HashMap<String, Room>,
    clients: HashMap<String, ClientSender>,
    host_event_tx: Option<mpsc::UnboundedSender<HostSignalEvent>>,
    peer_counter: AtomicU64,
}

impl ServerState {
    fn new() -> Self {
        Self {
            rooms: HashMap::new(),
            clients: HashMap::new(),
            host_event_tx: None,
            peer_counter: AtomicU64::new(0),
        }
    }

    fn next_peer_id(&self) -> String {
        let id = self.peer_counter.fetch_add(1, Ordering::SeqCst);
        format!("viewer_{}", id)
    }

    fn join_room(&mut self, peer_id: String, room_id: String) -> Vec<String> {
        let room = self.rooms.entry(room_id.clone()).or_insert(Room {
            clients: Vec::new(),
        });
        let existing = room.clients.clone();
        room.clients.push(peer_id.clone());

        // 通知 Host 有新 Viewer 加入
        if let Some(tx) = &self.host_event_tx {
            let _ = tx.send(HostSignalEvent::ViewerJoined { peer_id });
        }

        existing
    }

    fn leave_room(&mut self, peer_id: &str) -> Option<String> {
        for (room_id, room) in self.rooms.iter_mut() {
            if let Some(pos) = room.clients.iter().position(|id| id == peer_id) {
                room.clients.remove(pos);

                // 通知 Host Viewer 离开
                if let Some(tx) = &self.host_event_tx {
                    let _ = tx.send(HostSignalEvent::ViewerLeft {
                        peer_id: peer_id.to_string(),
                    });
                }

                return Some(room_id.clone());
            }
        }
        None
    }

    fn send_to(&self, peer_id: &str, msg: &str) -> bool {
        if let Some(sender) = self.clients.get(peer_id) {
            sender.sender.send(msg.to_string()).is_ok()
        } else {
            false
        }
    }

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

    /// 转发信令给 Host
    fn forward_to_host(&self, event: HostSignalEvent) {
        if let Some(tx) = &self.host_event_tx {
            let _ = tx.send(event);
        }
    }
}

/// 共享应用状态
#[derive(Clone)]
struct AppState {
    state: Arc<RwLock<ServerState>>,
}

/// 内嵌信令服务器
pub struct EmbeddedSignalingServer {
    port: u16,
    state: Arc<RwLock<ServerState>>,
    shutdown_tx: Option<broadcast::Sender<()>>,
    host_event_rx: Option<mpsc::UnboundedReceiver<HostSignalEvent>>,
}

impl EmbeddedSignalingServer {
    /// 创建新的内嵌信令服务器
    pub fn new(port: u16) -> Self {
        Self {
            port,
            state: Arc::new(RwLock::new(ServerState::new())),
            shutdown_tx: None,
            host_event_rx: None,
        }
    }

    /// 启动服务器
    pub async fn start(&mut self) -> Result<u16> {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        // 创建 Host 事件通道
        let (host_event_tx, host_event_rx) = mpsc::unbounded_channel();
        self.host_event_rx = Some(host_event_rx);
        self.state.write().await.host_event_tx = Some(host_event_tx);

        let app_state = AppState {
            state: self.state.clone(),
        };

        // 创建 CORS 层
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any);

        // 创建 axum 路由
        let app = Router::new()
            .route("/", get(root_handler))
            .route("/health", get(health_check))
            .route("/ws", get(ws_handler))
            .layer(cors)
            .with_state(app_state);

        let addr: SocketAddr = format!("0.0.0.0:{}", self.port).parse()?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let actual_port = listener.local_addr()?.port();

        tracing::info!("内嵌信令服务器启动: 0.0.0.0:{}", actual_port);

        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.recv().await;
                })
                .await
                .ok();
        });

        Ok(actual_port)
    }

    /// 获取 Host 事件接收器
    pub fn take_host_events(&mut self) -> Option<mpsc::UnboundedReceiver<HostSignalEvent>> {
        self.host_event_rx.take()
    }

    /// 发送 Answer 给 Viewer
    pub async fn send_answer(&self, to: &str, sdp: &str) {
        let msg = SignalMessage::Answer {
            from: "host".to_string(),
            to: to.to_string(),
            sdp: sdp.to_string(),
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            self.state.read().await.send_to(to, &json);
        }
    }

    /// 发送 ICE 候选给 Viewer
    pub async fn send_ice(&self, to: &str, candidate: &str, sdp_mid: &str, sdp_mline_index: u16) {
        let msg = SignalMessage::Ice {
            from: "host".to_string(),
            to: to.to_string(),
            candidate: candidate.to_string(),
            sdp_mid: sdp_mid.to_string(),
            sdp_mline_index,
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            self.state.read().await.send_to(to, &json);
        }
    }

    /// 停止服务器
    pub fn stop(&self) {
        if let Some(ref tx) = self.shutdown_tx {
            let _ = tx.send(());
        }
    }

    /// 获取监听端口
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// 根路径处理 - 同时支持健康检查和 WebSocket
async fn root_handler(
    ws: Option<WebSocketUpgrade>,
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    tracing::debug!("根路径请求, WebSocket升级: {}", ws.is_some());
    if let Some(ws) = ws {
        tracing::info!("接受 WebSocket 连接");
        ws.on_upgrade(move |socket| handle_socket(socket, app_state))
            .into_response()
    } else {
        tracing::debug!("HTTP 健康检查");
        Html("sscontrol signaling server - OK").into_response()
    }
}

/// 健康检查端点
async fn health_check() -> impl IntoResponse {
    Html("OK")
}

/// WebSocket 处理 (路径 /ws)
async fn ws_handler(ws: WebSocketUpgrade, State(app_state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, app_state))
}

/// 处理 WebSocket 连接
async fn handle_socket(socket: WebSocket, app_state: AppState) {
    let peer_id = {
        let state = app_state.state.read().await;
        state.next_peer_id()
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // 注册客户端
    {
        let mut state = app_state.state.write().await;
        state.clients.insert(peer_id.clone(), ClientSender { sender: tx });
    }

    tracing::info!("Viewer 连接: {}", peer_id);

    // 发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // 接收任务
    let state_clone = app_state.state.clone();
    let peer_id_clone = peer_id.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                        handle_signal(signal, &peer_id_clone, &state_clone).await;
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    tracing::debug!("接收错误: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    // 清理
    let mut state = app_state.state.write().await;
    state.clients.remove(&peer_id);

    if let Some(room_id) = state.leave_room(&peer_id) {
        if let Ok(msg) = serde_json::to_string(&SignalMessage::PeerLeft {
            peer_id: peer_id.clone(),
        }) {
            state.broadcast_to_room(&room_id, &msg, None);
        }
    }

    tracing::info!("Viewer 断开: {}", peer_id);
}

/// 处理信令消息
async fn handle_signal(
    signal: SignalMessage,
    peer_id: &str,
    state: &Arc<RwLock<ServerState>>,
) {
    match signal {
        SignalMessage::Join { room_id } => {
            let mut state = state.write().await;
            let existing_peers = state.join_room(peer_id.to_string(), room_id.clone());

            // 发送现有成员列表 (包括 host)
            let mut peers: Vec<PeerInfo> = existing_peers
                .iter()
                .map(|id| PeerInfo { id: id.clone() })
                .collect();
            // 添加 host 作为成员
            peers.push(PeerInfo { id: "host".to_string() });

            if let Ok(msg) = serde_json::to_string(&SignalMessage::Peers { peers }) {
                state.send_to(peer_id, &msg);
            }

            // 通知其他成员 (不通知 host，因为已经通过事件通知了)
            if let Ok(msg) = serde_json::to_string(&SignalMessage::NewPeer {
                peer_id: peer_id.to_string(),
            }) {
                state.broadcast_to_room(&room_id, &msg, Some(peer_id));
            }

            tracing::info!("Viewer {} 加入房间 {}", peer_id, room_id);
        }
        SignalMessage::Offer { to, sdp, .. } => {
            if to == "host" {
                // 转发给 Host
                let state = state.read().await;
                state.forward_to_host(HostSignalEvent::Offer {
                    from: peer_id.to_string(),
                    sdp,
                });
            } else {
                // 转发给其他 Viewer
                let state = state.read().await;
                if let Ok(msg) = serde_json::to_string(&SignalMessage::Offer {
                    from: peer_id.to_string(),
                    to: to.clone(),
                    sdp,
                }) {
                    state.send_to(&to, &msg);
                }
            }
        }
        SignalMessage::Answer { to, sdp, .. } => {
            let state = state.read().await;
            if let Ok(msg) = serde_json::to_string(&SignalMessage::Answer {
                from: peer_id.to_string(),
                to: to.clone(),
                sdp,
            }) {
                state.send_to(&to, &msg);
            }
        }
        SignalMessage::Ice {
            to,
            candidate,
            sdp_mid,
            sdp_mline_index,
            ..
        } => {
            if to == "host" {
                // 转发给 Host
                let state = state.read().await;
                state.forward_to_host(HostSignalEvent::Ice {
                    from: peer_id.to_string(),
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                });
            } else {
                // 转发给其他 Viewer
                let state = state.read().await;
                if let Ok(msg) = serde_json::to_string(&SignalMessage::Ice {
                    from: peer_id.to_string(),
                    to: to.clone(),
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                }) {
                    state.send_to(&to, &msg);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_message_serialization() {
        let msg = SignalMessage::Join {
            room_id: "test".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"join\""));
    }
}
