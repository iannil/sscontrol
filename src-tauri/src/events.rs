/// 系统托盘事件处理
///
/// 注意: 完整的托盘功能需要在 tauri.conf.json 中正确配置图标
/// 并且需要在 Cargo.toml 中启用 tray-icon feature

use tauri::AppHandle;

/// 初始化系统托盘
pub fn init_tray(_app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // 托盘功能需要 tray-icon feature
    // 暂时禁用，等添加图标后再启用
    Ok(())
}
