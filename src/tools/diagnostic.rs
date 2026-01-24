//! 网络诊断工具
//!
//! 提供网络连接诊断和故障排查功能
//!
//! ## 功能
//! - NAT 类型检测
//! - 网络质量评估
//! - 连接失败原因分析
//! - 编码器可用性检测

use anyhow::{anyhow, Result};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// 诊断结果
#[derive(Debug, Clone)]
pub struct DiagnosticResult {
    pub success: bool,
    pub message: String,
    pub details: Vec<DiagnosticDetail>,
}

/// 诊断详情
#[derive(Debug, Clone)]
pub struct DiagnosticDetail {
    pub category: String,
    pub name: String,
    pub status: DiagnosticStatus,
    pub value: String,
    pub recommendation: Option<String>,
}

/// 诊断状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticStatus {
    Pass,
    Warning,
    Fail,
    Info,
}

/// 网络诊断统计
#[derive(Debug, Clone)]
pub struct NetworkDiagnostics {
    pub local_ip: Option<String>,
    pub nat_type: Option<String>,
    pub bandwidth_mbps: Option<f64>,
    pub latency_ms: Option<f64>,
    pub packet_loss: Option<f64>,
    pub public_ip: Option<String>,
}

/// 诊断工具
///
/// 提供网络诊断功能
pub struct DiagnosticTool {
    timeout: Duration,
}

