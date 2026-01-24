//! NAT 类型检测模块
//!
//! 无需 STUN 服务器，主动探测 NAT 行为

use crate::nat::NatConfig;
use anyhow::{anyhow, Result};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// NAT 类型分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatType {
    /// 无 NAT，公网 IP
    Open,
    /// Full Cone NAT - 任何外部主机都可以通过映射地址通信
    FullCone,
    /// Restricted Cone NAT - 只有收到过数据包的外部 IP 可以通信
    RestrictedCone,
    /// Port-Restricted Cone NAT - 只有收到过数据包的 IP:Port 可以通信
    PortRestrictedCone,
    /// Symmetric NAT - 每个目标地址都有不同的端口映射
    Symmetric,
    /// 无法确定或被防火墙阻止
    Blocked,
}

/// NAT 行为分析结果
#[derive(Debug, Clone)]
pub struct NatBehavior {
    pub nat_type: NatType,
    pub external_ip: Option<String>,
    pub external_port: Option<u16>,
    pub port_allocation_pattern: PortAllocationPattern,
    pub hairpinning: bool, // 是否支持 hairpinning
}

/// 端口分配模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortAllocationPattern {
    /// 固定端口 (1:1 NAT)
    Fixed,
    /// 顺序递增 (N, N+1, N+2...)
    SequentialIncrement(u16), // 步长
    /// 随机端口
    Random,
    /// 基于目标地址的哈希
    HashBased,
}

/// NAT 检测器
pub struct NatDetector {
    config: NatConfig,
    probe_endpoints: Vec<SocketAddr>,
}

impl NatDetector {
    /// 创建新的 NAT 检测器
    pub fn new(config: NatConfig) -> Self {
        // 使用公共 HTTP/HTTPS 端点作为探测目标
        // 这些端口通常不被防火墙阻止
        let probe_endpoints = vec![
            "1.1.1.1:80".parse().unwrap(),     // Cloudflare DNS
            "8.8.8.8:80".parse().unwrap(),     // Google DNS
            "1.0.0.1:443".parse().unwrap(),    // Cloudflare DNS
        ];

        Self {
            config,
            probe_endpoints,
        }
    }

    /// 使用默认配置创建检测器
    pub fn with_default_config() -> Self {
        Self::new(NatConfig::default())
    }

    /// 检测 NAT 类型
    ///
    /// 通过多次探测不同的端点，分析端口分配模式来确定 NAT 类型
    pub async fn detect_nat_type(&self) -> Result<NatBehavior> {
        tracing::info!("开始 NAT 类型检测...");

        // 绑定本地 UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_millis(2000)))?;

        let local_addr = socket.local_addr()?;
        tracing::debug!("本地地址: {}", local_addr);

        // 第一步：检测是否有 NAT
        let (external_ip1, external_port1) = self
            .probe_external_addr(&socket, &self.probe_endpoints[0])
            .await?;

        if external_ip1.is_none() {
            tracing::warn!("无法获取外部地址，可能被防火墙阻止");
            return Ok(NatBehavior {
                nat_type: NatType::Blocked,
                external_ip: None,
                external_port: None,
                port_allocation_pattern: PortAllocationPattern::Fixed,
                hairpinning: false,
            });
        }

        let external_ip1 = external_ip1.unwrap();
        let external_port1 = external_port1.unwrap();

        // 检查是否有 NAT (比较本地 IP 和外部 IP)
        let has_nat = self.has_nat(&local_addr, &external_ip1);

        if !has_nat {
            tracing::info!("未检测到 NAT (公网 IP)");
            return Ok(NatBehavior {
                nat_type: NatType::Open,
                external_ip: Some(external_ip1),
                external_port: Some(external_port1),
                port_allocation_pattern: PortAllocationPattern::Fixed,
                hairpinning: false,
            });
        }

        // 第二步：检测端口分配模式
        let mut port_mappings = vec![(external_port1, self.probe_endpoints[0])];

        // 向多个端点探测，收集端口映射
        for endpoint in &self.probe_endpoints[1..] {
            let (_, ext_port) = self.probe_external_addr(&socket, endpoint).await?;
            if let Some(port) = ext_port {
                port_mappings.push((port, *endpoint));
            }
        }

        // 分析端口分配模式
        let allocation_pattern = self.analyze_port_allocation(&port_mappings);

        // 第三步：确定 NAT 类型
        let nat_type = match allocation_pattern {
            PortAllocationPattern::Fixed => NatType::FullCone,
            PortAllocationPattern::SequentialIncrement(_) => {
                // 检查增量是否一致
                if port_mappings.windows(2).all(|w| w[1].0 == w[0].0 + 1) {
                    NatType::PortRestrictedCone
                } else {
                    NatType::Symmetric
                }
            }
            PortAllocationPattern::Random | PortAllocationPattern::HashBased => {
                NatType::Symmetric
            }
        };

        tracing::info!("NAT 检测完成: {:?}", nat_type);
        tracing::info!("外部地址: {}:{}", external_ip1, external_port1);
        tracing::info!("端口分配模式: {:?}", allocation_pattern);

        Ok(NatBehavior {
            nat_type,
            external_ip: Some(external_ip1),
            external_port: Some(external_port1),
            port_allocation_pattern: allocation_pattern,
            hairpinning: false, // TODO: 实现 hairpinning 检测
        })
    }

