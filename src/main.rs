//! sscontrol - 无界面远程桌面应用
//!
//! 主入口程序

mod capture;
mod cli;
mod commands;
mod config;
mod connect_mode;
mod encoder;
mod host_mode;
mod input;
mod network;
mod nat;
mod quality;
mod tools;

#[cfg(feature = "discovery")]
mod discovery;

#[cfg(feature = "pairing")]
mod pairing;

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
use clap::Parser;

use cli::{Args, Commands};
use commands::*;
use encoder::hardware::HardwareEncoder;

#[tokio::main]
async fn main() -> Result<()> {
    // 安装 rustls CryptoProvider (webrtc-rs 需要)
    #[cfg(any(feature = "webrtc", feature = "security"))]
    {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

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
                host_mode::run_host_mode(port, tunnel, args.encoder, args.bitrate, args.adaptive).await
            }
            #[cfg(not(feature = "tunnel"))]
            Commands::Host { port, .. } => {
                init_logging(args.verbose.unwrap_or(1));
                host_mode::run_host_mode(port, false, args.encoder, args.bitrate, args.adaptive).await
            }
            Commands::Connect { ip, url, port } => {
                init_logging(args.verbose.unwrap_or(1));
                connect_mode::run_connect_mode(ip.as_deref(), url.as_deref(), port).await
            }
            Commands::ListEncoders => {
                init_logging(args.verbose.unwrap_or(1));
                handle_list_encoders()
            }
            Commands::Benchmark { duration, width, height } => {
                init_logging(args.verbose.unwrap_or(1));
                handle_benchmark(duration, width, height).await
            }
            Commands::Doctor { nat, quality } => {
                init_logging(args.verbose.unwrap_or(1));
                handle_doctor(nat, quality).await
            }
            Commands::SysInfo => {
                init_logging(args.verbose.unwrap_or(1));
                handle_sysinfo()
            }
            Commands::Config { path } => {
                handle_generate_config(path)
            }
            Commands::Stats => {
                handle_stats()
            }
        };
    }

    // 默认模式：显示帮助
    print_usage();
    Ok(())
}

/// Print usage information
fn print_usage() {
    println!("sscontrol - 无界面远程桌面应用");
    println!();
    println!("用法:");
    println!("  被控端: sscontrol host [--port 9527] [--tunnel] [--encoder <类型>] [--bitrate <kbps>] [--adaptive]");
    println!("  控制端: sscontrol connect --ip <IP> [--port 9527]");
    println!("          sscontrol connect --url <URL>");
    println!();
    println!("工具命令:");
    println!("  列出编码器: sscontrol list-encoders");
    println!("  编码器测试: sscontrol benchmark [--duration N] [--width W] [--height H]");
    println!("  网络诊断: sscontrol doctor [--nat] [--quality]");
    println!("  系统信息: sscontrol sysinfo");
    println!("  生成配置: sscontrol config [--path <路径>]");
    println!("  实时统计: sscontrol stats");
    println!();
    println!("编码器类型: auto, software, nvenc, amf, qsv, videotoolbox");
    println!();
    println!("运行 'sscontrol --help' 查看更多选项");
}

/// 服务模式运行
async fn run_service_mode() -> Result<()> {
    use tracing::info;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::signal;
    use tokio::sync::Mutex;
    use tracing::{error, warn};

    info!("sscontrol 服务模式启动...");

    // 加载默认配置
    let config_path = config::Config::get_config_path(None);
    let config = config::Config::load(&config_path)?;

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

    // 创建编码器 - 优先使用硬件编码器
    info!("初始化编码器...");
    let hw_config = encoder::hardware::HardwareEncoderConfig {
        encoder_type: encoder::hardware::HardwareEncoderType::Auto,
        bitrate: 2000,
        fps: config.capture.fps,
        preset: encoder::hardware::EncoderPreset::LowLatency,
    };

    let mut encoder: Box<dyn encoder::Encoder> = match encoder::hardware::HardwareEncoderWrapper::create(
        hw_config.encoder_type,
        capturer.width(),
        capturer.height(),
        hw_config,
    ) {
        Ok(hw_enc) => {
            info!("✅ 使用硬件编码器: {}", hw_enc.encoder_type());
            Box::new(hw_enc)
        }
        Err(e) => {
            warn!("⚠️  硬件编码器初始化失败，使用软件编码器: {}", e);
            #[cfg(feature = "h264")]
            {
                Box::new(encoder::H264Encoder::new(
                    capturer.width(),
                    capturer.height(),
                    config.capture.fps,
                    2000,
                )?)
            }
            #[cfg(not(feature = "h264"))]
            {
                Box::new(encoder::SimpleEncoder::new(
                    capturer.width(),
                    capturer.height(),
                    config.capture.fps,
                    2000,
                )?)
            }
        }
    };

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
