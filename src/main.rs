//! sscontrol - 无界面远程桌面应用
//!
//! 主入口程序

mod capture;
mod config;
mod encoder;
mod input;
mod network;

// 安全模块 (当启用 security feature 时)
#[cfg(feature = "security")]
mod security;

// 服务模块
mod service;

// 信令和 WebRTC 模块
mod signaling;
mod webrtc;

// 公网隧道模块 (当启用 tunnel feature 时)
#[cfg(feature = "tunnel")]
mod tunnel;

// Web 查看器模块
mod viewer;

use anyhow::Result;
use clap::{Parser, Subcommand};
use encoder::Encoder;
use service::ServiceController;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use tracing::Level;

/// sscontrol - 命令行参数
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 配置文件路径
    #[arg(short, long)]
    config: Option<String>,

    /// 目标帧率
    #[arg(short, long)]
    fps: Option<u32>,

    /// 屏幕索引
    #[arg(short = 'i', long)]
    screen: Option<u32>,

    /// 日志级别 (0=warn, 1=info, 2=debug, 3=trace)
    #[arg(short, long)]
    verbose: Option<u8>,
}

/// 子命令
#[derive(Subcommand, Debug)]
enum Commands {
    /// 以服务模式运行
    Run,

    /// 服务管理
    Service {
        #[command(subcommand)]
        action: ServiceCommands,
    },

    /// 被控端模式 - 启动内嵌信令服务器等待连接
    Host {
        /// 信令服务器端口 (默认 9527)
        #[arg(short, long, default_value = "9527")]
        port: u16,

        /// 启用公网隧道 (Cloudflare Tunnel)
        #[cfg(feature = "tunnel")]
        #[arg(long)]
        tunnel: bool,
    },

    /// 控制端模式 - 通过 IP 或公网 URL 连接被控端
    Connect {
        /// 被控端 IP 地址 (局域网模式)
        #[arg(long, conflicts_with = "url")]
        ip: Option<String>,

        /// 被控端公网 URL (隧道模式，如 wss://xxx.trycloudflare.com)
        #[arg(long, conflicts_with = "ip")]
        url: Option<String>,

        /// 被控端端口 (仅 --ip 时使用，默认 9527)
        #[arg(short, long, default_value = "9527")]
        port: u16,
    },
}

/// 服务命令
#[derive(Subcommand, Debug)]
enum ServiceCommands {
    /// 安装服务
    Install,
    /// 卸载服务
    Uninstall,
    /// 启动服务
    Start,
    /// 停止服务
    Stop,
    /// 查看服务状态
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 处理子命令
    if let Some(command) = args.command {
        return match command {
            Commands::Run => {
                init_logging(args.verbose.unwrap_or(1));
                run_service_mode().await
            }
            Commands::Service { action } => {
                handle_service_command(action)
            }
            #[cfg(feature = "tunnel")]
            Commands::Host { port, tunnel } => {
                init_logging(args.verbose.unwrap_or(1));
                run_host_mode(port, tunnel).await
            }
            #[cfg(not(feature = "tunnel"))]
            Commands::Host { port } => {
                init_logging(args.verbose.unwrap_or(1));
                run_host_mode(port).await
            }
            Commands::Connect { ip, url, port } => {
                init_logging(args.verbose.unwrap_or(1));
                run_connect_mode(ip.as_deref(), url.as_deref(), port).await
            }
        };
    }

    // 默认模式：显示帮助
    println!("sscontrol - 无界面远程桌面应用");
    println!();
    println!("用法:");
    println!("  被控端: sscontrol host [--port 9527] [--tunnel]");
    println!("  控制端: sscontrol connect --ip <IP> [--port 9527]");
    println!("          sscontrol connect --url <URL>");
    println!();
    println!("运行 'sscontrol --help' 查看更多选项");

    Ok(())
}

/// 初始化日志
fn init_logging(verbose: u8) {
    let log_level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let level = Level::from_str(log_level).unwrap_or(Level::INFO);

    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_max_level(level)
        .init();
}

/// 处理服务管理命令
fn handle_service_command(action: ServiceCommands) -> Result<()> {
    let controller = service::create_controller();

    match action {
        ServiceCommands::Install => {
            println!("正在安装服务...");
            controller.install()?;
            println!("服务安装成功!");
        }
        ServiceCommands::Uninstall => {
            println!("正在卸载服务...");
            controller.uninstall()?;
            println!("服务卸载成功!");
        }
        ServiceCommands::Start => {
            println!("正在启动服务...");
            controller.start()?;
        }
        ServiceCommands::Stop => {
            println!("正在停止服务...");
            controller.stop()?;
        }
        ServiceCommands::Status => {
            let status = controller.status()?;
            println!("服务状态: {}", status);
        }
    }

    Ok(())
}

