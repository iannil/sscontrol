//! 基于规则的自适应码率控制器
//!
//! 使用启发式规则引擎根据网络状态动态调整码率
//!
//! ## 特点
//! - 无 AI/ML 依赖
//! - 响应时间 <100ms
//! - 基于阈值的可预测行为
//! - 低内存占用
//!
//! ## 规则示例
//! - 延迟 >100ms → 降低码率 30%
//! - 丢包 >5% → 降低码率 50%
//! - 带宽 >10Mbps → 提升到最高质量
//! - 带宽波动大 → 选择保守码率

// 自适应码率模块尚未完全集成，标记为允许死代码
#![allow(dead_code)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// 网络状态
#[derive(Debug, Clone, Copy)]
pub struct NetworkState {
    /// 估计带宽 (Mbps)
    pub bandwidth_mbps: f64,
    /// 往返延迟 (ms)
    pub latency_ms: f64,
    /// 丢包率 (0.0 - 1.0)
    pub packet_loss: f64,
    /// 抖动 (ms)
    pub jitter_ms: f64,
}

impl Default for NetworkState {
    fn default() -> Self {
        Self {
            bandwidth_mbps: 10.0,
            latency_ms: 50.0,
            packet_loss: 0.01,
            jitter_ms: 10.0,
        }
    }
}

/// ABR 配置
#[derive(Debug, Clone)]
pub struct AbreConfig {
    /// 最小码率 (kbps)
    pub min_bitrate: u32,
    /// 最大码率 (kbps)
    pub max_bitrate: u32,
    /// 初始码率 (kbps)
    pub initial_bitrate: u32,
    /// 历史数据窗口大小
    pub history_size: usize,
    /// 码率调整步长 (百分比)
    pub adjustment_step: f64,
    /// 高延迟阈值 (ms)
    pub high_latency_threshold: f64,
    /// 高丢包阈值 (0.0 - 1.0)
    pub high_packet_loss_threshold: f64,
    /// 低带宽阈值 (Mbps)
    pub low_bandwidth_threshold: f64,
    /// 高带宽阈值 (Mbps)
    pub high_bandwidth_threshold: f64,
}

impl Default for AbreConfig {
    fn default() -> Self {
        Self {
            min_bitrate: 500,      // 500 kbps
            max_bitrate: 8000,     // 8 Mbps
            initial_bitrate: 2000, // 2 Mbps
            history_size: 10,
            adjustment_step: 0.1,  // 10% adjustment
            high_latency_threshold: 100.0,  // 100ms
            high_packet_loss_threshold: 0.05, // 5%
            low_bandwidth_threshold: 1.0,    // 1 Mbps
            high_bandwidth_threshold: 10.0,  // 10 Mbps
        }
    }
}

/// 基于规则的 ABR 控制器
///
/// 使用启发式规则引擎根据网络状态选择最优码率
pub struct RuleBasedAbreController {
    config: AbreConfig,
    current_bitrate: u32,
    bandwidth_history: VecDeque<f64>,
    latency_history: VecDeque<f64>,
    packet_loss_history: VecDeque<f64>,
    last_update: Instant,
}

impl RuleBasedAbreController {
    /// 创建新的 ABR 控制器
    pub fn new(config: AbreConfig) -> Self {
        let current_bitrate = config.initial_bitrate;
        let history_size = config.history_size;

        tracing::info!(
            "初始化 ABR 控制器: 初始码率 {}kbps, 范围: {}-{}kbps",
            current_bitrate, config.min_bitrate, config.max_bitrate
        );

        Self {
            config,
            current_bitrate,
            bandwidth_history: VecDeque::with_capacity(history_size),
            latency_history: VecDeque::with_capacity(history_size),
            packet_loss_history: VecDeque::with_capacity(history_size),
            last_update: Instant::now(),
        }
    }

