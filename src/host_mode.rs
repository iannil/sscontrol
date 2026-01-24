//! Host mode - Host/server mode implementation
//!
//! This module handles the host mode that starts an embedded signaling server
//! and streams video via WebRTC.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::capture;
use crate::config;
use crate::input;
use crate::quality::{self, adaptive_bitrate::AbreConfig, roi_encoder::ROIEncoderWrapper, static_detector::{StaticSceneDetector, StaticDetectionConfig}};
use crate::signaling::{EmbeddedSignalingServer, HostSignalEvent};
use crate::webrtc;

/// Host mode with tunnel support
#[cfg(feature = "tunnel")]
pub async fn run_host_mode(
    port: u16,
    enable_tunnel: bool,
    encoder_type: Option<String>,
    bitrate: Option<u32>,
    adaptive: bool,
) -> Result<()> {
    run_host_mode_impl(port, enable_tunnel, encoder_type, bitrate, adaptive).await
}

/// Host mode without tunnel support
#[cfg(not(feature = "tunnel"))]
pub async fn run_host_mode(
    port: u16,
    _enable_tunnel: bool,
    encoder_type: Option<String>,
    bitrate: Option<u32>,
    adaptive: bool,
) -> Result<()> {
    run_host_mode_impl(port, encoder_type, bitrate, adaptive).await
}

/// Host mode implementation - WebRTC video streaming
#[cfg(feature = "tunnel")]
async fn run_host_mode_impl(
    port: u16,
    enable_tunnel: bool,
    encoder_type: Option<String>,
    bitrate_arg: Option<u32>,
    adaptive: bool,
) -> Result<()> {
    run_host_mode_inner(port, enable_tunnel, encoder_type, bitrate_arg, adaptive).await
}

/// Host mode implementation without tunnel
#[cfg(not(feature = "tunnel"))]
async fn run_host_mode_impl(
    port: u16,
    encoder_type: Option<String>,
    bitrate_arg: Option<u32>,
    adaptive: bool,
) -> Result<()> {
    run_host_mode_inner(port, encoder_type, bitrate_arg, adaptive).await
}

/// Inner host mode implementation
async fn run_host_mode_inner(
    port: u16,
    #[cfg(feature = "tunnel")] enable_tunnel: bool,
    encoder_type: Option<String>,
    bitrate_arg: Option<u32>,
    adaptive: bool,
) -> Result<()> {
    info!("sscontrol 被控端模式启动...");
    if let Some(ref enc) = encoder_type {
        info!("指定的编码器: {}", enc);
    }
    if let Some(br) = bitrate_arg {
        info!("指定的码率: {} kbps", br);
    }
    if adaptive {
        info!("自适应码率控制: 已启用");
    }

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
        let mut cf_tunnel = crate::tunnel::CloudflareTunnel::new();
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

    // 根据编码器类型确定 WebRTC codec
    // VP8: 软件编码（默认）
    // H.264: 硬件编码（NVENC/AMF/QSV/VideoToolbox）
    #[cfg(feature = "webrtc")]
    let video_codec = match encoder_type.as_deref() {
        Some("nvenc") | Some("amf") | Some("qsv") | Some("videotoolbox") | Some("h264") => {
            webrtc::host_session::VideoCodec::H264
        }
        _ => webrtc::host_session::VideoCodec::VP8,
    };

    #[cfg(feature = "webrtc")]
    info!("选择的 WebRTC codec: {}", video_codec.name());

    // 处理信令事件
    #[cfg(feature = "webrtc")]
    let signaling_server_clone = signaling_server.clone();
    #[cfg(feature = "webrtc")]
    let sessions_clone = sessions.clone();
    #[cfg(feature = "webrtc")]
    let codec_for_session = video_codec;

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
                #[cfg(feature = "webrtc")]
                HostSignalEvent::Offer { from, sdp } => {
                    info!("收到 Offer from: {}", from);

                    // 创建 WebRTC 会话
                    match webrtc::host_session::HostSession::new(from.clone(), codec_for_session).await {
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
                HostSignalEvent::Offer { from, sdp: _ } => {
                    info!("收到 Offer from: {}", from);
                    warn!("WebRTC feature 未启用，无法处理 Offer");
                }
                #[cfg(feature = "webrtc")]
                HostSignalEvent::Ice {
                    from,
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                } => {
                    info!("收到 ICE from: {}", from);

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
                #[cfg(not(feature = "webrtc"))]
                HostSignalEvent::Ice {
                    from,
                    candidate: _,
                    sdp_mid: _,
                    sdp_mline_index: _,
                } => {
                    info!("收到 ICE from: {} (WebRTC 未启用，忽略)", from);
                }
            }
        }
    });

    // 视频捕获和发送循环
    let video_task = spawn_video_task(
        capturer.clone(),
        #[cfg(feature = "webrtc")]
        sessions,
        config,
        encoder_type,
        bitrate_arg,
        adaptive,
        screen_width,
        screen_height,
    );

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