/// 服务模式运行
async fn run_service_mode() -> Result<()> {
    info!("sscontrol 服务模式启动...");

    // 加载默认配置
    let config_path = config::Config::get_config_path(None);
    let config = config::Config::load(&config_path)?;

    run_main_loop(config).await
}

/// 主循环逻辑
async fn run_main_loop(config: config::Config) -> Result<()> {
    info!("设备 ID: {}", config.server.device_id);
    info!("目标帧率: {} fps", config.capture.fps);

    // 检查屏幕录制权限 (macOS)
    #[cfg(target_os = "macos")]
    {
        if !capture::macos::MacOSCapturer::check_screen_recording_permission() {
            warn!("屏幕录制权限未授予，请在系统设置中授权");
        }
    }

    // 创建屏幕捕获器
    info!("初始化屏幕捕获器...");
    let mut capturer = capture::create_capturer(config.capture.screen_index)?;
    capturer.start()?;

    info!("屏幕尺寸: {}x{}", capturer.width(), capturer.height());

    // 创建编码器
    info!("初始化编码器...");
    let mut encoder = encoder::H264Encoder::new(
        capturer.width(),
        capturer.height(),
        config.capture.fps,
        2000,
    )?;

    // 创建网络客户端
    let client = network::VideoClient::new(
        config.server.url.clone(),
        config.server.device_id.clone(),
    );

    // 创建输入模拟器
    info!("初始化输入模拟器...");
    let input_simulator = input::create_input_simulator()?;

    // 设置输入事件处理器
    let simulator = Arc::new(Mutex::new(input_simulator));
    let mut input_receiver = client.take_input_receiver().await?;

    // 启动输入事件处理任务
    let simulator_task = async move {
        while let Some(event) = input_receiver.recv().await {
            let mut sim = simulator.lock().await;
            if let Err(e) = sim.handle_event(&event) {
                error!("处理输入事件失败: {}", e);
            }
        }
    };
    let _simulator_handle = tokio::spawn(simulator_task);

    // 连接到服务器
    if let Err(e) = client.connect().await {
        error!("连接服务器失败: {}", e);
        warn!("将在后台继续尝试捕获屏幕...");
    } else {
        info!("连接成功!");
    }

    // 设置退出信号处理
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!("无法监听 Ctrl+C 信号: {}", e);
            tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
        }
        info!("收到退出信号，正在关闭...");
    };

    // 主捕获循环
    let capture_task = async {
        let frame_interval = Duration::from_millis(1000 / config.capture.fps as u64);
        let mut frame_count = 0u64;
        let mut last_report = std::time::Instant::now();

        loop {
            let start = std::time::Instant::now();

            match capturer.capture() {
                Ok(frame) => {
                    match encoder.encode(&frame) {
                        Ok(Some(packet)) => {
                            if client.is_connected().await {
                                if let Err(e) = client.send_packet(packet.data, packet.is_key_frame).await {
                                    error!("发送失败: {}", e);
                                }
                            }

                            frame_count += 1;

                            if last_report.elapsed() >= Duration::from_secs(1) {
                                let fps = frame_count as f64 / last_report.elapsed().as_secs_f64();
                                info!("捕获: {} 帧, 实际 FPS: {:.1}", frame_count, fps);
                                frame_count = 0;
                                last_report = std::time::Instant::now();
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            error!("编码失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("捕获失败: {}", e);
                }
            }

            let elapsed = start.elapsed();
            if elapsed < frame_interval {
                tokio::time::sleep(frame_interval - elapsed).await;
            }
        }
    };

    tokio::select! {
        _ = ctrl_c => {
            info!("正在退出...");
        }
        _ = capture_task => {}
    }

    capturer.stop()?;
    client.disconnect().await?;

    info!("sscontrol 已退出");
    Ok(())
}

// ============================================================================
// 被控端/控制端模式
// ============================================================================

/// 打印仅局域网模式的连接信息
fn print_local_only_info(local_ip: &str, port: u16) {
    println!();
    println!("========================================");
    println!("  sscontrol 被控端已启动");
    println!("========================================");
    println!();
    println!("  本机 IP: {}", local_ip);
    println!("  端口:    {}", port);
    println!();
    println!("控制端连接命令:");
    println!("  sscontrol connect --ip {} --port {}", local_ip, port);
    println!();
    println!("等待连接中... (按 Ctrl+C 退出)");
    println!();
}

/// 被控端模式 - 启动内嵌信令服务器
#[cfg(feature = "tunnel")]
async fn run_host_mode(port: u16, enable_tunnel: bool) -> Result<()> {
    run_host_mode_impl(port, enable_tunnel).await
}

/// 被控端模式 - 启动内嵌信令服务器 (无隧道支持)
#[cfg(not(feature = "tunnel"))]
async fn run_host_mode(port: u16) -> Result<()> {
    run_host_mode_impl(port, false).await
}

/// 被控端模式实现
async fn run_host_mode_impl(port: u16, #[allow(unused)] enable_tunnel: bool) -> Result<()> {
    use signaling::{EmbeddedSignalingServer, HostSignalEvent};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    info!("sscontrol 被控端模式启动...");

    // 加载配置
    let config_path = config::Config::get_config_path(None);
    let config = config::Config::load(&config_path)?;

    // 启动内嵌信令服务器
    let mut signaling_server = EmbeddedSignalingServer::new(port);
    let actual_port = signaling_server.start().await?;

    // 获取 Host 事件接收器
    let mut host_events = signaling_server
        .take_host_events()
        .expect("无法获取 Host 事件接收器");

    // 获取本机 IP 地址
    let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());

    // 启动公网隧道 (如果启用)
    #[cfg(feature = "tunnel")]
    let _tunnel = if enable_tunnel {
        info!("正在创建 Cloudflare Tunnel...");
        let mut cf_tunnel = tunnel::CloudflareTunnel::new();
        match cf_tunnel.start(actual_port) {
            Ok(tunnel_url) => {
                // 打印连接信息 (带隧道)
                println!();
                println!("========================================");
                println!("  sscontrol 被控端已启动");
                println!("========================================");
                println!();
                println!("  本机 IP: {}", local_ip);
                println!("  端口:    {}", actual_port);
                println!();
                println!("局域网连接:");
                println!("  sscontrol connect --ip {} --port {}", local_ip, actual_port);
                println!();
                println!("公网连接 (Cloudflare Tunnel):");
                println!("  sscontrol connect --url {}", tunnel_url);
                println!();
                println!("等待连接中... (按 Ctrl+C 退出)");
                println!();
                Some(cf_tunnel)
            }
            Err(e) => {
                error!("创建 Cloudflare Tunnel 失败: {}", e);
                warn!("将仅使用局域网模式");
                print_local_only_info(&local_ip, actual_port);
                None
            }
        }
    } else {
        print_local_only_info(&local_ip, actual_port);
        None
    };

    #[cfg(not(feature = "tunnel"))]
    print_local_only_info(&local_ip, actual_port);

    // 检查屏幕录制权限 (macOS)
    #[cfg(target_os = "macos")]
    {
        if !capture::macos::MacOSCapturer::check_screen_recording_permission() {
            warn!("屏幕录制权限未授予，请在系统设置中授权");
        }
    }

    // 创建屏幕捕获器
    info!("初始化屏幕捕获器...");
    let capturer = Arc::new(Mutex::new(capture::create_capturer(config.capture.screen_index)?));
    let screen_width;
    let screen_height;
    {
        let cap = capturer.lock().await;
        screen_width = cap.width();
        screen_height = cap.height();
    }
    info!("屏幕尺寸: {}x{}", screen_width, screen_height);

    // 创建输入模拟器
    info!("初始化输入模拟器...");
    let _input_simulator = input::create_input_simulator()?;

    // WebRTC 会话管理 - 使用 Arc<HostSession> 以便共享
    #[cfg(feature = "webrtc")]
    let sessions: Arc<Mutex<HashMap<String, Arc<webrtc::host_session::HostSession>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // 信令服务器引用
    let signaling_server = Arc::new(signaling_server);

    // 处理信令事件
    let signaling_server_clone = signaling_server.clone();
    #[cfg(feature = "webrtc")]
    let sessions_clone = sessions.clone();

    let signal_handler = tokio::spawn(async move {
        while let Some(event) = host_events.recv().await {
            match event {
                HostSignalEvent::ViewerJoined { peer_id } => {
                    info!("Viewer 加入: {}", peer_id);
                    println!("  [+] Viewer 连接: {}", peer_id);
                }
                HostSignalEvent::ViewerLeft { peer_id } => {
                    info!("Viewer 离开: {}", peer_id);
                    println!("  [-] Viewer 断开: {}", peer_id);

                    #[cfg(feature = "webrtc")]
                    {
                        let mut sessions = sessions_clone.lock().await;
                        if let Some(session) = sessions.remove(&peer_id) {
                            let _ = session.close().await;
                        }
                    }
                }
                HostSignalEvent::Offer { from, sdp } => {
                    info!("收到 Offer from: {}", from);

                    #[cfg(feature = "webrtc")]
                    {
                        // 创建 WebRTC 会话
                        match webrtc::host_session::HostSession::new(from.clone()).await {
                            Ok(session) => {
                                let session = Arc::new(session);

                                // 处理 Offer，生成 Answer
                                match session.handle_offer(&sdp).await {
                                    Ok(answer_sdp) => {
                                        // 发送 Answer
                                        signaling_server_clone
                                            .send_answer(&from, &answer_sdp)
                                            .await;
                                        info!("已发送 Answer to: {}", from);

                                        // 发送 ICE 候选
                                        let signaling = signaling_server_clone.clone();
                                        let peer_id = from.clone();
                                        let session_for_ice = session.clone();

                                        tokio::spawn(async move {
                                            while let Some(ice) =
                                                session_for_ice.next_ice_candidate().await
                                            {
                                                signaling
                                                    .send_ice(
                                                        &peer_id,
                                                        &ice.candidate,
                                                        &ice.sdp_mid,
                                                        ice.sdp_mline_index,
                                                    )
                                                    .await;
                                            }
                                        });

                                        // 保存会话
                                        {
                                            let mut sessions = sessions_clone.lock().await;
                                            sessions.insert(from.clone(), session);
                                        }
                                        info!("WebRTC 会话已建立: {}", from);
                                    }
                                    Err(e) => {
                                        error!("处理 Offer 失败: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                error!("创建 WebRTC 会话失败: {}", e);
                            }
                        }
                    }

                    #[cfg(not(feature = "webrtc"))]
                    {
                        warn!("WebRTC feature 未启用，无法处理 Offer");
                    }
                }
                HostSignalEvent::Ice {
                    from,
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                } => {
                    info!("收到 ICE from: {}", from);

                    #[cfg(feature = "webrtc")]
                    {
                        let sessions = sessions_clone.lock().await;
                        if let Some(session) = sessions.get(&from) {
                            let ice = webrtc::host_session::IceCandidate {
                                candidate,
                                sdp_mid,
                                sdp_mline_index,
                            };
                            if let Err(e) = session.add_ice_candidate(&ice).await {
                                error!("添加 ICE 候选失败: {}", e);
                            }
                        }
                    }
                }
            }
        }
    });

    // 视频捕获和发送循环
    #[cfg(feature = "webrtc")]
    let video_task = {
        let sessions = sessions.clone();
        let capturer = capturer.clone();
        let fps = config.capture.fps;

        tokio::spawn(async move {
            // 创建 VP8 编码器
            #[cfg(feature = "h264")]
            let mut vp8_encoder = match encoder::VP8Encoder::new(screen_width, screen_height, fps, 2000) {
                Ok(enc) => Some(enc),
                Err(e) => {
                    error!("创建 VP8 编码器失败: {}，视频流将不可用", e);
                    None
                }
            };

            #[cfg(not(feature = "h264"))]
            let vp8_encoder: Option<encoder::VP8Encoder> = None;

            if vp8_encoder.is_none() {
                warn!("VP8 编码器不可用，视频流功能禁用");
                return;
            }

            let frame_interval = Duration::from_millis(1000 / fps as u64);
            let mut last_report = std::time::Instant::now();
            let mut frame_count = 0u64;

            // 启动捕获器
            {
                let mut cap = capturer.lock().await;
                if let Err(e) = cap.start() {
                    error!("启动屏幕捕获失败: {}", e);
                    return;
                }
            }

            loop {
                let start = std::time::Instant::now();

                // 检查是否有活跃会话
                let active_sessions: Vec<Arc<webrtc::host_session::HostSession>> = {
                    let sessions = sessions.lock().await;
                    sessions.values().cloned().collect()
                };

                if !active_sessions.is_empty() {
                    // 捕获屏幕
                    let frame = {
                        let mut cap = capturer.lock().await;
                        cap.capture()
                    };

                    match frame {
                        Ok(frame) => {
                            // VP8 编码
                            #[cfg(feature = "h264")]
                            if let Some(ref mut encoder) = vp8_encoder {
                                match encoder.encode_frame(&frame) {
                                    Ok(Some(vp8_data)) => {
                                        // 发送给所有活跃会话
                                        for session in &active_sessions {
                                            if let Err(e) = session
                                                .send_video_sample(vp8_data.clone(), frame_interval)
                                                .await
                                            {
                                                error!("发送视频帧失败: {}", e);
                                            }
                                        }

                                        frame_count += 1;
                                    }
                                    Ok(None) => {}
                                    Err(e) => {
                                        error!("VP8 编码失败: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("屏幕捕获失败: {}", e);
                        }
                    }
                }

                // 每秒报告一次
                if last_report.elapsed() >= Duration::from_secs(5) {
                    if !active_sessions.is_empty() {
                        let fps_actual = frame_count as f64 / last_report.elapsed().as_secs_f64();
                        info!("视频流: {} 帧, {:.1} FPS, {} 个观看者", frame_count, fps_actual, active_sessions.len());
                    }
                    frame_count = 0;
                    last_report = std::time::Instant::now();
                }

                // 控制帧率
                let elapsed = start.elapsed();
                if elapsed < frame_interval {
                    tokio::time::sleep(frame_interval - elapsed).await;
                }
            }
        })
    };

    #[cfg(not(feature = "webrtc"))]
    let video_task = tokio::spawn(async {
        // WebRTC 未启用时的占位符
        tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
    });

    // 等待退出信号
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!("无法监听 Ctrl+C 信号: {}", e);
            tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
        }
        info!("收到退出信号，正在关闭...");
    };

    ctrl_c.await;

    // 清理
    signal_handler.abort();
    video_task.abort();
    signaling_server.stop();

    // 停止捕获器
    {
        let mut cap = capturer.lock().await;
        let _ = cap.stop();
    }

    info!("被控端模式已退出");
    Ok(())
}

/// 控制端模式 - 通过 IP 或公网 URL 连接被控端
async fn run_connect_mode(ip: Option<&str>, url: Option<&str>, port: u16) -> Result<()> {
    use viewer::WebViewer;

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
    let viewer = WebViewer::new(ws_url.clone(), 0); // 0 = 随机端口
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
    signal::ctrl_c().await?;

    info!("控制端模式已退出");
    Ok(())
}

/// 打开浏览器
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

/// 获取本机 IP 地址
fn get_local_ip() -> Option<String> {
    use std::net::{IpAddr, UdpSocket};

    // Try to get local IP by connecting to a public address
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local_addr = socket.local_addr().ok()?;
    let ip = local_addr.ip();

    // Check if it's a valid LAN IP (not WARP/VPN)
    if is_valid_lan_ip(&ip) {
        return Some(ip.to_string());
    }

    // Fallback: try to find a valid LAN IP from all interfaces
    #[cfg(unix)]
    {
        if let Ok(output) = std::process::Command::new("ifconfig").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if line.contains("inet ") && !line.contains("127.0.0.1") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(idx) = parts.iter().position(|&x| x == "inet") {
                        if let Some(ip_str) = parts.get(idx + 1) {
                            if let Ok(parsed_ip) = ip_str.parse::<IpAddr>() {
                                if is_valid_lan_ip(&parsed_ip) {
                                    return Some(parsed_ip.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Return the original IP if no better option found
    Some(ip.to_string())
}

/// Check if IP is a valid LAN IP (not WARP, VPN, or other virtual interfaces)
fn is_valid_lan_ip(ip: &std::net::IpAddr) -> bool {
    if let std::net::IpAddr::V4(ipv4) = ip {
        let octets = ipv4.octets();

        // Exclude Cloudflare WARP IPs (198.18.0.0/15)
        if octets[0] == 198 && (octets[1] == 18 || octets[1] == 19) {
            return false;
        }

        // Exclude CGNAT range (100.64.0.0/10) - used by some VPNs
        if octets[0] == 100 && octets[1] >= 64 && octets[1] <= 127 {
            return false;
        }

        // Exclude loopback
        if octets[0] == 127 {
            return false;
        }

        // Prefer private IP ranges (192.168.x.x, 10.x.x.x, 172.16-31.x.x)
        let is_private =
            (octets[0] == 192 && octets[1] == 168) ||
            (octets[0] == 10) ||
            (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31);

        return is_private;
    }
    false
}