    /// 更新网络状态并返回建议的码率
    pub fn update(&mut self, state: NetworkState) -> u32 {
        // 记录历史数据
        if self.bandwidth_history.len() >= self.config.history_size {
            self.bandwidth_history.pop_front();
            self.latency_history.pop_front();
            self.packet_loss_history.pop_front();
        }

        self.bandwidth_history.push_back(state.bandwidth_mbps);
        self.latency_history.push_back(state.latency_ms);
        self.packet_loss_history.push_back(state.packet_loss);

        // 应用规则引擎
        let new_bitrate = self.apply_rules(state);

        // 更新当前码率
        if new_bitrate != self.current_bitrate {
            tracing::debug!(
                "码率调整: {}kbps -> {}kbps (延迟: {:.1}ms, 丢包: {:.1}%, 带宽: {:.1}Mbps)",
                self.current_bitrate,
                new_bitrate,
                state.latency_ms,
                state.packet_loss * 100.0,
                state.bandwidth_mbps
            );
            self.current_bitrate = new_bitrate;
        }

        self.last_update = Instant::now();
        self.current_bitrate
    }

    /// 获取当前码率
    pub fn current_bitrate(&self) -> u32 {
        self.current_bitrate
    }

    /// 应用启发式规则
    fn apply_rules(&self, state: NetworkState) -> u32 {
        let mut bitrate = self.current_bitrate as f64;
        let mut adjustment_reason = String::new();

        // 规则 1: 高延迟 → 降低码率
        if state.latency_ms > self.config.high_latency_threshold {
            let factor = 1.0 - self.config.adjustment_step;
            bitrate *= factor;
            adjustment_reason = format!("高延迟 ({:.1}ms)", state.latency_ms);
        }
        // 规则 2: 低延迟 → 可以提高码率
        else if state.latency_ms < self.config.high_latency_threshold * 0.5 {
            let factor = 1.0 + self.config.adjustment_step * 0.5;
            bitrate *= factor;
            adjustment_reason = format!("低延迟 ({:.1}ms)", state.latency_ms);
        }

        // 规则 3: 高丢包率 → 大幅降低码率
        if state.packet_loss > self.config.high_packet_loss_threshold {
            let factor = 1.0 - (self.config.adjustment_step * 2.0);
            bitrate *= factor;
            if !adjustment_reason.is_empty() {
                adjustment_reason.push_str(&format!(", 高丢包 ({:.1}%)", state.packet_loss * 100.0));
            } else {
                adjustment_reason = format!("高丢包 ({:.1}%)", state.packet_loss * 100.0);
            }
        }

        // 规则 4: 带宽不足 → 降低码率
        if state.bandwidth_mbps < self.config.low_bandwidth_threshold {
            // 码率不能超过可用带宽的 80%
            let max_safe_bitrate = (state.bandwidth_mbps * 1000.0 * 0.8) as u32;
            bitrate = bitrate.min(max_safe_bitrate as f64);
            if !adjustment_reason.is_empty() {
                adjustment_reason.push_str(&format!(", 低带宽 ({:.2}Mbps)", state.bandwidth_mbps));
            } else {
                adjustment_reason = format!("低带宽 ({:.2}Mbps)", state.bandwidth_mbps);
            }
        }
        // 规则 5: 充足带宽 → 提升到高质量
        else if state.bandwidth_mbps > self.config.high_bandwidth_threshold {
            let factor = 1.0 + self.config.adjustment_step;
            bitrate *= factor;
            if !adjustment_reason.is_empty() {
                adjustment_reason.push_str(&format!(", 高带宽 ({:.1}Mbps)", state.bandwidth_mbps));
            } else {
                adjustment_reason = format!("高带宽 ({:.1}Mbps)", state.bandwidth_mbps);
            }
        }

        // 规则 6: 带宽波动大 → 选择保守码率
        if self.bandwidth_history.len() >= 3 {
            let bandwidth_variance = self.calculate_variance(&self.bandwidth_history);
            let bandwidth_mean = self.bandwidth_history.iter().sum::<f64>() / self.bandwidth_history.len() as f64;
            let coefficient_of_variation = if bandwidth_mean > 0.0 {
                bandwidth_variance.sqrt() / bandwidth_mean
            } else {
                0.0
            };

            // 如果变异系数 > 30%，选择保守码率
            if coefficient_of_variation > 0.3 {
                let conservative_factor = 0.8;
                bitrate *= conservative_factor;
                if !adjustment_reason.is_empty() {
                    adjustment_reason.push_str(&format!(", 带宽波动 (CV: {:.0}%)", coefficient_of_variation * 100.0));
                } else {
                    adjustment_reason = format!("带宽波动 (CV: {:.0}%)", coefficient_of_variation * 100.0);
                }
            }
        }

        if !adjustment_reason.is_empty() {
            tracing::trace!("码率调整原因: {}", adjustment_reason);
        }

        // 限制在最小/最大范围内
        bitrate = bitrate.max(self.config.min_bitrate as f64);
        bitrate = bitrate.min(self.config.max_bitrate as f64);

        bitrate as u32
    }

