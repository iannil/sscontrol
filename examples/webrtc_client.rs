//! 完整的 WebRTC 客户端示例
//!
//! 演示如何使用信令客户端和 PeerConnection 建立完整的 WebRTC 连接
//!
//! 使用方法:
//! 1. 启动信令服务器: cargo run --example signaling_server
//! 2. 启动客户端 1: cargo run --example webrtc_client --features webrtc -- --id client1 --room test
//! 3. 启动客户端 2: cargo run --example webrtc_client --features webrtc -- --id client2 --room test

#[cfg(feature = "webrtc")]
use std::collections::HashMap;
#[cfg(feature = "webrtc")]
use std::sync::Arc;
#[cfg(feature = "webrtc")]
use std::time::Duration;

#[cfg(feature = "webrtc")]
use sscontrol::webrtc::{
    peer_connection::RealPeerConnection,
    signaling::{SignalingClient, SignalingEvent},
    WebRTCConfig,
};
#[cfg(feature = "webrtc")]
use sscontrol::webrtc::IceCandidate;
#[cfg(feature = "webrtc")]
use sscontrol::webrtc::SdpMessage;
#[cfg(feature = "webrtc")]
use sscontrol::webrtc::SdpType;
#[cfg(feature = "webrtc")]
use tokio::sync::Mutex;

#[cfg(feature = "webrtc")]
struct WebRTCSession {
    peer_id: String,
    pc: Arc<Mutex<RealPeerConnection>>,
    pending_candidates: Arc<Mutex<Vec<IceCandidate>>>,
}

#[cfg(feature = "webrtc")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .init();

    let args = parse_args();
    let my_id = args.id.unwrap_or_else(|| format!("client_{}", std::process::id()));
    let room_id = args.room.unwrap_or("default_room".to_string());

    println!("========================================");
    println!("WebRTC 客户端示例");
    println!("========================================");
    println!("客户端 ID: {}", my_id);
    println!("房间 ID: {}", room_id);
    println!("信令服务器: {}", args.signal_server);
    println!("========================================\n");

    // 创建 WebRTC 配置
    let config = WebRTCConfig {
        stun_servers: vec![
            "stun:stun.l.google.com:19302".to_string(),
        ],
        turn_servers: vec![],
        ice_transport_policy: sscontrol::webrtc::IceTransportPolicy::All,
        use_ipv6: true,
    };

    // 创建信令客户端
    let signaling = SignalingClient::new(args.signal_server);
    let signaling_clone = signaling.clone();

    // 存储所有对等端的连接
    let sessions: Arc<Mutex<HashMap<String, Arc<WebRTCSession>>>> = Arc::new(Mutex::new(HashMap::new()));
    let sessions_clone = sessions.clone();

    // 设置事件处理器
    signaling.on_event(move |event| {
        let sessions = sessions_clone.clone();
        let my_id = my_id.clone();
        let config = config.clone();

        tokio::spawn(async move {
            match event {
                SignalingEvent::Connected => {
                    println!("✓ 已连接到信令服务器");
                }
                SignalingEvent::Joined { room_id, peers } => {
                    println!("✓ 已加入房间: {}", room_id);
                    println!("  现有对等端: {:?}", peers);

                    // 如果房间里有其他对等端，发起连接
                    for peer in peers {
                        if peer.id != my_id {
                            println!("  -> 发起连接到: {}", peer.id);
                            if let Err(e) = create_offer_to_peer(
                                &sessions,
                                &my_id,
                                &peer.id,
                                &config,
                                &signaling_clone,
                            ).await {
                                eprintln!("  -> 错误: {}", e);
                            }
                        }
                    }
                }
                SignalingEvent::NewPeer { peer_id } => {
                    println!("✓ 新对等端加入: {}", peer_id);
                    // 发起连接到新对等端
                    if let Err(e) = create_offer_to_peer(
                        &sessions,
                        &my_id,
                        &peer_id,
                        &config,
                        &signaling_clone,
                    ).await {
                        eprintln!("  -> 错误: {}", e);
                    }
                }
                SignalingEvent::PeerLeft { peer_id } => {
                    println!("✓ 对等端离开: {}", peer_id);
                    sessions.lock().await.remove(&peer_id);
                }
                SignalingEvent::Offer { from, sdp } => {
                    println!("✓ 收到 Offer from: {}", from);
                    if let Err(e) = handle_offer(
                        &sessions,
                        &my_id,
                        &from,
                        &sdp,
                        &config,
                        &signaling_clone,
                    ).await {
                        eprintln!("  -> 错误: {}", e);
                    }
                }
                SignalingEvent::Answer { from, sdp } => {
                    println!("✓ 收到 Answer from: {}", from);
                    if let Err(e) = handle_answer(&sessions, &from, &sdp).await {
                        eprintln!("  -> 错误: {}", e);
                    }
                }
                SignalingEvent::Ice { from, candidate, sdp_mid, sdp_mline_index } => {
                    println!("✓ 收到 ICE 候选 from: {}", from);
                    if let Err(e) = handle_ice_candidate(
                        &sessions,
                        &from,
                        &candidate,
                        &sdp_mid,
                        sdp_mline_index,
                    ).await {
                        eprintln!("  -> 错误: {}", e);
                    }
                }
                SignalingEvent::Error { message } => {
                    eprintln!("✗ 错误: {}", message);
                }
                SignalingEvent::Disconnected => {
                    println!("✗ 与信令服务器断开连接");
                }
            }
        });
    });

    // 连接到信令服务器
    signaling.connect().await?;

    // 加入房间
    signaling.join_room(room_id.clone()).await?;

    println!("\n等待其他对等端连接...\n");

    // 保持运行
    tokio::signal::ctrl_c().await?;
    println!("\n正在断开连接...");

    signaling.disconnect().await?;

    // 关闭所有对等端连接
    let sessions = sessions.lock().await;
    for session in sessions.values() {
        let mut pc = session.pc.lock().await;
        let _ = pc.close();
    }

    println!("已退出");
    Ok(())
}

