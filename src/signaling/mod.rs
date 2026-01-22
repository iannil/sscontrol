//! 公共信令服务客户端模块
//!
//! 提供与公共信令服务（如 Cloudflare Workers）的通信功能

mod cloudflare;

pub use cloudflare::{CloudflareSignaling, SessionInfo, IceCandidate, SignalingError};
