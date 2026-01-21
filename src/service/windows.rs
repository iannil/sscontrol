//! Windows Service 实现
//!
//! 使用 Windows Service API 管理系统服务
//!
//! Windows Service 是 Windows 的后台服务机制，支持:
//! - 开机自启动
//! - 服务控制 (启动/停止/暂停)
//! - 事件日志集成
//! - 多种服务类型

use anyhow::{anyhow, Result};
use std::time::Duration;
use std::ffi::OsString;

use windows_service::{
    service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};

const SERVICE_NAME: &str = "sscontrol";
const SERVICE_DISPLAY_NAME: &str = "SSControl Remote Desktop Service";

/// Windows Service 控制器
pub struct WindowsServiceController;

impl WindowsServiceController {
    pub fn new() -> Self {
        Self
    }

    /// 获取服务访问权限
    fn get_service_access() -> ServiceAccess {
        ServiceAccess::START | ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS
    }

    /// 连接到服务管理器
    fn connect_manager() -> Result<ServiceManager> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE | ServiceManagerAccess::CONNECT,
        )?;
        Ok(manager)
    }
}

impl super::ServiceController for WindowsServiceController {
    fn install(&self) -> Result<()> {
        let manager = Self::connect_manager()?;

        // 获取可执行文件路径
        let exe_path = std::env::current_exe()
            .map_err(|e| anyhow!("获取可执行文件路径失败: {}", e))?;

        // 创建服务信息
        let service_info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(SERVICE_DISPLAY_NAME),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe_path.clone(),
            launch_arguments: vec![OsString::from("run")],
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };

        // 创建服务
        let _service = manager.create_service(&service_info, Self::get_service_access())
            .map_err(|e| anyhow!("创建服务失败: {}", e))?;

        println!("服务已安装: {}", SERVICE_NAME);
        println!("可执行文件路径: {}", exe_path.display());

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let manager = Self::connect_manager()?;

        // 打开服务
        let service = manager.open_service(
            OsString::from(SERVICE_NAME),
            ServiceAccess::DELETE,
        )
        .map_err(|e| anyhow!("打开服务失败: {}", e))?;

        // 删除服务
        service.delete()
            .map_err(|e| anyhow!("删除服务失败: {}", e))?;

        println!("服务已卸载: {}", SERVICE_NAME);

        Ok(())
    }

    fn start(&self) -> Result<()> {
        let manager = Self::connect_manager()?;

        // 打开服务
        let service = manager.open_service(
            OsString::from(SERVICE_NAME),
            ServiceAccess::START,
        )
        .map_err(|e| anyhow!("打开服务失败: {}", e))?;

        // 启动服务
        service.start(&[OsString::from("run")])
            .map_err(|e| anyhow!("启动服务失败: {}", e))?;

        println!("服务已启动: {}", SERVICE_NAME);

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let manager = Self::connect_manager()?;

        // 打开服务
        let service = manager.open_service(
            OsString::from(SERVICE_NAME),
            ServiceAccess::STOP,
        )
        .map_err(|e| anyhow!("打开服务失败: {}", e))?;

        // 停止服务
        let _status = service.stop()
            .map_err(|e| anyhow!("停止服务失败: {}", e))?;

        // 等待服务停止
        service.query_status()
            .map_err(|e| anyhow!("查询服务状态失败: {}", e))?;

        println!("服务已停止: {}", SERVICE_NAME);

        Ok(())
    }

    fn status(&self) -> Result<super::ServiceStatus> {
        let manager = Self::connect_manager()?;

        // 打开服务
        let service = manager.open_service(
            OsString::from(SERVICE_NAME),
            ServiceAccess::QUERY_STATUS,
        )
        .map_err(|e| anyhow!("打开服务失败: {}", e))?;

        // 查询服务状态
        let status = service.query_status()
            .map_err(|e| anyhow!("查询服务状态失败: {}", e))?;

        match status.current_state {
            windows_service::service::ServiceState::Running => {
                Ok(super::ServiceStatus::Running)
            }
            windows_service::service::ServiceState::Stopped => {
                Ok(super::ServiceStatus::Stopped)
            }
            _ => Ok(super::ServiceStatus::Unknown),
        }
    }

    fn is_installed(&self) -> bool {
        if let Ok(manager) = Self::connect_manager() {
            if manager.open_service(OsString::from(SERVICE_NAME), ServiceAccess::QUERY_STATUS).is_ok() {
                return true;
            }
        }
        false
    }
}

/// Windows 服务主循环
///
/// 此函数在服务模式下运行，处理服务控制事件
#[cfg(target_os = "windows")]
pub fn run_service() -> Result<()> {
    use windows_service::{
        service::ServiceControl,
        service_control_handler::ServiceControlHandlerResult,
        service::{ServiceControlAccept, ServiceState, ServiceStatus},
    };

    use std::sync::mpsc;

    let (tx, _rx) = mpsc::channel();

    // 定义服务控制事件处理函数
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                println!("收到停止信号");
                let _ = tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Shutdown => {
                println!("收到关机信号");
                let _ = tx.send(());
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // 注册服务控制处理器
    let status_handle = windows_service::service_control_handler::register(
        SERVICE_NAME,
        event_handler,
    )
    .map_err(|e| anyhow!("注册服务控制处理器失败: {}", e))?;

    // 设置服务状态为运行中
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: Default::default(),
        checkpoint: Default::default(),
        wait_hint: Duration::default(),
        process_id: Default::default(),
    })
    .map_err(|e| anyhow!("设置服务状态失败: {}", e))?;

    println!("Windows 服务已启动");

    // 运行主循环
    run_main_loop()?;

    // 设置服务状态为已停止
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: Default::default(),
        checkpoint: Default::default(),
        wait_hint: Duration::default(),
        process_id: Default::default(),
    })
    .map_err(|e| anyhow!("设置服务状态失败: {}", e))?;

    println!("Windows 服务已停止");

    Ok(())
}

/// Windows 服务主循环
///
/// 这里调用实际的业务逻辑
#[cfg(target_os = "windows")]
fn run_main_loop() -> Result<()> {
    // 在这里调用实际的捕获和发送逻辑
    // 使用 tokio runtime 而不是 #[tokio::main]
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| anyhow!("创建 tokio runtime 失败: {}", e))?;

    rt.block_on(async {
        // 调用实际的主循环逻辑
        // 这里应该调用 src/main.rs 中的实际捕获逻辑
        // 为了避免循环依赖，我们使用一个标志来决定是否以服务模式运行

        println!("服务主循环已启动");

        // 模拟运行直到收到停止信号
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_name() {
        assert_eq!(SERVICE_NAME, "sscontrol");
        assert_eq!(SERVICE_DISPLAY_NAME, "SSControl Remote Desktop Service");
    }
}
