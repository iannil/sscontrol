// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::config::get_config,
            commands::config::get_config_path,
            commands::config::update_config,
            commands::config::reset_config,
            commands::config::export_config,
            commands::config::import_config,
            commands::connection::start_connection,
            commands::connection::stop_connection,
            commands::connection::get_connection_status,
            commands::connection::get_statistics,
            commands::screen::get_screens,
            commands::screen::capture_screen_preview,
            commands::service::service_install,
            commands::service::service_uninstall,
            commands::service::service_start,
            commands::service::service_stop,
            commands::service::service_status,
        ])
        .setup(|app| {
            tracing::info!("SSControl UI 已启动");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run()
}
