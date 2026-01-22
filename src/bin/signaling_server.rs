//! SSControl 信令服务器
//!
//! 独立运行的 WebRTC 信令服务器，用于在对等端之间交换 SDP 和 ICE 候选
//!
//! # 运行方式
//!
//! ```bash
//! # 基本运行
//! sscontrol-signaling --port 8443
//!
//! # 启用 TLS
//! sscontrol-signaling --port 8443 --tls-cert cert.pem --tls-key key.pem
//!
//! # 使用环境变量设置 API Key
//! SSCONTROL_API_KEY=your-secret-key sscontrol-signaling --port 8443
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::accept_async;
use tungstenite::protocol::Message;

#[cfg(feature = "security")]
use sscontrol::security::{ApiKeyAuth, TokenManager};

#[cfg(feature = "security")]
use std::pin::Pin;
#[cfg(feature = "security")]
use std::task::{Context, Poll};
#[cfg(feature = "security")]
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// TLS/非 TLS 统一流类型
#[cfg(feature = "security")]
enum MaybeTlsStream {
    Plain(TcpStream),
    Tls(tokio_rustls::server::TlsStream<TcpStream>),
}

#[cfg(feature = "security")]
impl AsyncRead for MaybeTlsStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::Tls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

#[cfg(feature = "security")]
impl AsyncWrite for MaybeTlsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(s) => Pin::new(s).poll_flush(cx),
            MaybeTlsStream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            MaybeTlsStream::Tls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

/// SSControl 信令服务器
#[derive(Parser, Debug)]
#[command(name = "sscontrol-signaling")]
#[command(about = "SSControl WebRTC 信令服务器")]
#[command(version)]
struct Args {
    /// 监听主机地址
    #[arg(long, default_value = "0.0.0.0", env = "SIGNALING_HOST")]
    host: String,

    /// 监听端口
    #[arg(short, long, default_value = "8443", env = "SIGNALING_PORT")]
    port: u16,

    /// TLS 证书路径
    #[arg(long, env = "SSCONTROL_TLS_CERT")]
    tls_cert: Option<String>,

    /// TLS 私钥路径
    #[arg(long, env = "SSCONTROL_TLS_KEY")]
    tls_key: Option<String>,
}

/// 认证状态
#[derive(Debug, Clone, PartialEq)]
enum AuthStatus {
    Unauthenticated,
    Authenticated(String),
    Failed,
}

/// 信令消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    #[serde(rename = "auth")]
    Auth {
        device_id: String,
        api_key: String,
        timestamp: u64,
        nonce: String,
        token: String,
    },
    #[serde(rename = "join")]
    Join { room_id: String },
    #[serde(rename = "peers")]
    Peers { peers: Vec<PeerInfo> },
    #[serde(rename = "new_peer")]
    NewPeer { peer_id: String },
    #[serde(rename = "peer_left")]
    PeerLeft { peer_id: String },
    #[serde(rename = "offer")]
    Offer { from: String, to: String, sdp: String },
    #[serde(rename = "answer")]
    Answer { from: String, to: String, sdp: String },
    #[serde(rename = "ice")]
    Ice {
        from: String,
        to: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "auth_success")]
    AuthSuccess { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: String,
}

struct Sender {
    peer_id: String,
    sender: tokio::sync::mpsc::UnboundedSender<String>,
}

struct Room {
    clients: Vec<String>,
}

struct ServerConfig {
    require_auth: bool,
    #[cfg(feature = "security")]
    token_manager: Option<TokenManager>,
}

struct ServerState {
    rooms: HashMap<String, Room>,
    clients: HashMap<String, Sender>,
    config: ServerConfig,
}

