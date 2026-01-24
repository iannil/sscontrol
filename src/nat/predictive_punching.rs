//! 预测性端口攻击 (Predictive Port Punching)
//!
//! 通过分析 NAT 的端口分配模式，预测下一个外部端口，提高对称 NAT 穿透率

// 预测性打洞模块尚未激活，标记为允许死代码和未使用导入
#![allow(dead_code, unused_imports)]

use crate::nat::detector::{NatBehavior, NatType, PortAllocationPattern};
use anyhow::Result;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::Duration;

/// 端口预测结果
#[derive(Debug, Clone)]
pub struct PortPrediction {
    /// 预测的端口列表 (按概率排序)
    pub predicted_ports: Vec<u16>,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f64,
}

/// 预测性打洞
pub struct PredictivePunching {
    /// 端口分配历史
    port_history: VecDeque<(u16, SocketAddr)>,
    /// 最大历史记录数
    max_history: usize,
}

impl PredictivePunching {
    /// 创建新的预测性打洞实例
    pub fn new() -> Self {
        Self {
            port_history: VecDeque::with_capacity(10),
            max_history: 10,
        }
    }

    /// 添加端口映射观察
    pub fn add_observation(&mut self, external_port: u16, target: SocketAddr) {
        if self.port_history.len() >= self.max_history {
            self.port_history.pop_front();
        }
        self.port_history.push_back((external_port, target));
    }

    /// 预测下一个外部端口
    ///
    /// # 参数
    /// * `local_port` - 本地端口
    /// * `target` - 目标地址
    /// * `nat_behavior` - NAT 行为分析结果
    /// * `num_predictions` - 预测数量
    pub fn predict_next_ports(
        &self,
        local_port: u16,
        target: SocketAddr,
        nat_behavior: &NatBehavior,
        num_predictions: usize,
    ) -> PortPrediction {
        if self.port_history.is_empty() {
            // 没有历史数据，无法预测
            return PortPrediction {
                predicted_ports: vec![local_port],
                confidence: 0.0,
            };
        }

        let (predicted_ports, confidence) = match nat_behavior.port_allocation_pattern {
            PortAllocationPattern::Fixed => {
                // 固定端口: 所有目标都映射到同一个外部端口
                (vec![self.port_history[0].0], 1.0)
            }

            PortAllocationPattern::SequentialIncrement(step) => {
                // 顺序递增: N, N+step, N+2*step, ...
                let last_port = self.port_history.back().unwrap().0;
                let ports: Vec<u16> = (0..num_predictions as u16)
                    .map(|i| last_port.wrapping_add(i * step))
                    .collect();
                (ports, 0.85)
            }

            PortAllocationPattern::Random => {
                // 随机端口: 尝试常见范围
                self.predict_random_range(nat_behavior, num_predictions)
            }

            PortAllocationPattern::HashBased => {
                // 基于哈希: 端口 = hash(local_port, target_ip, target_port)
                let predicted = self.hash_based_prediction(local_port, target, num_predictions);
                (predicted, 0.70)
            }
        };

        PortPrediction {
            predicted_ports,
            confidence,
        }
    }

    /// 随机端口范围预测
    ///
    /// 大多数 NAT 分配的端口在特定范围内
    fn predict_random_range(
        &self,
        nat_behavior: &NatBehavior,
        num_predictions: usize,
    ) -> (Vec<u16>, f64) {
        // 获取最近观察到的基础端口
        let base_port = nat_behavior.external_port.unwrap_or(self.port_history[0].0);

        // 常见 NAT 端口范围: 1024-65535
        // 但大多数 NAT 使用 20000-60000 范围
        let min_port = base_port.saturating_sub(5000);
        let max_port = base_port.saturating_add(5000);

        // 生成预测端口 (优先使用靠近基础端口的)
        let mut ports: Vec<u16> = (0..num_predictions as u16)
            .map(|i| {
                // 螺旋式搜索: 0, +1, -1, +2, -2, +3, -3, ...
                if i % 2 == 0 {
                    base_port.wrapping_add(i / 2)
                } else {
                    base_port.wrapping_sub((i + 1) / 2)
                }
            })
            .filter(|&p| p >= min_port && p <= max_port)
            .collect();

        // 去重
        ports.sort();
        ports.dedup();
        ports.truncate(num_predictions);

        (ports, 0.60)
    }

