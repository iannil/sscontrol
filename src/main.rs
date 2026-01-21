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
use tracing_subscriber;
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

    /// 服务器 URL
    #[arg(short, long)]
    server: Option<String>,

    /// 设备 ID
    #[arg(short, long)]
    device_id: Option<String>,

    /// 目标帧率
    #[arg(short, long)]
    fps: Option<u32>,

    /// 屏幕索引
    #[arg(short = 'i', long)]
    screen: Option<u32>,

    /// 日志级别
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
                // 以服务模式运行
                init_logging(args.verbose.unwrap_or(1));
                run_service_mode().await
            }
            Commands::Service { action } => {
                // 服务管理命令
                handle_service_command(action)
            }
        };
    }

    // 默认模式：交互式运行
    run_interactive(args).await
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

/// 交互式运行模式
async fn run_interactive(args: Args) -> Result<()> {
    // 初始化日志
    init_logging(args.verbose.unwrap_or(1));

    info!("sscontrol v0.1.0 启动中...");

    // 加载配置
    let config_path = if let Some(ref path) = args.config {
        path.clone()
    } else {
        config::Config::get_config_path(None)
    };

    let mut config = config::Config::load(&config_path)?;

    // 命令行参数覆盖配置
    if let Some(server) = args.server {
        config.server.url = server;
    }
    if let Some(device_id) = args.device_id {
        config.server.device_id = device_id;
    }
    if let Some(fps) = args.fps {
        config.capture.fps = fps;
    }
    if let Some(screen) = args.screen {
        config.capture.screen_index = Some(screen);
    }

    // 运行主循环
    run_main_loop(config).await
}

/// 服务模式运行
///
/// 此函数在服务模式下被调用，运行相同的捕获和发送逻辑
/// 但不会因为 Ctrl+C 而退出（由服务管理器控制生命周期）
async fn run_service_mode() -> Result<()> {
    info!("sscontrol 服务模式启动...");

    // 加载默认配置
    let config_path = config::Config::get_config_path(None);
    let config = config::Config::load(&config_path)?;

    run_main_loop(config).await
}

/// 主循环逻辑
///
/// 被 run_interactive 和 run_service_mode 共享
async fn run_main_loop(config: config::Config) -> Result<()> {
    // 打印配置信息
    info!("设备 ID: {}", config.server.device_id);
    info!("服务器: {}", config.server.url);
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
    info!("初始化 H.264 编码器...");
    let mut encoder = encoder::H264Encoder::new(
        capturer.width(),
        capturer.height(),
        config.capture.fps,
        2000, // 2000 kbps
    )?;

    // 创建网络客户端
    info!("连接到服务器...");
    let client = network::VideoClient::new(
        config.server.url.clone(),
        config.server.device_id.clone(),
    );

    // 创建输入模拟器
    info!("初始化输入模拟器...");
    let input_simulator = input::create_input_simulator()?;
    info!("输入模拟器初始化成功");

    // 设置输入事件处理器 (使用 channel)
    let simulator = Arc::new(Mutex::new(input_simulator));
    let mut input_receiver = client.input_receiver().await;

    // 启动输入事件处理任务
    let simulator_task = async move {
        while let Some(event) = input_receiver.recv().await {
            let mut sim = simulator.lock().await;
            if let Err(e) = sim.handle_event(&event) {
                error!("处理输入事件失败: {}", e);
            }
        }
    };
    let simulator_handle = tokio::spawn(simulator_task);

    // 连接到服务器
    if let Err(e) = client.connect().await {
        error!("连接服务器失败: {}", e);
        warn!("将在后台继续尝试捕获屏幕...");
    } else {
        info!("连接成功!");
    }

    // 设置退出信号处理
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("无法监听 Ctrl+C");
        info!("收到退出信号，正在关闭...");
    };

    // 主捕获循环
    let capture_task = async {
        let frame_interval = Duration::from_millis(1000 / config.capture.fps as u64);
        let mut frame_count = 0u64;
        let mut last_report = std::time::Instant::now();

        loop {
            let start = std::time::Instant::now();

            // 捕获屏幕
            match capturer.capture() {
                Ok(frame) => {
                    // 编码
                    match encoder.encode(&frame) {
                        Ok(Some(packet)) => {
                            // 发送
                            if client.is_connected().await {
                                if let Err(e) = client.send_packet(packet.data, packet.is_key_frame).await {
                                    error!("发送失败: {}", e);
                                }
                            }

                            frame_count += 1;

                            // 每秒报告一次
                            if last_report.elapsed() >= Duration::from_secs(1) {
                                let fps = frame_count as f64 / last_report.elapsed().as_secs_f64();
                                info!("捕获: {} 帧, 实际 FPS: {:.1}", frame_count, fps);
                                frame_count = 0;
                                last_report = std::time::Instant::now();
                            }
                        }
                        Ok(None) => {
                            // 编码器需要更多数据
                        }
                        Err(e) => {
                            error!("编码失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("捕获失败: {}", e);
                }
            }

            // 帧率控制
            let elapsed = start.elapsed();
            if elapsed < frame_interval {
                tokio::time::sleep(frame_interval - elapsed).await;
            }
        }
    };

    // 等待退出信号
    tokio::select! {
        _ = ctrl_c => {
            info!("正在退出...");
        }
        _ = capture_task => {
            // 永不返回
        }
    }

    // 清理
    capturer.stop()?;
    client.disconnect().await?;

    info!("sscontrol 已退出");
    Ok(())
}