impl ServerState {
    fn new() -> Self {
        let require_auth = std::env::var("SSCONTROL_API_KEY").is_ok();

        #[cfg(feature = "security")]
        let token_manager = if let Ok(api_key) = std::env::var("SSCONTROL_API_KEY") {
            tracing::info!("API Key 认证已启用");
            Some(TokenManager::new(ApiKeyAuth::new(api_key)))
        } else {
            tracing::warn!("未设置 SSCONTROL_API_KEY，认证已禁用");
            None
        };

        #[cfg(not(feature = "security"))]
        if require_auth {
            tracing::warn!("需要 security feature 才能启用认证");
        }

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

    async fn verify_auth(&self, device_id: &str, timestamp: u64, nonce: &str, token: &str) -> bool {
        if !self.config.require_auth {
            return true;
        }

        #[cfg(feature = "security")]
        {
            if let Some(ref manager) = self.config.token_manager {
                return manager
                    .verify_auth_token(device_id, timestamp, nonce, token)
                    .await
                    .is_ok();
            }
            false
        }

        #[cfg(not(feature = "security"))]
        {
            let _ = (device_id, timestamp, nonce, token);
            false
        }
    }

    fn join_room(&mut self, peer_id: String, room_id: String) -> Vec<String> {
        if !self.rooms.contains_key(&room_id) {
            self.rooms.insert(
                room_id.clone(),
                Room {
                    clients: Vec::new(),
                },
            );
        }

        let room = self.rooms.get_mut(&room_id).unwrap();
        let existing_peers = room.clients.clone();
        room.clients.push(peer_id);
        existing_peers
    }

    fn leave_room(&mut self, peer_id: &str) -> Option<String> {
        for (room_id, room) in self.rooms.iter_mut() {
            if let Some(pos) = room.clients.iter().position(|id| id == peer_id) {
                room.clients.remove(pos);
                return Some(room_id.clone());
            }
        }
        None
    }

    fn get_room_peers(&self, room_id: &str) -> Vec<String> {
        self.rooms
            .get(room_id)
            .map(|r| r.clients.clone())
            .unwrap_or_default()
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

    fn stats(&self) -> (usize, usize) {
        (self.clients.len(), self.rooms.len())
    }
}

async fn handle_client<S>(
    ws_stream: tokio_tungstenite::WebSocketStream<S>,
    peer_id: String,
    state: Arc<RwLock<ServerState>>,
)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let auth_status = Arc::new(Mutex::new(AuthStatus::Unauthenticated));

    {
        let mut state = state.write().await;
        state.clients.insert(
            peer_id.clone(),
            Sender {
                peer_id: peer_id.clone(),
                sender: tx,
            },
        );
    }

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

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
                        )
                        .await;
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
        _ = send_task => {},
        _ = recv_task => {},
    }

    let mut state = state.write().await;
    state.clients.remove(&peer_id);

    if let Some(room_id) = state.leave_room(&peer_id) {
        if let Ok(msg) = serde_json::to_string(&SignalMessage::PeerLeft {
            peer_id: peer_id.clone(),
        }) {
            state.broadcast_to_room(&room_id, &msg, None);
        }

        let peers = state.get_room_peers(&room_id);
        if peers.is_empty() {
            state.rooms.remove(&room_id);
        }
    }

    tracing::debug!("客户端 {} 断开连接", peer_id);
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
            let state_read = state.read().await;
            let verified = state_read
                .verify_auth(&device_id, timestamp, &nonce, &token)
                .await;
            drop(state_read);

            let mut status = auth_status.lock().await;
            if verified {
                *status = AuthStatus::Authenticated(device_id.clone());
                tracing::info!("客户端 {} 认证成功", device_id);

                let state = state.read().await;
                if let Ok(msg) = serde_json::to_string(&SignalMessage::AuthSuccess {
                    message: "认证成功".to_string(),
                }) {
                    state.send_to(&peer_id, &msg);
                }
            } else {
                *status = AuthStatus::Failed;
                tracing::warn!("客户端 {} 认证失败", device_id);

                let state = state.read().await;
                if let Ok(msg) = serde_json::to_string(&SignalMessage::Error {
                    message: "认证失败".to_string(),
                }) {
                    state.send_to(&peer_id, &msg);
                }
            }
        }
        SignalMessage::Join { room_id } => {
            {
                let status = auth_status.lock().await;
                if state.read().await.config.require_auth
                    && !matches!(*status, AuthStatus::Authenticated(_))
                {
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

            let mut state = state.write().await;
            let existing_peers = state.join_room(peer_id.clone(), room_id.clone());

            let peers: Vec<PeerInfo> = existing_peers
                .iter()
                .map(|id| PeerInfo { id: id.clone() })
                .collect();

            if let Ok(msg) = serde_json::to_string(&SignalMessage::Peers { peers }) {
                state.send_to(&peer_id, &msg);
            }

            if let Ok(msg) = serde_json::to_string(&SignalMessage::NewPeer {
                peer_id: peer_id.clone(),
            }) {
                state.broadcast_to_room(&room_id, &msg, Some(&peer_id));
            }

            tracing::info!("客户端 {} 加入房间 {}", peer_id, room_id);
        }
        SignalMessage::Offer { to, sdp, .. } => {
            {
                let status = auth_status.lock().await;
                if state.read().await.config.require_auth
                    && !matches!(*status, AuthStatus::Authenticated(_))
                {
                    return;
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
            {
                let status = auth_status.lock().await;
                if state.read().await.config.require_auth
                    && !matches!(*status, AuthStatus::Authenticated(_))
                {
                    return;
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
        SignalMessage::Ice {
            to,
            candidate,
            sdp_mid,
            sdp_mline_index,
            ..
        } => {
            {
                let status = auth_status.lock().await;
                if state.read().await.config.require_auth
                    && !matches!(*status, AuthStatus::Authenticated(_))
                {
                    return;
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

/// 处理 HTTP 请求（用于健康检查）
/// 返回 Some(response) 如果是 HTTP 请求，None 如果应该继续作为 WebSocket 处理
async fn check_and_handle_http(
    stream: &mut TcpStream,
    state: Arc<RwLock<ServerState>>,
) -> Result<Option<()>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = [0u8; 1024];

    // 使用 peek 检查数据而不消耗
    let timeout = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        stream.peek(&mut buf),
    )
    .await;

    let n = match timeout {
        Ok(Ok(n)) if n > 0 => n,
        _ => return Ok(None), // 不是 HTTP 或超时，继续 WebSocket 处理
    };

    let request = String::from_utf8_lossy(&buf[..n]);

    // 检查是否是 HTTP 健康检查请求
    if request.starts_with("GET /health") || request.starts_with("HEAD /health") {
        // 读取完整请求
        let _ = stream.read(&mut buf).await;

        let state = state.read().await;
        let (clients, rooms) = state.stats();

        let body = format!(
            r#"{{"status":"ok","clients":{},"rooms":{}}}"#,
            clients, rooms
        );

        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );

        stream.write_all(response.as_bytes()).await?;
        stream.flush().await?;
        return Ok(Some(()));
    }

    // 检查是否是根路径请求
    if request.starts_with("GET / HTTP") || request.starts_with("HEAD / HTTP") {
        // 但如果包含 Upgrade: websocket，不处理为普通 HTTP
        if request.contains("Upgrade:") || request.contains("upgrade:") {
            return Ok(None);
        }

        let _ = stream.read(&mut buf).await;

        let body = "SSControl Signaling Server";
        let response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            body.len(),
            body
        );

        stream.write_all(response.as_bytes()).await?;
        stream.flush().await?;
        return Ok(Some(()));
    }

    Ok(None)
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = Args::parse();

    // TLS 支持（需要 security feature）
    #[cfg(feature = "security")]
    let tls_acceptor = if let (Some(cert_path), Some(key_path)) =
        (&args.tls_cert, &args.tls_key)
    {
        use std::fs::File;
        use std::io::BufReader;
        use rustls::pki_types::{CertificateDer, PrivateKeyDer};
        use tokio_rustls::rustls::ServerConfig;
        use tokio_rustls::TlsAcceptor;

        tracing::info!("启用 TLS: cert={}, key={}", cert_path, key_path);

        let cert_file = File::open(cert_path)?;
        let key_file = File::open(key_path)?;

        let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut BufReader::new(cert_file))
            .collect::<Result<Vec<_>, _>>()?;

        let key: PrivateKeyDer<'static> = {
            let mut keys: Vec<PrivateKeyDer<'static>> =
                rustls_pemfile::pkcs8_private_keys(&mut BufReader::new(key_file))
                    .map(|k| k.map(|k| k.into()))
                    .collect::<Result<Vec<_>, _>>()?;

            if keys.is_empty() {
                // 尝试读取 RSA 私钥
                let key_file = File::open(key_path)?;
                keys = rustls_pemfile::rsa_private_keys(&mut BufReader::new(key_file))
                    .map(|k| k.map(|k| k.into()))
                    .collect::<Result<Vec<_>, _>>()?;
            }

            if keys.is_empty() {
                anyhow::bail!("未找到有效的私钥");
            }

            keys.remove(0)
        };

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;

        Some(TlsAcceptor::from(Arc::new(config)))
    } else {
        None
    };

    #[cfg(not(feature = "security"))]
    if args.tls_cert.is_some() || args.tls_key.is_some() {
        tracing::warn!("TLS 需要 security feature，当前未启用");
    }

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let listener = TcpListener::bind(addr).await?;

    let scheme = if args.tls_cert.is_some() && args.tls_key.is_some() {
        "wss"
    } else {
        "ws"
    };

    tracing::info!("信令服务器启动");
    tracing::info!("监听地址: {}", addr);
    tracing::info!("WebSocket 端点: {}://{}", scheme, addr);
    tracing::info!("健康检查端点: http://{}/health", addr);

    let state = Arc::new(RwLock::new(ServerState::new()));

    // 优雅关闭
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    // 处理 SIGTERM/SIGINT
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("收到关闭信号，正在优雅关闭...");
        let _ = shutdown_tx_clone.send(());
    });

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((mut stream, addr)) => {
                        let peer_id = format!("peer_{}", addr.port());
                        let state = state.clone();

                        #[cfg(feature = "security")]
                        let tls_acceptor = tls_acceptor.clone();

                        tokio::spawn(async move {
                            // 首先检查是否是 HTTP 健康检查请求
                            if let Ok(Some(())) = check_and_handle_http(&mut stream, state.clone()).await {
                                return;
                            }

                            // 否则尝试 WebSocket 升级
                            #[cfg(feature = "security")]
                            let ws_stream = {
                                let maybe_tls_stream = if let Some(acceptor) = tls_acceptor {
                                    match acceptor.accept(stream).await {
                                        Ok(tls_stream) => MaybeTlsStream::Tls(tls_stream),
                                        Err(e) => {
                                            tracing::debug!("TLS 握手失败: {}", e);
                                            return;
                                        }
                                    }
                                } else {
                                    MaybeTlsStream::Plain(stream)
                                };

                                match accept_async(maybe_tls_stream).await {
                                    Ok(ws) => ws,
                                    Err(e) => {
                                        tracing::debug!("WebSocket 握手失败: {}", e);
                                        return;
                                    }
                                }
                            };

                            #[cfg(not(feature = "security"))]
                            let ws_stream = match accept_async(stream).await {
                                Ok(ws) => ws,
                                Err(e) => {
                                    tracing::debug!("WebSocket 握手失败: {}", e);
                                    return;
                                }
                            };

                            tracing::debug!("新连接: {}", peer_id);
                            handle_client(ws_stream, peer_id, state).await;
                        });
                    }
                    Err(e) => {
                        tracing::error!("接受连接失败: {}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                tracing::info!("服务器关闭");
                break;
            }
        }
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
    }

    #[test]
    fn test_args_default() {
        let args = Args::try_parse_from(["sscontrol-signaling"]).unwrap();
        assert_eq!(args.host, "0.0.0.0");
        assert_eq!(args.port, 8443);
        assert!(args.tls_cert.is_none());
    }
}
