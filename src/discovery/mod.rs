//! 设备发现和零配置连接模块
//!
//! 提供以下功能：
//! - 连接码生成和解析
//! - mDNS 局域网设备发现
//! - 公共信令服务客户端

// 设备发现模块尚未完全激活，标记为允许死代码
#![allow(dead_code)]

mod connection_code;
mod mdns;

pub use connection_code::ConnectionCode;
