//! SSControl 本地 REST API 信令服务器
//!
//! 兼容 Cloudflare Workers API 接口，用于局域网内的设备发现和连接
//!
//! # 运行方式
//!
//! ```bash
//! # 基本运行
//! sscontrol-signaling-rest --port 8080
//!
//! # 指定监听地址
//! sscontrol-signaling-rest --host 0.0.0.0 --port 8080
//! ```
//!
//! # API 端点
//!
//! - `POST /api/session` - 创建会话（被控端调用）
//! - `GET /api/session/{session_id}` - 获取会话信息（控制端调用）
//! - `POST /api/session/{session_id}/answer` - 发送 Answer（控制端调用）
//! - `POST /api/session/{session_id}/ice` - 添加 ICE 候选
//! - `DELETE /api/session/{session_id}` - 删除会话
//! - `GET /health` - 健康检查

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

/// SSControl REST 信令服务器
#[derive(Parser, Debug)]
#[command(name = "sscontrol-signaling-rest")]
#[command(about = "SSControl 本地 REST API 信令服务器")]
#[command(version)]
struct Args {
    /// 监听主机地址
    #[arg(long, default_value = "0.0.0.0", env = "SIGNALING_HOST")]
    host: String,

    /// 监听端口
    #[arg(short, long, default_value = "8080", env = "SIGNALING_PORT")]
    port: u16,
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

/// 会话数据
#[derive(Debug, Clone)]
struct Session {
    session_id: String,
    offer: String,
    candidates: Vec<IceCandidate>,
    public_key: Option<String>,
    pin_hash: Option<String>,
    expires_at: u64,
    answer: Option<String>,
    client_candidates: Vec<IceCandidate>,
    status: String,
}

/// 服务器状态
struct ServerState {
    sessions: HashMap<String, Session>,
}

impl ServerState {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    fn cleanup_expired(&mut self) {
        let now = current_timestamp();
        self.sessions.retain(|_, session| session.expires_at > now);
    }
}

type AppState = Arc<RwLock<ServerState>>;

/// 获取当前时间戳
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

// ============================================================================
// 请求/响应结构
// ============================================================================

/// 创建会话请求
#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    session_id: String,
    offer: String,
    candidates: Vec<IceCandidate>,
    #[serde(default)]
    public_key: Option<String>,
    #[serde(default)]
    pin_hash: Option<String>,
    #[serde(default = "default_ttl")]
    ttl: u64,
}

fn default_ttl() -> u64 {
    300
}

/// 创建会话响应
#[derive(Debug, Serialize)]
struct CreateSessionResponse {
    success: bool,
    session_id: String,
    expires_at: u64,
}

/// 获取会话响应
#[derive(Debug, Serialize)]
struct GetSessionResponse {
    offer: String,
    candidates: Vec<IceCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_key: Option<String>,
    expires_at: u64,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_candidates: Option<Vec<IceCandidate>>,
}

/// 发送 Answer 请求
#[derive(Debug, Deserialize)]
struct PostAnswerRequest {
    answer: String,
    #[serde(default)]
    candidates: Vec<IceCandidate>,
}

/// 添加 ICE 候选请求
#[derive(Debug, Deserialize)]
struct PostIceRequest {
    role: String,
    candidate: IceCandidate,
}

/// 通用响应
#[derive(Debug, Serialize)]
struct GenericResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// 健康检查响应
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    sessions: usize,
    timestamp: u64,
}

/// 错误响应
#[derive(Debug, Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

// ============================================================================
// API 处理函数
// ============================================================================

/// 创建会话
async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let mut state = state.write().await;

    // 清理过期会话
    state.cleanup_expired();

    // 检查会话是否已存在
    if state.sessions.contains_key(&req.session_id) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                success: false,
                error: "Session already exists".to_string(),
            }),
        )
            .into_response();
    }

    let expires_at = current_timestamp() + req.ttl;

    let session = Session {
        session_id: req.session_id.clone(),
        offer: req.offer,
        candidates: req.candidates,
        public_key: req.public_key,
        pin_hash: req.pin_hash,
        expires_at,
        answer: None,
        client_candidates: Vec::new(),
        status: "waiting".to_string(),
    };

    state.sessions.insert(req.session_id.clone(), session);

    tracing::info!("Session created: {}, expires at: {}", req.session_id, expires_at);

    (
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            success: true,
            session_id: req.session_id,
            expires_at,
        }),
    )
        .into_response()
}

