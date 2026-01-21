use sscontrol::service::{ServiceController, ServiceStatus};
use serde::{Deserialize, Serialize};

/// 服务状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatusResponse {
    /// 是否已安装
    pub installed: bool,
    /// 服务状态
    pub status: String,
}

/// 安装系统服务
#[tauri::command]
pub fn service_install() -> Result<String, String> {
    let controller = sscontrol::service::create_controller();
    controller
        .install()
        .map_err(|e| format!("安装服务失败: {}", e))?;
    Ok("服务已安装".to_string())
}

/// 卸载系统服务
#[tauri::command]
pub fn service_uninstall() -> Result<String, String> {
    let controller = sscontrol::service::create_controller();
    controller
        .uninstall()
        .map_err(|e| format!("卸载服务失败: {}", e))?;
    Ok("服务已卸载".to_string())
}

/// 启动服务
#[tauri::command]
pub fn service_start() -> Result<String, String> {
    let controller = sscontrol::service::create_controller();
    controller
        .start()
        .map_err(|e| format!("启动服务失败: {}", e))?;
    Ok("服务已启动".to_string())
}

/// 停止服务
#[tauri::command]
pub fn service_stop() -> Result<String, String> {
    let controller = sscontrol::service::create_controller();
    controller
        .stop()
        .map_err(|e| format!("停止服务失败: {}", e))?;
    Ok("服务已停止".to_string())
}

/// 获取服务状态
#[tauri::command]
pub fn service_status() -> Result<ServiceStatusResponse, String> {
    let controller = sscontrol::service::create_controller();
    let installed = controller.is_installed();

    let status = if installed {
        match controller.status() {
            Ok(ServiceStatus::Running) => "running".to_string(),
            Ok(ServiceStatus::Stopped) => "stopped".to_string(),
            Ok(ServiceStatus::Failed(msg)) => format!("failed: {}", msg),
            Ok(ServiceStatus::Unknown) => "unknown".to_string(),
            Err(e) => format!("error: {}", e),
        }
    } else {
        "not_installed".to_string()
    };

    Ok(ServiceStatusResponse { installed, status })
}