/// Spawn the video capture and streaming task
fn spawn_video_task(
    capturer: Arc<Mutex<Box<dyn capture::Capturer>>>,
    #[cfg(feature = "webrtc")] sessions: Arc<Mutex<HashMap<String, Arc<webrtc::host_session::HostSession>>>>,
    config: config::Config,
    selected_encoder: Option<String>,
    bitrate_arg: Option<u32>,
    enable_adaptive: bool,
    screen_width: u32,
    screen_height: u32,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        use crate::encoder;

        let fps = config.capture.fps;
        let bitrate = bitrate_arg.unwrap_or(2000);

        // 自适应码率控制器
        let _adaptive_controller = if enable_adaptive {
            Some(quality::adaptive_bitrate::RuleBasedAbreController::new(
                AbreConfig {
                    initial_bitrate: bitrate,
                    min_bitrate: 500,
                    max_bitrate: 10000,
                    ..Default::default()
                }
            ))
        } else {
            None
        };

        // 创建编码器（将在第一次循环时根据 session codec 决定）
        #[cfg(all(feature = "h264", feature = "webrtc"))]
        let mut vp8_encoder: Option<encoder::VP8Encoder> = None;
        #[cfg(all(feature = "h264", feature = "webrtc"))]
        let mut h264_encoder: Option<encoder::hardware::HardwareEncoderWrapper> = None;

        #[cfg(all(not(feature = "h264"), feature = "webrtc"))]
        let vp8_encoder: Option<encoder::VP8Encoder> = None;

        #[cfg(feature = "webrtc")]
        let mut current_codec: Option<webrtc::host_session::VideoCodec> = None;
        #[cfg(not(feature = "webrtc"))]
        let mut current_codec: Option<()> = None;

        // ROI 编码器包装器（基于鼠标位置的区域化编码）
        let mut _roi_encoder = ROIEncoderWrapper::new(screen_width, screen_height, None);

        // 静态画面检测器
        let mut static_detector = StaticSceneDetector::new(StaticDetectionConfig::default());

        if enable_adaptive {
            info!("自适应码率控制器已启用 (初始码率: {} kbps)", bitrate);
        }
        info!("ROI 编码器包装器已启用（基于鼠标位置）");
        info!("静态画面检测器已启用");

        let frame_interval = Duration::from_millis(1000 / fps as u64);
        let mut last_report = std::time::Instant::now();
        let mut last_stats_report = std::time::Instant::now();
        let mut frame_count = 0u64;

        // 性能统计
        let mut total_encode_time = Duration::from_secs(0);
        let mut total_bytes_sent = 0u64;
        let mut static_frames_count = 0u64;
        let mut static_skipped_count = 0u64;
        let mut consecutive_static_frames = 0u32;
        let mut last_fps_time = std::time::Instant::now();
        let mut fps_frame_count = 0u32;

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

            #[cfg(feature = "webrtc")]
            // 检查是否有活跃会话
            let active_sessions: Vec<Arc<webrtc::host_session::HostSession>> = {
                let sessions = sessions.lock().await;
                sessions.values().cloned().collect()
            };

            #[cfg(not(feature = "webrtc"))]
            let active_sessions: Vec<()> = vec![];

            if !active_sessions.is_empty() {
                #[cfg(feature = "webrtc")]
                // 获取第一个 session 的 codec 类型（所有 session 应该使用相同的 codec）
                let session_codec = active_sessions.first().map(|s| s.codec());

                #[cfg(feature = "webrtc")]
                // 如果 codec 类型改变，重新创建编码器
                if current_codec != session_codec {
                    current_codec = session_codec;

                    match session_codec {
                        Some(webrtc::host_session::VideoCodec::VP8) => {
                            info!("切换到 VP8 编码器");
                            #[cfg(feature = "h264")]
                            {
                                h264_encoder = None;
                                vp8_encoder = match encoder::VP8Encoder::new(screen_width, screen_height, fps, bitrate) {
                                    Ok(enc) => Some(enc),
                                    Err(e) => {
                                        error!("创建 VP8 编码器失败: {}", e);
                                        None
                                    }
                                };
                            }
                        }
                        Some(webrtc::host_session::VideoCodec::H264) => {
                            info!("切换到 H.264 硬件编码器");
                            #[cfg(feature = "h264")]
                            {
                                vp8_encoder = None;
                                // 根据选择的编码器类型创建
                                let hw_encoder_type = match selected_encoder.as_deref() {
                                    Some("nvenc") => Some(encoder::hardware::HardwareEncoderType::NVENC),
                                    Some("amf") => Some(encoder::hardware::HardwareEncoderType::AMF),
                                    Some("qsv") => Some(encoder::hardware::HardwareEncoderType::QuickSync),
                                    Some("videotoolbox") => Some(encoder::hardware::HardwareEncoderType::VideoToolbox),
                                    _ => None,  // Auto
                                };

                                let hw_config = encoder::hardware::HardwareEncoderConfig {
                                    encoder_type: hw_encoder_type.unwrap_or(encoder::hardware::HardwareEncoderType::Auto),
                                    bitrate,
                                    fps,
                                    preset: encoder::hardware::EncoderPreset::LowLatency,
                                };

                                h264_encoder = match encoder::hardware::HardwareEncoderWrapper::create(
                                    hw_config.encoder_type,
                                    screen_width,
                                    screen_height,
                                    hw_config,
                                ) {
                                    Ok(enc) => Some(enc),
                                    Err(e) => {
                                        error!("创建 H.264 编码器失败: {}", e);
                                        None
                                    }
                                };
                            }
                        }
                        None => {
                            warn!("无法确定 session codec 类型");
                        }
                    }
                }

                // 捕获屏幕
                let frame = {
                    let mut cap = capturer.lock().await;
                    cap.capture()
                };

                match frame {
                    Ok(_frame) => {
                        // 静态画面检测 - 如果画面静态，跳过编码以节省资源
                        let mut should_skip = false;
                        match static_detector.detect(&_frame) {
                            Ok(diff) => {
                                let is_static = diff.difference_ratio < 0.01; // 1% 阈值
                                static_detector.update_previous_frame(&_frame);

                                if is_static {
                                    static_frames_count += 1;
                                    consecutive_static_frames += 1;

                                    // 每 30 帧强制编码一个关键帧（保持连接活跃）
                                    if consecutive_static_frames % 30 == 0 {
                                        debug!("静态场景，发送关键帧保持连接");
                                        should_skip = false;
                                        // 请求关键帧 (仅支持 H264 硬件编码器)
                                        #[cfg(feature = "h264")]
                                        if let Some(ref mut enc) = h264_encoder {
                                            let _ = enc.request_key_frame();
                                        }
                                    } else {
                                        // 跳过编码，直接进入下一帧
                                        static_skipped_count += 1;
                                        should_skip = true;
                                    }
                                } else {
                                    // 动态场景，正常编码
                                    consecutive_static_frames = 0;
                                }
                            }
                            Err(e) => {
                                warn!("静态检测失败: {}，继续编码", e);
                                consecutive_static_frames = 0;
                            }
                        }

                        // 如果画面静态且不是关键帧时刻，跳过编码
                        if should_skip {
                            // 跳过编码以节省 CPU
                            continue;
                        }

                        // 根据当前 codec 编码
                        let encode_start = std::time::Instant::now();

                        #[cfg(feature = "h264")]
                        match current_codec {
                            Some(webrtc::host_session::VideoCodec::VP8) => {
                                if let Some(ref mut encoder) = vp8_encoder {
                                    match encoder.encode(&_frame) {
                                        Ok(Some(vp8_data)) => {
                                            let encode_duration = encode_start.elapsed();
                                            total_encode_time += encode_duration;

                                            // 发送给所有活跃会话
                                            for session in &active_sessions {
                                                if let Err(e) = session
                                                    .send_video_sample(vp8_data.clone(), frame_interval)
                                                    .await
                                                {
                                                    error!("发送视频帧失败: {}", e);
                                                }
                                            }
                                            total_bytes_sent += vp8_data.len() as u64;
                                            frame_count += 1;
                                            fps_frame_count += 1;
                                        }
                                        Ok(None) => {}
                                        Err(e) => {
                                            error!("VP8 编码失败: {}", e);
                                        }
                                    }
                                }
                            }
                            Some(webrtc::host_session::VideoCodec::H264) => {
                                if let Some(ref mut encoder) = h264_encoder {
                                    match encoder.encode(&_frame) {
                                        Ok(Some(packet)) => {
                                            let encode_duration = encode_start.elapsed();
                                            total_encode_time += encode_duration;

                                            // 发送给所有活跃会话
                                            for session in &active_sessions {
                                                if let Err(e) = session
                                                    .send_video_sample(packet.data.clone(), frame_interval)
                                                    .await
                                                {
                                                    error!("发送视频帧失败: {}", e);
                                                }
                                            }
                                            total_bytes_sent += packet.data.len() as u64;
                                            frame_count += 1;
                                            fps_frame_count += 1;
                                        }
                                        Ok(None) => {}
                                        Err(e) => {
                                            error!("H.264 编码失败: {}", e);
                                        }
                                    }
                                }
                            }
                            None => {}
                        }

                        #[cfg(all(not(feature = "h264"), feature = "webrtc"))]
                        if let Some(ref mut encoder) = vp8_encoder {
                            match encoder.encode(&_frame) {
                                Ok(Some(vp8_data)) => {
                                    let encode_duration = encode_start.elapsed();
                                    total_encode_time += encode_duration;

                                    // 发送给所有活跃会话
                                    for session in &active_sessions {
                                        if let Err(e) = session
                                            .send_video_sample(vp8_data.clone(), frame_interval)
                                            .await
                                        {
                                            error!("发送视频帧失败: {}", e);
                                        }
                                    }
                                    total_bytes_sent += vp8_data.len() as u64;
                                    frame_count += 1;
                                    fps_frame_count += 1;
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    error!("VP8 编码失败: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // 超时是正常的，屏幕未更新时发生
                        if e.to_string().contains("超时") {
                            debug!("屏幕捕获超时 (屏幕未更新)");
                        } else {
                            error!("屏幕捕获失败: {}", e);
                        }
                    }
                }
            }

            // 每秒报告一次
            if last_report.elapsed() >= Duration::from_secs(5) {
                if !active_sessions.is_empty() {
                    let fps_actual = frame_count as f64 / last_report.elapsed().as_secs_f64();
                    let avg_encode_time = if frame_count > 0 {
                        total_encode_time / frame_count as u32
                    } else {
                        Duration::from_secs(0)
                    };
                    let bandwidth_mbps = (total_bytes_sent as f64 / 1_000_000.0) / last_report.elapsed().as_secs_f64();

                    info!("视频流统计:");
                    info!("  帧数: {}, FPS: {:.1}", frame_count, fps_actual);
                    info!("  观看者: {}", active_sessions.len());
                    info!("  平均编码延迟: {:?}", avg_encode_time);
                    info!("  带宽: {:.2} Mbps", bandwidth_mbps);
                    info!("  静态帧检测: {}, 跳过编码: {}", static_frames_count, static_skipped_count);
                }
                frame_count = 0;
                total_bytes_sent = 0;
                static_frames_count = 0;
                static_skipped_count = 0;
                total_encode_time = Duration::from_secs(0);
                last_report = std::time::Instant::now();
            }

            // 每秒报告一次 FPS
            if last_fps_time.elapsed() >= Duration::from_secs(1) {
                let fps = fps_frame_count as f64 / last_fps_time.elapsed().as_secs_f64();
                if !active_sessions.is_empty() {
                    debug!("实时 FPS: {:.1}", fps);
                }
                fps_frame_count = 0;
                last_fps_time = std::time::Instant::now();
            }

            // 控制帧率
            let elapsed = start.elapsed();
            if elapsed < frame_interval {
                tokio::time::sleep(frame_interval - elapsed).await;
            }
        }
    })
}

/// Print local-only connection information
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

/// Get local IP address
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