    /// 基于哈希的端口预测
    ///
    /// 某些 NAT 使用本地端口和目标地址的哈希来分配外部端口
    fn hash_based_prediction(
        &self,
        local_port: u16,
        target: SocketAddr,
        num_predictions: usize,
    ) -> Vec<u16> {
        // 简单哈希函数
        let hash = |port: u16, target: &SocketAddr| -> u16 {
            let mut hash = port as u32;

            // 混入目标 IP
            match target.ip() {
                std::net::IpAddr::V4(ipv4) => {
                    hash ^= u32::from(ipv4);
                }
                std::net::IpAddr::V6(ipv6) => {
                    let octets = ipv6.octets();
                    hash ^= u32::from_be_bytes([octets[0], octets[1], octets[2], octets[3]]);
                    hash ^= u32::from_be_bytes([octets[4], octets[5], octets[6], octets[7]]);
                    hash ^= u32::from_be_bytes([octets[8], octets[9], octets[10], octets[11]]);
                    hash ^= u32::from_be_bytes([octets[12], octets[13], octets[14], octets[15]]);
                }
            }

            // 混入目标端口
            hash ^= target.port() as u32;

            // 简单的混合函数
            hash = hash.wrapping_mul(0x517cc1b7);
            hash ^= hash >> 16;

            // 映射到端口范围
            ((hash % 50000) + 1024) as u16
        };

        let base_port = hash(local_port, &target);

        // 生成多个预测 (使用不同的混合方式)
        let mut ports = vec![base_port];

        let variants = [
            0x9e3779b9u32, // 黄金比例
            0x85ebca6bu32,
            0xc2b2ae3du32,
        ];

        for variant in variants {
            let port = ((base_port as u32).wrapping_mul(variant) % 50000 + 1024) as u16;
            ports.push(port);
        }

        ports.truncate(num_predictions);
        ports
    }

    /// 执行并行打洞
    ///
    /// 向多个预测端口同时发送打洞包
    pub async fn parallel_punch(
        &self,
        socket: &tokio::net::UdpSocket,
        target: &SocketAddr,
        predicted_ports: &[u16],
        punch_timeout: Duration,
    ) -> Result<bool> {
        tracing::info!(
            "开始并行打洞: 目标={}, 预测端口数={}",
            target,
            predicted_ports.len()
        );

        // 向所有预测端口发送打洞包
        let punch_data = b"PUNCH";
        for &port in predicted_ports {
            let punch_addr = SocketAddr::new(target.ip(), port);
            if let Err(e) = socket.send_to(punch_data, punch_addr).await {
                tracing::debug!("发送打洞包到 {} 失败: {}", punch_addr, e);
            }
        }

        // 等待响应
        let mut buf = [0u8; 1024];
        let start = std::time::Instant::now();

        loop {
            let elapsed = start.elapsed();
            if elapsed >= punch_timeout {
                break;
            }

            let remaining = punch_timeout - elapsed;

            // 使用 tokio::time::timeout 实现超时
            match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
                Ok(Ok((size, from))) => {
                    tracing::info!("收到打洞响应: {} ({} 字节)", from, size);
                    return Ok(true);
                }
                Ok(Err(e)) => {
                    tracing::debug!("打洞接收错误: {}", e);
                }
                Err(_) => {
                    // 超时
                    break;
                }
            }
        }

        Ok(false)
    }

    /// 清除历史记录
    pub fn clear_history(&mut self) {
        self.port_history.clear();
    }

    /// 获取历史记录数
    pub fn history_len(&self) -> usize {
        self.port_history.len()
    }
}

impl Default for PredictivePunching {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_observation() {
        let mut punching = PredictivePunching::new();
        punching.add_observation(12345, "1.2.3.4:80".parse().unwrap());
        assert_eq!(punching.history_len(), 1);
    }

    #[test]
    fn test_clear_history() {
        let mut punching = PredictivePunching::new();
        punching.add_observation(12345, "1.2.3.4:80".parse().unwrap());
        punching.clear_history();
        assert_eq!(punching.history_len(), 0);
    }

    #[test]
    fn test_sequential_prediction() {
        let mut punching = PredictivePunching::new();
        punching.add_observation(20000, "1.2.3.4:80".parse().unwrap());
        punching.add_observation(20001, "1.2.3.5:80".parse().unwrap());

        let behavior = NatBehavior {
            nat_type: NatType::Symmetric,
            external_ip: Some("5.6.7.8".to_string()),
            external_port: Some(20002),
            port_allocation_pattern: PortAllocationPattern::SequentialIncrement(1),
            hairpinning: false,
        };

        let prediction = punching.predict_next_ports(30000, "1.2.3.6:80".parse().unwrap(), &behavior, 5);
        assert!(!prediction.predicted_ports.is_empty());
        assert!(prediction.confidence > 0.0);
    }
}