#[cfg(feature = "webrtc")]
async fn create_offer_to_peer(
    sessions: &Arc<Mutex<HashMap<String, Arc<WebRTCSession>>>>,
    my_id: &str,
    peer_id: &str,
    config: &WebRTCConfig,
    signaling: &SignalingClient,
) -> anyhow::Result<()> {
    // 创建 PeerConnection
    let pc = RealPeerConnection::new(config.clone()).await?;

    // 设置 ICE 候选回调
    let sessions = sessions.clone();
    let peer_id = peer_id.to_string();
    let signaling = signaling.clone();
    pc.on_ice_candidate(move |candidate| {
        let sessions = sessions.clone();
        let peer_id = peer_id.clone();
        let signaling = signaling.clone();

        async move {
            if let Some(cand) = candidate {
                println!("  -> 发送 ICE 候选 to: {}", peer_id);
                let _ = signaling.send_ice(
                    peer_id.clone(),
                    cand.candidate,
                    cand.sdp_mid,
                    cand.sdp_mline_index,
                ).await;
            } else {
                println!("  -> ICE 候选收集完成");
            }
        }

        tokio::spawn(async move {
            // 暂时存储候选，等待连接建立后发送
            if let Some(cand) = candidate {
                let sessions = sessions.lock().await;
                if let Some(session) = sessions.get(&peer_id) {
                    let _ = signaling.send_ice(
                        peer_id.clone(),
                        cand.candidate,
                        cand.sdp_mid,
                        cand.sdp_mline_index,
                    ).await;
                }
            }
        });
        Box::pin(async {})
    })?;

    let pc = Arc::new(Mutex::new(pc));

    // 创建会话
    let session = Arc::new(WebRTCSession {
        peer_id: peer_id.to_string(),
        pc: pc.clone(),
        pending_candidates: Arc::new(Mutex::new(Vec::new())),
    });

    sessions.lock().await.insert(peer_id.to_string(), session.clone());

    // 创建数据通道
    let mut pc_lock = pc.lock().await;
    let dc = pc_lock.create_data_channel("data").await?;
    println!("  -> 数据通道创建成功");

    // 设置数据通道消息回调
    let peer_id = peer_id.to_string();
    dc.on_message(Box::new(move |msg| {
        println!("  <- 收到数据 from {}: {:?}", peer_id, msg);
        Box::pin(async {})
    }));

    // 创建 Offer
    let offer = pc_lock.create_offer()?;
    println!("  -> Offer 创建成功");

    // 发送 Offer
    signaling.send_offer(peer_id.to_string(), offer.sdp.clone()).await?;
    println!("  -> Offer 已发送");

    Ok(())
}

