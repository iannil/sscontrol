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

#[cfg(feature = "discovery")]
pub mod discovery;

// 信令模块 - 内嵌信令服务器
pub mod signaling;

// Web 查看器模块
pub mod viewer;
