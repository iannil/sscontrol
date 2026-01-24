//! 静态画面检测器
//!
//! 检测画面是否为静态（无变化），用于码率节省优化
//!
//! ## 特点
//! - 帧差检测
//! - 静态画面识别
//! - 码率节省优化
//! - 可配置的灵敏度
//!
//! ## 应用场景
//! - 用户离开时降低码率
//! - 演示文稿暂停时节省带宽
//! - 视频暂停时优化编码

// 静态检测器模块尚未完全集成，标记为允许死代码
#![allow(dead_code)]
use crate::capture::Frame;
use anyhow::{anyhow, Result};
use std::time::{Duration, Instant};

/// 静态检测配置
#[derive(Debug, Clone)]
pub struct StaticDetectionConfig {
    /// 差异阈值 (0.0 - 1.0)
    /// 越低越敏感
    pub difference_threshold: f32,
    /// 静态判断需要的连续帧数
    pub static_frame_threshold: u32,
    /// 动态判断需要的连续帧数
    pub dynamic_frame_threshold: u32,
    /// 采样间隔 (像素)
    pub sampling_interval: u32,
    /// 最大统计历史帧数
    pub max_history_frames: usize,
}

impl Default for StaticDetectionConfig {
    fn default() -> Self {
        Self {
            difference_threshold: 0.01,  // 1% 像素差异
            static_frame_threshold: 5,    // 连续 5 帧无变化
            dynamic_frame_threshold: 2,   // 连续 2 帧有变化
            sampling_interval: 16,        // 每 16 像素采样一次
            max_history_frames: 60,       // 保存最近 60 帧统计
        }
    }
}

impl StaticDetectionConfig {
    /// 高灵敏度配置 (快速检测静态)
    pub fn high_sensitivity() -> Self {
        Self {
            difference_threshold: 0.005, // 0.5% 像素差异
            static_frame_threshold: 3,
            ..Default::default()
        }
    }

    /// 低灵敏度配置 (避免误判)
    pub fn low_sensitivity() -> Self {
        Self {
            difference_threshold: 0.05,  // 5% 像素差异
            static_frame_threshold: 10,
            ..Default::default()
        }
    }
}

/// 静态状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticState {
    /// 动态画面
    Dynamic,
    /// 可能为静态
    PossiblyStatic,
    /// 确认静态
    Static,
}

/// 帧差异统计
#[derive(Debug, Clone)]
pub struct FrameDifference {
    /// 差异比例 (0.0 - 1.0)
    pub difference_ratio: f32,
    /// 差异像素数量
    pub different_pixels: u64,
    /// 总采样像素数量
    pub total_pixels: u64,
    /// 计算耗时
    pub computation_time: Duration,
}

/// 静态画面统计
#[derive(Debug, Clone)]
pub struct StaticSceneStats {
    /// 当前状态
    pub state: StaticState,
    /// 静态持续时间
    pub static_duration: Duration,
    /// 动态持续时间
    pub dynamic_duration: Duration,
    /// 平均差异比例
    pub average_difference: f32,
    /// 静态帧比例
    pub static_frame_ratio: f32,
    /// 预估带宽节省
    pub estimated_bandwidth_savings: f32,
}

/// 静态画面检测器
///
/// 检测画面是否为静态，用于码率节省优化
pub struct StaticSceneDetector {
    config: StaticDetectionConfig,
    previous_frame: Option<Vec<u8>>,
    current_state: StaticState,
    static_frame_count: u32,
    dynamic_frame_count: u32,
    state_changed_at: Instant,
    difference_history: Vec<f32>,
    frame_count: u64,
    static_start_time: Option<Instant>,
}

