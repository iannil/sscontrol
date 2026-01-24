//! Connect mode - Viewer/client mode implementation
//!
//! This module handles the client/viewer mode that connects to a remote host.

use anyhow::Result;
use tracing::{info, warn};

/// Connect mode - Connect to a remote host via IP or public URL
///
/// # Arguments
/// * `ip` - Optional IP address for LAN mode
/// * `url` - Optional public URL for tunnel mode
/// * `port` - Port number (only used with IP mode, defaults to 9527)
pub async fn run_connect_mode(ip: Option<&str>, url: Option<&str>, port: u16) -> Result<()> {
    info!("sscontrol 控制端模式启动...");

    // 构建 WebSocket URL
    let (ws_url, display_target) = if let Some(url) = url {
        // 公网 URL 模式
        info!("目标地址: {} (公网隧道)", url);
        (url.to_string(), url.to_string())
    } else if let Some(ip) = ip {
        // 局域网 IP 模式
        info!("目标地址: {}:{}", ip, port);
        (format!("ws://{}:{}", ip, port), format!("{}:{}", ip, port))
    } else {
        anyhow::bail!("必须指定 --ip 或 --url 参数");
    };

    println!();
    println!("========================================");
    println!("  sscontrol 控制端");
    println!("========================================");
    println!();

    // 启动 Web 查看器
    let viewer = crate::viewer::WebViewer::new(ws_url.clone(), 0); // 0 = 随机端口
    let viewer_port = viewer.start().await?;

    let viewer_url = format!("http://127.0.0.1:{}", viewer_port);

    println!("  被控端: {}", display_target);
    println!("  查看器: {}", viewer_url);
    println!();

    // 打开浏览器
    info!("正在打开浏览器...");
    if let Err(e) = open_browser(&viewer_url) {
        warn!("无法自动打开浏览器: {}", e);
        println!("请手动打开浏览器访问: {}", viewer_url);
    } else {
        println!("浏览器已打开，如未自动打开请访问: {}", viewer_url);
    }

    println!();
    println!("按 Ctrl+C 退出");

    // 等待退出信号
    tokio::signal::ctrl_c().await?;

    info!("控制端模式已退出");
    Ok(())
}

/// Open a browser with the specified URL
///
/// # Arguments
/// * `url` - The URL to open
fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()?;
    }

    Ok(())
}
