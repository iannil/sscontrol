//! WebRTC 使用示例
//!
//! 演示如何使用 WebRTC 功能创建 PeerConnection 并建立数据通道

#[cfg(feature = "webrtc")]
use sscontrol::webrtc::{
    peer_connection::RealPeerConnection,
    WebRTCConfig,
};
#[cfg(feature = "webrtc")]
use sscontrol::webrtc::SdpType;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    #[cfg(feature = "webrtc")]
    {
        // 创建 WebRTC 配置
        let config = WebRTCConfig {
            stun_servers: vec![
                "stun:stun.l.google.com:19302".to_string(),
            ],
            turn_servers: vec![],
            ice_transport_policy: sscontrol::webrtc::IceTransportPolicy::All,
            use_ipv6: true,
        };

        println!("创建 WebRTC PeerConnection...");

        // 创建 PeerConnection
        let pc = RealPeerConnection::new(config).await?;

        // 创建数据通道
        println!("创建数据通道...");
        let dc = pc.create_data_channel("sscontrol").await?;
        println!("数据通道创建成功: {:?}", dc.label());

        // 设置 ICE 候选回调
        pc.on_ice_candidate(|candidate| {
            if let Some(cand) = candidate {
                println!("收到 ICE 候选: {}", cand.candidate);
            } else {
                println!("ICE 候选收集完成");
            }
        })?;

        // 创建 SDP Offer
        println!("创建 SDP Offer...");
        let offer = pc.create_offer()?;
        println!("Offer 创建成功:");
        println!("{}", offer.sdp);

        // 注意: 在实际使用中，需要:
        // 1. 将 Offer 发送给远程对等端 (通过信令服务器)
        // 2. 接收远程的 Answer
        // 3. 调用 set_answer 设置远程描述
        // 4. 交换 ICE 候选

        println!("\n示例完成");
        println!("在实际应用中，你需要:");
        println!("1. 设置信令服务器用于交换 SDP 和 ICE 候选");
        println!("2. 实现完整的 WebRTC 握手流程");
        println!("3. 处理数据通道的消息和状态变化");
    }

    #[cfg(not(feature = "webrtc"))]
    {
        println!("WebRTC 功能未启用");
        println!("请使用 --features webrtc 编译此示例:");
        println!("  cargo run --example webrtc_example --features webrtc");
    }

    Ok(())
}