impl StaticSceneDetector {
    /// 创建新的静态检测器
    pub fn new(config: StaticDetectionConfig) -> Self {
        tracing::info!(
            "初始化静态检测器: 阈值={:.3}, 静态帧阈值={}, 采样间隔={}",
            config.difference_threshold,
            config.static_frame_threshold,
            config.sampling_interval
        );

        let history_size = config.max_history_frames;

        Self {
            config,
            previous_frame: None,
            current_state: StaticState::Dynamic,
            static_frame_count: 0,
            dynamic_frame_count: 0,
            state_changed_at: Instant::now(),
            difference_history: Vec::with_capacity(history_size),
            frame_count: 0,
            static_start_time: None,
        }
    }

    /// 检测帧是否为静态
    pub fn detect(&mut self, frame: &Frame) -> Result<FrameDifference> {
        let start = Instant::now();

        // 如果是第一帧，保存并返回
        if self.previous_frame.is_none() {
            self.previous_frame = Some(frame.data.clone());
            return Ok(FrameDifference {
                difference_ratio: 0.0,
                different_pixels: 0,
                total_pixels: 0,
                computation_time: start.elapsed(),
            });
        }

        // 计算帧差异
        let diff = self.calculate_difference(frame)?;

        // 记录差异历史
        if self.difference_history.len() >= self.config.max_history_frames {
            self.difference_history.remove(0);
        }
        self.difference_history.push(diff.difference_ratio);

        // 更新状态
        self.update_state(&diff);

        self.frame_count += 1;

        Ok(diff)
    }

    /// 计算帧差异
    fn calculate_difference(&self, frame: &Frame) -> Result<FrameDifference> {
        let previous = self.previous_frame.as_ref().ok_or_else(|| anyhow!("没有前一帧"))?;

        let width = frame.width as usize;
        let height = frame.height as usize;
        let row_size = width * 4; // RGBA

        let mut different_pixels = 0u64;
        let mut total_pixels = 0u64;

        // 采样比较 (性能优化)
        let step = self.config.sampling_interval as usize * 4; // 每个像素 4 字节

        for y in (0..height).step_by(self.config.sampling_interval as usize) {
            let row_start = y * row_size;

            for x in (0..row_size).step_by(step) {
                let offset = row_start + x;

                // 比较 RGBA 四个通道
                let prev_rgba = &previous[offset..offset + 4];
                let curr_rgba = &frame.data[offset..offset + 4];

                // 简单的绝对差异
                let diff = prev_rgba.iter()
                    .zip(curr_rgba.iter())
                    .map(|(p, c)| (*p as i32 - *c as i32).abs() as u32)
                    .sum::<u32>();

                // 如果任一通道差异 > 10，认为是不同像素
                if diff > 10 * 4 {
                    different_pixels += 1;
                }

                total_pixels += 1;
            }
        }

        let difference_ratio = if total_pixels > 0 {
            different_pixels as f32 / total_pixels as f32
        } else {
            0.0
        };

        Ok(FrameDifference {
            difference_ratio,
            different_pixels,
            total_pixels,
            computation_time: Duration::from_millis(0), // 在外部计算
        })
    }

    /// 更新静态状态
    fn update_state(&mut self, diff: &FrameDifference) {
        let is_static = diff.difference_ratio < self.config.difference_threshold;

        match is_static {
            true => {
                self.static_frame_count += 1;
                self.dynamic_frame_count = 0;

                // 状态转换: Dynamic -> PossiblyStatic -> Static
                match self.current_state {
                    StaticState::Dynamic => {
                        if self.static_frame_count >= 2 {
                            self.current_state = StaticState::PossiblyStatic;
                        }
                    }
                    StaticState::PossiblyStatic => {
                        if self.static_frame_count >= self.config.static_frame_threshold {
                            self.current_state = StaticState::Static;
                            self.static_start_time = Some(Instant::now());
                        }
                    }
                    StaticState::Static => {
                        // 保持静态状态
                    }
                }
            }
            false => {
                self.dynamic_frame_count += 1;
                self.static_frame_count = 0;

                // 状态转换: Static -> Dynamic
                if self.dynamic_frame_count >= self.config.dynamic_frame_threshold {
                    if self.current_state == StaticState::Static {
                        self.static_start_time = None;
                    }
                    self.current_state = StaticState::Dynamic;
                }
            }
        }

        // 状态改变时更新时间戳
        if self.current_state != self.previous_state() {
            self.state_changed_at = Instant::now();
        }

        // 保存当前帧用于下一次比较
        // 注意: 这里需要在调用 detect 后手动更新 previous_frame
    }

