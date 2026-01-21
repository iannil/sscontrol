//! 系统服务模块
//!
//! 提供跨平台的服务安装和管理功能
//!
//! 支持的平台:
//! - Windows: Windows Service
//! - macOS: LaunchAgent
//! - Linux: systemd

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

use anyhow::Result;

/// 服务状态
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceStatus {
    /// 运行中
    Running,
    /// 已停止
    Stopped,
    /// 失败，包含错误信息
    Failed(String),
    /// 未知状态
    Unknown,
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceStatus::Running => write!(f, "运行中"),
            ServiceStatus::Stopped => write!(f, "已停止"),
            ServiceStatus::Failed(msg) => write!(f, "失败: {}", msg),
            ServiceStatus::Unknown => write!(f, "未知"),
        }
    }
}

/// 服务控制器 trait
///
/// 定义了服务管理的基本操作接口
pub trait ServiceController {
    /// 安装服务
    ///
    /// 将当前程序注册为系统服务，设置为开机自启动
    fn install(&self) -> Result<()>;

    /// 卸载服务
    ///
    /// 从系统中移除服务注册
    fn uninstall(&self) -> Result<()>;

    /// 启动服务
    ///
    /// 启动已安装的服务
    fn start(&self) -> Result<()>;

    /// 停止服务
    ///
    /// 停止正在运行的服务
    fn stop(&self) -> Result<()>;

    /// 获取服务状态
    ///
    /// 查询服务当前是否运行
    fn status(&self) -> Result<ServiceStatus>;

    /// 检查服务是否已安装
    ///
    /// 返回服务是否已经注册到系统中
    fn is_installed(&self) -> bool;
}

/// 创建平台特定的服务控制器
///
/// 根据编译平台返回对应的服务控制器实现
pub fn create_controller() -> impl ServiceController {
    #[cfg(target_os = "windows")]
    {
        return windows::WindowsServiceController::new();
    }

    #[cfg(target_os = "macos")]
    {
        return macos::MacOSLaunchAgent::new();
    }

    #[cfg(target_os = "linux")]
    {
        return linux::SystemdService::new();
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    compile_error!("Unsupported platform for system service");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_status_display() {
        assert_eq!(ServiceStatus::Running.to_string(), "运行中");
        assert_eq!(ServiceStatus::Stopped.to_string(), "已停止");
        assert_eq!(ServiceStatus::Failed("test".to_string()).to_string(), "失败: test");
        assert_eq!(ServiceStatus::Unknown.to_string(), "未知");
    }
}