    /// 探测外部地址
    ///
    /// 向目标端点发送数据包，观察外部 IP 和端口
    async fn probe_external_addr(
        &self,
        socket: &UdpSocket,
        target: &SocketAddr,
    ) -> Result<(Option<String>, Option<u16>)> {
        // 发送探测包
        let probe_data = b"NAT_PROBE";
        socket.send_to(probe_data, target)?;

        // 等待响应 (使用超时)
        let mut buf = [0u8; 1024];
        match socket.recv_from(&mut buf) {
            Ok((size, from)) => {
                // 解析响应
                let response = String::from_utf8_lossy(&buf[..size]);
                tracing::trace!("收到响应: {} from {}", response, from);

                // 从响应中提取外部地址
                // 注意: 这里需要探测服务器返回外部地址
                // 实际实现中需要专用探测协议
                // 这里简化处理，使用 from 地址
                Ok((Some(from.ip().to_string()), Some(from.port())))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // 超时
                Ok((None, None))
            }
            Err(e) => Err(anyhow!("探测失败: {}", e)),
        }
    }

    /// 检查是否有 NAT
    fn has_nat(&self, local_addr: &SocketAddr, external_ip: &str) -> bool {
        let local_ip = local_addr.ip().to_string();
        local_ip != external_ip
    }

    /// 分析端口分配模式
    fn analyze_port_allocation(&self, mappings: &[(u16, SocketAddr)]) -> PortAllocationPattern {
        if mappings.len() < 2 {
            return PortAllocationPattern::Fixed;
        }

        let ports: Vec<u16> = mappings.iter().map(|(p, _)| *p).collect();

        // 检查是否固定端口
        if ports.iter().all(|&p| p == ports[0]) {
            return PortAllocationPattern::Fixed;
        }

        // 检查是否顺序递增
        let increments: Vec<i32> = ports
            .windows(2)
            .map(|w| w[1] as i32 - w[0] as i32)
            .collect();

        if increments.iter().all(|&inc| inc == increments[0]) && increments[0] > 0 {
            return PortAllocationPattern::SequentialIncrement(increments[0] as u16);
        }

        // 检查是否随机或基于哈希
        // 简单启发式: 如果增量差异很大，认为是随机的
        let variance = self.calculate_variance(&increments);
        if variance > 100.0 {
            PortAllocationPattern::Random
        } else {
            PortAllocationPattern::HashBased
        }
    }

    /// 计算方差
    fn calculate_variance(&self, values: &[i32]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let mean = values.iter().map(|&x| x as f64).sum::<f64>() / values.len() as f64;
        let variance = values
            .iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;

        variance
    }

    /// 评估 NAT 穿透难度
    pub fn assess_difficulty(&self, behavior: &NatBehavior) -> TraversalDifficulty {
        match behavior.nat_type {
            NatType::Open => TraversalDifficulty::Easy,
            NatType::FullCone => TraversalDifficulty::Easy,
            NatType::RestrictedCone => TraversalDifficulty::Medium,
            NatType::PortRestrictedCone => TraversalDifficulty::Medium,
            NatType::Symmetric => {
                match behavior.port_allocation_pattern {
                    PortAllocationPattern::SequentialIncrement(step) if step <= 10 => {
                        TraversalDifficulty::Medium
                    }
                    _ => TraversalDifficulty::Hard,
                }
            }
            NatType::Blocked => TraversalDifficulty::Impossible,
        }
    }
}

/// NAT 穿透难度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDifficulty {
    /// 容易 (直接连接)
    Easy,
    /// 中等 (需要预测性打洞)
    Medium,
    /// 困难 (需要多次尝试)
    Hard,
    /// 不可能 (需要中继)
    Impossible,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nat_detection() {
        let detector = NatDetector::with_default_config();
        let behavior = detector.detect_nat_type().await;

        // NAT detection may fail in test environments (no network, firewall, etc.)
        // Just verify the detector can be created and run
        match behavior {
            Ok(b) => {
                println!("NAT Type: {:?}", b.nat_type);
                println!("External IP: {:?}", b.external_ip);
                println!("Port Pattern: {:?}", b.port_allocation_pattern);
            }
            Err(e) => {
                println!("NAT detection failed (expected in test environments): {}", e);
                // This is acceptable for unit tests
            }
        }
    }

    #[test]
    fn test_variance_calculation() {
        let detector = NatDetector::with_default_config();
        // Test with uniform values (zero variance)
        let uniform = vec![5, 5, 5, 5, 5];
        let variance = detector.calculate_variance(&uniform);
        assert!(variance < 0.01); // Should be close to 0

        // Test with low variance values
        let low_variance = vec![10, 11, 10, 11, 10];
        let variance = detector.calculate_variance(&low_variance);
        assert!(variance < 1.0); // Low variance
    }
}