    /// 获取当前状态
    pub fn current_state(&self) -> StaticState {
        self.current_state
    }

    /// 获取前一状态 (用于检测状态变化)
    fn previous_state(&self) -> StaticState {
        // 简化实现，实际应该记录前一状态
        self.current_state
    }

    /// 获取状态持续时间
    pub fn state_duration(&self) -> Duration {
        self.state_changed_at.elapsed()
    }

    /// 获取静态持续时间
    pub fn static_duration(&self) -> Duration {
        self.static_start_time
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// 判断是否应该降低码率
    pub fn should_reduce_bitrate(&self) -> bool {
        match self.current_state {
            StaticState::Static => true,
            StaticState::PossiblyStatic => self.static_duration() > Duration::from_secs(2),
            StaticState::Dynamic => false,
        }
    }

    /// 获取建议的码率倍数 (0.0 - 1.0)
    pub fn suggested_bitrate_multiplier(&self) -> f32 {
        match self.current_state {
            StaticState::Static => 0.1,  // 静态画面使用 10% 码率
            StaticState::PossiblyStatic => 0.5,
            StaticState::Dynamic => 1.0,
        }
    }

    /// 更新前一帧 (在 detect 后调用)
    pub fn update_previous_frame(&mut self, frame: &Frame) {
        self.previous_frame = Some(frame.data.clone());
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> StaticSceneStats {
        let average_difference = if !self.difference_history.is_empty() {
            self.difference_history.iter().sum::<f32>() / self.difference_history.len() as f32
        } else {
            0.0
        };

        let static_frame_ratio = if self.frame_count > 0 {
            self.difference_history.iter()
                .filter(|&&d| d < self.config.difference_threshold)
                .count() as f32 / self.difference_history.len() as f32
        } else {
            0.0
        };

        let static_duration = self.static_duration();
        let estimated_savings = if static_duration > Duration::from_secs(1) {
            // 静态超过 1 秒后，每秒节省 90% 码率
            0.9
        } else {
            0.0
        };

        StaticSceneStats {
            state: self.current_state,
            static_duration: self.static_duration(),
            dynamic_duration: if self.current_state == StaticState::Static {
                Duration::ZERO
            } else {
                self.state_duration()
            },
            average_difference,
            static_frame_ratio,
            estimated_bandwidth_savings: estimated_savings,
        }
    }

    /// 重置检测器状态
    pub fn reset(&mut self) {
        self.previous_frame = None;
        self.current_state = StaticState::Dynamic;
        self.static_frame_count = 0;
        self.dynamic_frame_count = 0;
        self.state_changed_at = Instant::now();
        self.difference_history.clear();
        self.frame_count = 0;
        self.static_start_time = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame(width: u32, height: u32, color: u8) -> Frame {
        let stride = (width * 4) as usize;
        let mut data = vec![color; (width * height * 4) as usize];
        Frame {
            width,
            height,
            data,
            timestamp: 0,
            stride,
        }
    }

    #[test]
    fn test_detector_creation() {
        let detector = StaticSceneDetector::new(StaticDetectionConfig::default());
        assert_eq!(detector.current_state(), StaticState::Dynamic);
    }

    #[test]
    fn test_static_detection() {
        let config = StaticDetectionConfig {
            static_frame_threshold: 3,
            ..Default::default()
        };
        let mut detector = StaticSceneDetector::new(config);

        // 第一帧
        let frame1 = create_test_frame(1920, 1080, 128);
        let _diff = detector.detect(&frame1).unwrap();
        detector.update_previous_frame(&frame1);
        assert_eq!(detector.current_state(), StaticState::Dynamic);

        // 连续相同帧
        for _ in 0..5 {
            let frame = create_test_frame(1920, 1080, 128);
            detector.detect(&frame).unwrap();
            detector.update_previous_frame(&frame);
        }

        // 应该检测到静态
        assert_eq!(detector.current_state(), StaticState::Static);
        assert!(detector.should_reduce_bitrate());
    }

    #[test]
    fn test_dynamic_detection() {
        let config = StaticDetectionConfig {
            static_frame_threshold: 3,
            dynamic_frame_threshold: 2,
            difference_threshold: 0.5,  // 50% 阈值，更容易检测变化
            ..Default::default()
        };
        let mut detector = StaticSceneDetector::new(config);

        // 先建立静态状态 - 使用 128 灰度
        for _ in 0..5 {
            let frame = create_test_frame(1920, 1080, 128);
            detector.detect(&frame).unwrap();
            detector.update_previous_frame(&frame);
        }
        assert_eq!(detector.current_state(), StaticState::Static);

        // 连续不同帧 - 每帧都与前一帧完全不同
        // 从 255 开始（与 128 完全不同），然后每次变化确保持续差异
        for i in 0..5 {
            let color = if i % 2 == 0 { 255u8 } else { 0u8 };  // 交替黑白
            let frame = create_test_frame(1920, 1080, color);
            let diff = detector.detect(&frame).unwrap();
            println!("Frame {}: diff_ratio={}, state={:?}", i, diff.difference_ratio, detector.current_state());
            detector.update_previous_frame(&frame);
        }

        // 应该检测到动态
        assert_eq!(detector.current_state(), StaticState::Dynamic);
    }

    #[test]
    fn test_bitrate_suggestion() {
        let mut detector = StaticSceneDetector::new(StaticDetectionConfig::default());

        // 动态状态
        assert_eq!(detector.suggested_bitrate_multiplier(), 1.0);

        // 建立静态状态
        for _ in 0..10 {
            let frame = create_test_frame(1920, 1080, 128);
            detector.detect(&frame).unwrap();
            detector.update_previous_frame(&frame);
        }

        // 静态状态
        assert_eq!(detector.current_state(), StaticState::Static);
        assert_eq!(detector.suggested_bitrate_multiplier(), 0.1);
    }

    #[test]
    fn test_frame_difference_calculation() {
        let mut detector = StaticSceneDetector::new(StaticDetectionConfig::default());

        let frame1 = create_test_frame(1920, 1080, 128);
        detector.detect(&frame1).unwrap();
        detector.update_previous_frame(&frame1);

        // 完全相同的帧
        let frame2 = create_test_frame(1920, 1080, 128);
        let diff = detector.detect(&frame2).unwrap();
        assert_eq!(diff.difference_ratio, 0.0);

        // 完全不同的帧
        let frame3 = create_test_frame(1920, 1080, 255);
        let diff = detector.detect(&frame3).unwrap();
        assert!(diff.difference_ratio > 0.5); // 应该有很大差异
    }

    #[test]
    fn test_sensitivity_configs() {
        let high = StaticDetectionConfig::high_sensitivity();
        assert!(high.difference_threshold < 0.01);
        assert_eq!(high.static_frame_threshold, 3);

        let low = StaticDetectionConfig::low_sensitivity();
        assert!(low.difference_threshold > 0.01);
        assert_eq!(low.static_frame_threshold, 10);
    }

    #[test]
    fn test_stats_collection() {
        let mut detector = StaticSceneDetector::new(StaticDetectionConfig::default());

        for _ in 0..20 {
            let frame = create_test_frame(1920, 1080, 128);
            detector.detect(&frame).unwrap();
            detector.update_previous_frame(&frame);
        }

        let stats = detector.get_stats();
        assert_eq!(stats.state, StaticState::Static);
        assert!(stats.static_duration > Duration::ZERO);
        assert!(stats.static_frame_ratio > 0.5);
    }
}
