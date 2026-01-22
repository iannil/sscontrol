//! 设备发现和零配置连接模块
//!
//! 提供以下功能：
//! - 连接码生成和解析
//! - mDNS 局域网设备发现
//! - 公共信令服务客户端

mod connection_code;
mod mdns;

pub use connection_code::{ConnectionCode, ConnectionCodeError};
pub use mdns::{MdnsService, MdnsDiscovery, DiscoveredPeer};
