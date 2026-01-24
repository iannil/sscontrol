//! sscontrol - 无界面远程桌面应用库
//!
//! 提供屏幕捕获、编码和网络传输功能

pub mod capture;
pub mod config;
pub mod encoder;
pub mod input;
pub mod network;
pub mod security;
pub mod service;
pub mod webrtc;

// NAT 穿透模块 (零依赖)
pub mod nat;

// 质量优化模块
pub mod quality;

// 命令行工具模块
pub mod tools;

#[cfg(feature = "discovery")]
pub mod discovery;

#[cfg(feature = "pairing")]
pub mod pairing;

// 信令模块 - 内嵌信令服务器
pub mod signaling;

// Web 查看器模块
pub mod viewer;
