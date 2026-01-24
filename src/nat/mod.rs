//! NAT 穿透模块
//!
//! 实现零依赖的 NAT 穿透技术，无需第三方 STUN/TURN 服务器

// NAT 穿透模块尚未完全激活，标记为允许死代码和未使用导入
#![allow(dead_code, unused_imports)]

pub mod detector;
pub mod predictive_punching;

pub use detector::{NatDetector, NatType};

/// NAT 穿透结果
#[derive(Debug, Clone, PartialEq)]
pub enum TraversalResult {
    /// 直连成功
    DirectConnected,
    /// 需要预测性打洞
    PredictionNeeded,
    /// 无法穿透 (需要中继)
    Blocked,
}

/// NAT 穿透配置
#[derive(Debug, Clone)]
pub struct NatConfig {
    /// 预测性打洞尝试次数
    pub prediction_attempts: u16,
    /// 打洞超时时间 (毫秒)
    pub punch_timeout_ms: u64,
    /// 是否启用并行打洞
    pub enable_parallel_punch: bool,
}

impl Default for NatConfig {
    fn default() -> Self {
        Self {
            prediction_attempts: 100, // 尝试 100 个预测端口
            punch_timeout_ms: 3000,   // 3 秒超时
            enable_parallel_punch: true,
        }
    }
}
