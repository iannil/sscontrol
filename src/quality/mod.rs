//! 质量优化模块
//!
//! 提供基于传统算法的智能质量优化功能，无需 AI/ML 依赖
//!
//! ## 模块
//! - `adaptive_bitrate`: 基于规则的自适应码率控制
//! - `roi_encoder`: 基于鼠标位置的区域化编码
//! - `static_detector`: 静态画面检测

pub mod adaptive_bitrate;
pub mod roi_encoder;
pub mod static_detector;

// Type alias for convenience
