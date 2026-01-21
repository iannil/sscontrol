use crate::state::AppState;
use sscontrol::config::Config;
use tauri::State;
use std::fs;

/// 获取当前配置
#[tauri::command]
pub async fn get_config(
    state: State<'_, AppState>,
) -> Result<Config, String> {
    let config = state.config.read().await;
    Ok(config.clone())
}

/// 获取配置文件路径
#[tauri::command]
pub async fn get_config_path() -> Result<String, String> {
    Ok(Config::get_config_path(None))
}

/// 更新配置
#[tauri::command]
pub async fn update_config(
    state: State<'_, AppState>,
    config: Config,
) -> Result<(), String> {
    // 保存到文件
    let path = Config::get_config_path(None);
    config.save(&path).map_err(|e| e.to_string())?;

    // 更新内存中的配置
    let mut state_config = state.config.write().await;
    *state_config = config;

    Ok(())
}

/// 重置为默认配置
#[tauri::command]
pub async fn reset_config(
    state: State<'_, AppState>,
) -> Result<Config, String> {
    let default_config = Config::default();

    // 保存到文件
    let path = Config::get_config_path(None);
    default_config.save(&path).map_err(|e| e.to_string())?;

    // 更新内存中的配置
    let mut state_config = state.config.write().await;
    *state_config = default_config.clone();

    Ok(default_config)
}

/// 导出配置到指定路径
#[tauri::command]
pub async fn export_config(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let config = state.config.read().await;
    config.save(&path).map_err(|e| e.to_string())
}

/// 从指定路径导入配置
#[tauri::command]
pub async fn import_config(
    state: State<'_, AppState>,
    path: String,
) -> Result<Config, String> {
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let config: Config = toml::from_str(&content).map_err(|e| e.to_string())?;

    // 保存到默认配置文件
    let config_path = Config::get_config_path(None);
    config.save(&config_path).map_err(|e| e.to_string())?;

    // 更新内存中的配置
    let mut state_config = state.config.write().await;
    *state_config = config.clone();

    Ok(config)
}
