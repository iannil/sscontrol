//! 信令服务模块
//!
//! 提供内嵌信令服务器，用于局域网极简模式

mod embedded;

pub use embedded::{EmbeddedSignalingServer, HostSignalEvent};