/// 获取会话信息
async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let mut state = state.write().await;

    // 清理过期会话
    state.cleanup_expired();

    match state.sessions.get(&session_id) {
        Some(session) => {
            // 检查是否过期
            if session.expires_at < current_timestamp() {
                state.sessions.remove(&session_id);
                return (
                    StatusCode::GONE,
                    Json(ErrorResponse {
                        success: false,
                        error: "Session expired".to_string(),
                    }),
                )
                    .into_response();
            }

            let response = GetSessionResponse {
                offer: session.offer.clone(),
                candidates: session.candidates.clone(),
                public_key: session.public_key.clone(),
                expires_at: session.expires_at,
                status: session.status.clone(),
                answer: session.answer.clone(),
                client_candidates: if session.client_candidates.is_empty() {
                    None
                } else {
                    Some(session.client_candidates.clone())
                },
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: "Session not found".to_string(),
            }),
        )
            .into_response(),
    }
}

/// 发送 Answer
async fn post_answer(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<PostAnswerRequest>,
) -> impl IntoResponse {
    let mut state = state.write().await;

    match state.sessions.get_mut(&session_id) {
        Some(session) => {
            // 检查是否过期
            if session.expires_at < current_timestamp() {
                state.sessions.remove(&session_id);
                return (
                    StatusCode::GONE,
                    Json(ErrorResponse {
                        success: false,
                        error: "Session expired".to_string(),
                    }),
                )
                    .into_response();
            }

            session.answer = Some(req.answer);
            session.client_candidates = req.candidates;
            session.status = "answered".to_string();

            tracing::info!("Answer posted to session: {}", session_id);

            (
                StatusCode::OK,
                Json(GenericResponse {
                    success: true,
                    error: None,
                }),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: "Session not found".to_string(),
            }),
        )
            .into_response(),
    }
}

/// 添加 ICE 候选
async fn post_ice(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(req): Json<PostIceRequest>,
) -> impl IntoResponse {
    let mut state = state.write().await;

    match state.sessions.get_mut(&session_id) {
        Some(session) => {
            // 检查是否过期
            if session.expires_at < current_timestamp() {
                return (
                    StatusCode::GONE,
                    Json(ErrorResponse {
                        success: false,
                        error: "Session expired".to_string(),
                    }),
                )
                    .into_response();
            }

            match req.role.as_str() {
                "host" => {
                    session.candidates.push(req.candidate);
                }
                "client" => {
                    session.client_candidates.push(req.candidate);
                }
                _ => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            success: false,
                            error: "Invalid role, must be 'host' or 'client'".to_string(),
                        }),
                    )
                        .into_response();
                }
            }

            (
                StatusCode::OK,
                Json(GenericResponse {
                    success: true,
                    error: None,
                }),
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: "Session not found".to_string(),
            }),
        )
            .into_response(),
    }
}

/// 删除会话
async fn delete_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let mut state = state.write().await;

    if state.sessions.remove(&session_id).is_some() {
        tracing::info!("Session deleted: {}", session_id);
        (
            StatusCode::OK,
            Json(GenericResponse {
                success: true,
                error: None,
            }),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(GenericResponse {
                success: false,
                error: Some("Session not found".to_string()),
            }),
        )
    }
}

/// 健康检查
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let state = state.read().await;

    Json(HealthResponse {
        status: "ok".to_string(),
        sessions: state.sessions.len(),
        timestamp: current_timestamp(),
    })
}

/// 根路径
async fn root() -> impl IntoResponse {
    "SSControl REST Signaling Server"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let args = Args::parse();

    // 创建状态
    let state: AppState = Arc::new(RwLock::new(ServerState::new()));

    // 启动定期清理任务
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let mut state = cleanup_state.write().await;
            let before = state.sessions.len();
            state.cleanup_expired();
            let after = state.sessions.len();
            if before != after {
                tracing::info!("Cleaned up {} expired sessions", before - after);
            }
        }
    });

    // CORS 配置
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 构建路由
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/api/session", post(create_session))
        .route("/api/session/:session_id", get(get_session))
        .route("/api/session/:session_id", delete(delete_session))
        .route("/api/session/:session_id/answer", post(post_answer))
        .route("/api/session/:session_id/ice", post(post_ice))
        .layer(cors)
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    tracing::info!("==============================================");
    tracing::info!("  SSControl REST 信令服务器");
    tracing::info!("==============================================");
    tracing::info!("监听地址: http://{}", addr);
    tracing::info!("健康检查: http://{}/health", addr);
    tracing::info!("");
    tracing::info!("使用方法:");
    tracing::info!("  被控端: sscontrol host --signaling-url http://{}", addr);
    tracing::info!("  控制端: sscontrol connect --code <CODE> --pin <PIN> --signaling-url http://{}", addr);
    tracing::info!("==============================================");

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        assert!(ts > 0);
    }

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