    /// 计算方差
    fn calculate_variance(&self, data: &VecDeque<f64>) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let mean = data.iter().sum::<f64>() / data.len() as f64;
        let variance = data.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / data.len() as f64;

        variance
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> AbreStats {
        let (bandwidth_mean, latency_mean, packet_loss_mean) = if !self.bandwidth_history.is_empty() {
            (
                self.bandwidth_history.iter().sum::<f64>() / self.bandwidth_history.len() as f64,
                self.latency_history.iter().sum::<f64>() / self.latency_history.len() as f64,
                self.packet_loss_history.iter().sum::<f64>() / self.packet_loss_history.len() as f64,
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        AbreStats {
            current_bitrate: self.current_bitrate,
            bandwidth_mean,
            latency_mean,
            packet_loss_mean,
            time_since_last_update: self.last_update.elapsed(),
        }
    }
}

/// ABR 统计信息
#[derive(Debug, Clone)]
pub struct AbreStats {
    pub current_bitrate: u32,
    pub bandwidth_mean: f64,
    pub latency_mean: f64,
    pub packet_loss_mean: f64,
    pub time_since_last_update: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abre_controller_creation() {
        let config = AbreConfig::default();
        let controller = RuleBasedAbreController::new(config);
        assert_eq!(controller.current_bitrate(), 2000);
    }

    #[test]
    fn test_high_latency_reduce_bitrate() {
        let config = AbreConfig::default();
        let mut controller = RuleBasedAbreController::new(config);

        // 高延迟状态
        let state = NetworkState {
            bandwidth_mbps: 10.0,
            latency_ms: 150.0,
            packet_loss: 0.01,
            jitter_ms: 10.0,
        };

        let new_bitrate = controller.update(state);
        assert!(new_bitrate < 2000, "高延迟应降低码率");
    }

    #[test]
    fn test_high_packet_loss_reduce_bitrate() {
        let config = AbreConfig::default();
        let mut controller = RuleBasedAbreController::new(config);

        // 高丢包状态
        let state = NetworkState {
            bandwidth_mbps: 10.0,
            latency_ms: 50.0,
            packet_loss: 0.10, // 10%
            jitter_ms: 10.0,
        };

        let new_bitrate = controller.update(state);
        assert!(new_bitrate < 2000, "高丢包应降低码率");
    }

    #[test]
    fn test_high_bandwidth_increase_bitrate() {
        let config = AbreConfig::default();
        let mut controller = RuleBasedAbreController::new(config);

        // 高带宽状态
        let state = NetworkState {
            bandwidth_mbps: 20.0,
            latency_ms: 30.0,
            packet_loss: 0.001,
            jitter_ms: 5.0,
        };

        let new_bitrate = controller.update(state);
        assert!(new_bitrate > 2000, "高带宽应提高码率");
    }

    #[test]
    fn test_bitrate_clamping() {
        let config = AbreConfig {
            min_bitrate: 1000,
            max_bitrate: 5000,
            ..Default::default()
        };
        let mut controller = RuleBasedAbreController::new(config);

        // 极低带宽
        let state = NetworkState {
            bandwidth_mbps: 0.5,
            latency_ms: 50.0,
            packet_loss: 0.01,
            jitter_ms: 10.0,
        };
        let bitrate = controller.update(state);
        assert!(bitrate >= 1000, "码率不应低于最小值");

        // 极高带宽
        let state = NetworkState {
            bandwidth_mbps: 100.0,
            latency_ms: 10.0,
            packet_loss: 0.0,
            jitter_ms: 1.0,
        };
        let bitrate = controller.update(state);
        assert!(bitrate <= 5000, "码率不应超过最大值");
    }

    #[test]
    fn test_variance_calculation() {
        let config = AbreConfig::default();
        let controller = RuleBasedAbreController::new(config);

        let data: VecDeque<f64> = vec![10.0, 12.0, 8.0, 11.0, 9.0].into_iter().collect();
        let variance = controller.calculate_variance(&data);

        // 方差应该 > 0
        assert!(variance > 0.0);
    }
}