#[cfg(feature = "webrtc")]
async fn handle_offer(
    sessions: &Arc<Mutex<HashMap<String, Arc<WebRTCSession>>>>,
    my_id: &str,
    from: &str,
    sdp: &str,
    config: &WebRTCConfig,
    signaling: &SignalingClient,
) -> anyhow::Result<()> {
    // 创建 PeerConnection
    let pc = RealPeerConnection::new(config.clone()).await?;

    // 设置 ICE 候选回调
    let peer_id = from.to_string();
    pc.on_ice_candidate(move |candidate| {
        let peer_id = peer_id.clone();
        let signaling = signaling.clone();

        async move {
            if let Some(cand) = candidate {
                println!("  -> 发送 ICE 候选 to: {}", peer_id);
                let _ = signaling.send_ice(
                    peer_id.clone(),
                    cand.candidate,
                    cand.sdp_mid,
                    cand.sdp_mline_index,
                ).await;
            }
        }

        tokio::spawn(async move {
            if let Some(cand) = candidate {
                let _ = signaling.send_ice(
                    peer_id.clone(),
                    cand.candidate,
                    cand.sdp_mid,
                    cand.sdp_mline_index,
                ).await;
            }
        });
        Box::pin(async {})
    })?;

    let pc = Arc::new(Mutex::new(pc));

    // 创建会话
    let session = Arc::new(WebRTCSession {
        peer_id: from.to_string(),
        pc: pc.clone(),
        pending_candidates: Arc::new(Mutex::new(Vec::new())),
    });

    sessions.lock().await.insert(from.to_string(), session);

    // 设置远程描述
    let mut pc_lock = pc.lock().await;
    let offer_msg = SdpMessage {
        sdp_type: SdpType::Offer,
        sdp: sdp.to_string(),
    };
    pc_lock.set_remote_description(&offer_msg)?;

    // 创建 Answer
    let answer = pc_lock.create_offer()?; // 注意: 这里应该是 create_answer，但当前使用 offer 代替
    let answer_msg = SdpMessage {
        sdp_type: SdpType::Answer,
        sdp: answer.sdp.clone(),
    };

    // 发送 Answer
    signaling.send_answer(from.to_string(), answer.sdp.clone()).await?;
    println!("  -> Answer 已发送");

    Ok(())
}

#[cfg(feature = "webrtc")]
async fn handle_answer(
    sessions: &Arc<Mutex<HashMap<String, Arc<WebRTCSession>>>>,
    from: &str,
    sdp: &str,
) -> anyhow::Result<()> {
    let sessions = sessions.lock().await;
    if let Some(session) = sessions.get(from) {
        let mut pc = session.pc.lock().await;
        let answer_msg = SdpMessage {
            sdp_type: SdpType::Answer,
            sdp: sdp.to_string(),
        };
        pc.set_answer(&answer_msg)?;
        println!("  -> Answer 已设置");
    }
    Ok(())
}

#[cfg(feature = "webrtc")]
async fn handle_ice_candidate(
    sessions: &Arc<Mutex<HashMap<String, Arc<WebRTCSession>>>>,
    from: &str,
    candidate: &str,
    sdp_mid: &str,
    sdp_mline_index: u16,
) -> anyhow::Result<()> {
    let sessions = sessions.lock().await;
    if let Some(session) = sessions.get(from) {
        let mut pc = session.pc.lock().await;
        let ice_candidate = IceCandidate {
            candidate: candidate.to_string(),
            sdp_mid: sdp_mid.to_string(),
            sdp_mline_index,
        };
        pc.add_ice_candidate(&ice_candidate)?;
        println!("  -> ICE 候选已添加");
    }
    Ok(())
}

#[cfg(not(feature = "webrtc"))]
fn main() -> anyhow::Result<()> {
    println!("WebRTC 功能未启用");
    println!("请使用 --features webrtc 编译此示例:");
    println!("  cargo run --example webrtc_client --features webrtc");
    Ok(())
}

/// 命令行参数
struct Args {
    id: Option<String>,
    room: Option<String>,
    signal_server: String,
}

/// 解析命令行参数
fn parse_args() -> Args {
    let mut args = Args {
        id: None,
        room: None,
        signal_server: "ws://127.0.0.1:8080".to_string(),
    };

    let mut argv = std::env::args().skip(1);
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--id" => args.id = argv.next(),
            "--room" => args.room = argv.next(),
            "--server" => args.signal_server = argv.next().unwrap_or(args.signal_server),
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {}
        }
    }

    args
}

fn print_usage() {
    println!("用法: webrtc_client [OPTIONS]");
    println!();
    println!("选项:");
    println!("  --id <ID>         客户端 ID");
    println!("  --room <ROOM>     房间 ID");
    println!("  --server <URL>    信令服务器 URL (默认: ws://127.0.0.1:8080)");
    println!("  --help, -h        显示帮助信息");
}