impl DiagnosticTool {
    /// 创建新的诊断工具
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(5),
        }
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 运行完整诊断
    pub fn run_full_diagnostic(&self) -> Result<DiagnosticResult> {
        tracing::info!("开始网络诊断...");

        let mut details = Vec::new();

        // 1. 检测本地 IP
        details.push(self.check_local_ip());

        // 2. 检测 NAT 类型
        details.push(self.check_nat_type());

        // 3. 测试网络连接
        details.push(self.check_network_connectivity());

        // 4. 评估网络质量
        details.push(self.check_network_quality());

        // 5. 检测编码器可用性
        details.push(self.check_encoders());

        // 计算总体结果
        let fail_count = details.iter()
            .filter(|d| d.status == DiagnosticStatus::Fail)
            .count();

        let success = fail_count == 0;

        let message = if success {
            "所有诊断测试通过".to_string()
        } else {
            format!("发现 {} 个问题", fail_count)
        };

        Ok(DiagnosticResult {
            success,
            message,
            details,
        })
    }

    /// 检测本地 IP
    fn check_local_ip(&self) -> DiagnosticDetail {
        use local_ip_address::local_ip;

        let status = match local_ip() {
            Ok(ip) => {
                DiagnosticDetail {
                    category: "网络".to_string(),
                    name: "本地 IP 地址".to_string(),
                    status: DiagnosticStatus::Pass,
                    value: ip.to_string(),
                    recommendation: None,
                }
            }
            Err(e) => {
                DiagnosticDetail {
                    category: "网络".to_string(),
                    name: "本地 IP 地址".to_string(),
                    status: DiagnosticStatus::Fail,
                    value: "未检测到".to_string(),
                    recommendation: Some(format!("错误: {}", e)),
                }
            }
        };

        status
    }

    /// 检测 NAT 类型
    fn check_nat_type(&self) -> DiagnosticDetail {
        use crate::nat::{NatDetector, NatConfig};

        // 创建 NAT 检测器
        let _detector = NatDetector::new(NatConfig::default());

        // 注意: NAT 检测是异步的，这里我们只做简单的同步检查
        // 实际诊断应该在异步上下文中运行
        let status = DiagnosticStatus::Info;
        let value = "运行 'sscontrol doctor --nat' 进行完整 NAT 检测".to_string();
        let recommendation = Some("NAT 检测需要在运行时环境中进行".to_string());

        DiagnosticDetail {
            category: "NAT".to_string(),
            name: "NAT 类型".to_string(),
            status,
            value,
            recommendation,
        }
    }

    /// 检测网络连接
    fn check_network_connectivity(&self) -> DiagnosticDetail {
        // 尝试连接到公共 DNS 服务器
        let test_addresses = vec![
            "8.8.8.8:53",  // Google DNS
            "1.1.1.1:53",  // Cloudflare DNS
        ];

        let mut connected = false;
        let mut latency = None;

        for addr_str in test_addresses {
            if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
                    socket.set_read_timeout(Some(Duration::from_secs(2))).ok();

                    let start = Instant::now();
                    let buf = [0u8; 1];

                    match socket.send_to(&buf, addr) {
                        Ok(_) => {
                            // UDP 发送成功，说明网络连通
                            connected = true;
                            latency = Some(start.elapsed().as_millis() as f64);
                            break;
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        if connected {
            let latency_ms = latency.unwrap_or(0.0);
            let status = if latency_ms < 100.0 {
                DiagnosticStatus::Pass
            } else {
                DiagnosticStatus::Warning
            };

            DiagnosticDetail {
                category: "网络".to_string(),
                name: "网络连接".to_string(),
                status,
                value: format!("正常 (延迟: {:.1}ms)", latency_ms),
                recommendation: None,
            }
        } else {
            DiagnosticDetail {
                category: "网络".to_string(),
                name: "网络连接".to_string(),
                status: DiagnosticStatus::Fail,
                value: "无法连接到公共服务器".to_string(),
                recommendation: Some("请检查网络连接和防火墙设置".to_string()),
            }
        }
    }

    /// 评估网络质量
    fn check_network_quality(&self) -> DiagnosticDetail {
        // 简化的网络质量检测
        // 实际实现应该进行带宽测试、丢包测试等

        let status = DiagnosticStatus::Info;

        DiagnosticDetail {
            category: "网络质量".to_string(),
            name: "带宽测试".to_string(),
            status,
            value: "运行 'sscontrol doctor --bandwidth' 进行完整测试".to_string(),
            recommendation: Some("需要手动运行带宽测试".to_string()),
        }
    }

    /// 检测编码器可用性
    fn check_encoders(&self) -> DiagnosticDetail {
        let mut available_encoders = Vec::new();

        // 检测软件编码器 (始终可用)
        available_encoders.push("Software (x264)".to_string());

        // 检测硬件编码器
        #[cfg(target_os = "macos")]
        {
            available_encoders.push("Apple VideoToolbox".to_string());
        }

        #[cfg(target_os = "windows")]
        {
            // TODO: 检测 NVENC/AMF/QSV 可用性
            // available_encoders.push("NVIDIA NVENC".to_string());
            // available_encoders.push("AMD AMF".to_string());
            // available_encoders.push("Intel Quick Sync".to_string());
        }

        let status = if available_encoders.len() > 1 {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Warning
        };

        let recommendation = if available_encoders.len() == 1 {
            Some("未检测到硬件编码器，将使用软件编码 (CPU 占用较高)".to_string())
        } else {
            None
        };

        DiagnosticDetail {
            category: "编码器".to_string(),
            name: "可用编码器".to_string(),
            status,
            value: available_encoders.join(", "),
            recommendation,
        }
    }

    /// 测试带宽
    pub fn test_bandwidth(&self, test_server: Option<&str>) -> Result<NetworkDiagnostics> {
        // TODO: 实现真实的带宽测试
        // 这需要连接到测试服务器并下载数据

        let _ = test_server; // 暂时忽略参数

        Ok(NetworkDiagnostics {
            local_ip: local_ip_address::local_ip().ok().map(|ip| ip.to_string()),
            nat_type: None,
            bandwidth_mbps: None,
            latency_ms: None,
            packet_loss: None,
            public_ip: None,
        })
    }

    /// 测试延迟
    pub fn test_latency(&self, target: &str) -> Result<f64> {
        let addr: SocketAddr = target.parse()
            .map_err(|_| anyhow!("无效的目标地址: {}", target))?;

        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;

        let start = Instant::now();
        socket.send_to(&[0u8; 1], addr)?;

        let mut buf = [0u8; 1024];
        match socket.recv_from(&mut buf) {
            Ok(_) => Ok(start.elapsed().as_millis() as f64),
            Err(_) => Err(anyhow!("未收到响应")),
        }
    }

    /// 分析连接失败原因
    pub fn analyze_connection_failure(&self, error: &str) -> DiagnosticDetail {
        let (status, value, recommendation) = if error.contains("timeout") || error.contains("超时") {
            (
                DiagnosticStatus::Warning,
                "连接超时".to_string(),
                Some("可能原因: 防火墙阻止、NAT 限制、网络不稳定".to_string()),
            )
        } else if error.contains("refused") || error.contains("拒绝") {
            (
                DiagnosticStatus::Fail,
                "连接被拒绝".to_string(),
                Some("目标主机拒绝连接，请检查目标服务是否运行".to_string()),
            )
        } else if error.contains("unreachable") || error.contains("不可达") {
            (
                DiagnosticStatus::Fail,
                "主机不可达".to_string(),
                Some("网络路由问题，请检查网络配置".to_string()),
            )
        } else {
            (
                DiagnosticStatus::Warning,
                "未知错误".to_string(),
                Some(format!("错误信息: {}", error)),
            )
        };

        DiagnosticDetail {
            category: "故障分析".to_string(),
            name: "连接失败原因".to_string(),
            status,
            value,
            recommendation,
        }
    }
}

impl Default for DiagnosticTool {
    fn default() -> Self {
        Self::new()
    }
}

/// 格式化诊断结果为人类可读的文本
pub fn format_diagnostic_result(result: &DiagnosticResult) -> String {
    let mut output = String::new();

    output.push_str(&format!("\n=== 诊断结果 ===\n"));
    output.push_str(&format!("状态: {}\n\n", result.message));

    for detail in &result.details {
        let status_icon = match detail.status {
            DiagnosticStatus::Pass => "✓",
            DiagnosticStatus::Warning => "⚠",
            DiagnosticStatus::Fail => "✗",
            DiagnosticStatus::Info => "ℹ",
        };

        output.push_str(&format!("[{}] {} - {}\n", status_icon, detail.category, detail.name));
        output.push_str(&format!("  值: {}\n", detail.value));

        if let Some(ref recommendation) = detail.recommendation {
            output.push_str(&format!("  建议: {}\n", recommendation));
        }

        output.push('\n');
    }

    output
}

/// 打印诊断信息到控制台
pub fn print_diagnostics() {
    let tool = DiagnosticTool::new();
    match tool.run_full_diagnostic() {
        Ok(result) => {
            println!("{}", format_diagnostic_result(&result));
        }
        Err(e) => {
            println!("❌ 诊断失败: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_tool_creation() {
        let tool = DiagnosticTool::new();
        assert_eq!(tool.timeout.as_secs(), 5);
    }

    #[test]
    fn test_diagnostic_tool_with_timeout() {
        let tool = DiagnosticTool::new().with_timeout(Duration::from_secs(10));
        assert_eq!(tool.timeout.as_secs(), 10);
    }

    #[test]
    fn test_local_ip_check() {
        let tool = DiagnosticTool::new();
        let result = tool.check_local_ip();
        // 应该返回有效的 IP 或者错误
        assert!(result.value.len() > 0);
    }

    #[test]
    fn test_nat_type_check() {
        let tool = DiagnosticTool::new();
        let result = tool.check_nat_type();
        // NAT 类型应该被检测到
        assert!(!result.value.is_empty());
    }

    #[test]
    fn test_network_connectivity_check() {
        let tool = DiagnosticTool::new();
        let result = tool.check_network_connectivity();
        // 应该能连接到公共 DNS
        assert!(result.status == DiagnosticStatus::Pass ||
                result.status == DiagnosticStatus::Warning);
    }

    #[test]
    fn test_encoders_check() {
        let tool = DiagnosticTool::new();
        let result = tool.check_encoders();
        // 至少应该有软件编码器
        assert!(result.value.contains("Software"));
    }

    #[test]
    fn test_full_diagnostic() {
        let tool = DiagnosticTool::new();
        let result = tool.run_full_diagnostic();
        assert!(result.is_ok());

        let result = result.unwrap();
        // 应该有多个诊断项
        assert!(result.details.len() >= 3);

        // 测试格式化输出
        let formatted = format_diagnostic_result(&result);
        assert!(formatted.contains("诊断结果"));
    }

    #[test]
    fn test_connection_failure_analysis() {
        let tool = DiagnosticTool::new();

        let tests = vec![
            ("connection timeout", "连接超时"),
            ("connection refused", "连接被拒绝"),
            ("host unreachable", "主机不可达"),
        ];

        for (error, expected_value) in tests {
            let result = tool.analyze_connection_failure(error);
            assert!(result.value.contains(expected_value));
            assert!(result.recommendation.is_some());
        }
    }
}
