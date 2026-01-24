//! Host 端 WebRTC 会话处理
//!
//! 处理来自控制端的 WebRTC 连接请求，发送屏幕视频流
//!
//! ## 支持的 Codec
//! - VP8: 软件编码 (libvpx)
//! - H.264: 硬件编码 (NVENC/AMF/QSV/VideoToolbox)

#![allow(dead_code)]

#[cfg(feature = "webrtc")]
use anyhow::{anyhow, Result};
#[cfg(feature = "webrtc")]
use std::sync::Arc;
#[cfg(feature = "webrtc")]
use tokio::sync::{mpsc, Mutex};
#[cfg(feature = "webrtc")]
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors,
        media_engine::{MediaEngine, MIME_TYPE_VP8, MIME_TYPE_H264},
        setting_engine::SettingEngine,
        APIBuilder,
    },
    ice::network_type::NetworkType,
    ice_transport::ice_server::RTCIceServer,
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration,
        sdp::session_description::RTCSessionDescription,
        RTCPeerConnection,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
};

/// 视频 Codec 类型
#[cfg(feature = "webrtc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    VP8,
    H264,
}

#[cfg(feature = "webrtc")]
impl VideoCodec {
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::VP8 => MIME_TYPE_VP8,
            Self::H264 => MIME_TYPE_H264,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::VP8 => "VP8",
            Self::H264 => "H.264",
        }
    }
}


/// Host WebRTC 会话
#[cfg(feature = "webrtc")]
pub struct HostSession {
    peer_id: String,
    pc: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
    ice_tx: mpsc::UnboundedSender<IceCandidate>,
    ice_rx: Arc<Mutex<mpsc::UnboundedReceiver<IceCandidate>>>,
    codec: VideoCodec,
}

/// ICE 候选
#[cfg(feature = "webrtc")]
#[derive(Debug, Clone)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: String,
    pub sdp_mline_index: u16,
}

#[cfg(feature = "webrtc")]
impl HostSession {
    /// 创建新的 Host 会话
    ///
    /// # 参数
    /// * `peer_id` - 对端 ID
    /// * `codec` - 视频 codec 类型（VP8 或 H.264）
    pub async fn new(peer_id: String, codec: VideoCodec) -> Result<Self> {
        // 创建媒体引擎
        let mut m = MediaEngine::default();
        m.register_default_codecs()
            .map_err(|e| anyhow!("注册编解码器失败: {:?}", e))?;

        // 创建拦截器
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut m)
            .map_err(|e| anyhow!("注册拦截器失败: {:?}", e))?;

        // 创建设置引擎 - 禁用 IPv6，只使用 IPv4 UDP
        let mut setting_engine = SettingEngine::default();
        setting_engine.set_network_types(vec![NetworkType::Udp4]);

        // 尝试自动获取本机 IP 并设置为 NAT 1:1 映射
        // 这将强制 ICE 使用该 IP 而不是自动发现
        if let Ok(local_ip) = local_ip_address::local_ip() {
            tracing::info!("自动检测到的本机 IP: {}", local_ip);

            // 验证不是链路本地地址
            if let std::net::IpAddr::V4(ipv4) = local_ip {
                let octets = ipv4.octets();
                if !(octets[0] == 169 && octets[1] == 254) {
                    // 设置 NAT 1:1 IP 映射，强制 ICE 使用此 IP
                    // 对于所有候选类型（host, srflx, relay）都使用此 IP
                    use webrtc::ice_transport::ice_candidate_type::RTCIceCandidateType;

                    setting_engine.set_nat_1to1_ips(
                        vec![local_ip.to_string()],
                        RTCIceCandidateType::Host,
                    );
                    setting_engine.set_nat_1to1_ips(
                        vec![local_ip.to_string()],
                        RTCIceCandidateType::Srflx,
                    );
                    tracing::info!("设置 NAT 1:1 IP 映射: {}", local_ip);
                }
            }
        }

        // 设置接口过滤 - 过滤掉链路本地地址接口 (名称包含某些特征)
        // 注意: webrtc-rs 的 interface_filter 只接受接口名称参数
        setting_engine.set_interface_filter(Box::new(|interface_name: &str| {
            tracing::debug!("检查网络接口: {}", interface_name);

            // 过滤掉一些已知的链路本地接口名称模式
            // Windows 上的链路本地接口通常没有特定名称模式
            // 所以我们接受所有接口，在候选阶段过滤
            true
        }));

        // 创建 API
        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .build();

        // ICE 服务器配置 - 零第三方依赖
        //
        // sscontrol 2.0 使用纯 P2P 架构，无需 STUN/TURN 服务器
        // NAT 穿透通过以下技术实现：
        // 1. 主动 NAT 类型探测 (无需 STUN)
        // 2. 预测性端口攻击 (突破对称 NAT)
        // 3. 本地网络发现 (mDNS)
        //
        // 空的 ice_servers 列表表示仅使用主机候选
        let config = RTCConfiguration {
            ice_servers: vec![], // 零第三方依赖
            ..Default::default()
        };

        tracing::info!("ICE 配置: 零第三方依赖 (纯 P2P 模式)");

        // 创建 PeerConnection
        let pc = Arc::new(
            api.new_peer_connection(config)
                .await
                .map_err(|e| anyhow!("创建 PeerConnection 失败: {:?}", e))?,
        );

        tracing::info!("使用视频 codec: {}", codec.name());

        // 创建视频轨道
        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: codec.mime_type().to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "screen".to_owned(),
        ));

        // 添加视频轨道到 PeerConnection
        pc.add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await
            .map_err(|e| anyhow!("添加视频轨道失败: {:?}", e))?;

        // ICE 候选通道
        let (ice_tx, ice_rx) = mpsc::unbounded_channel();

        // 设置 ICE 候选回调
        let ice_tx_clone = ice_tx.clone();
        pc.on_ice_candidate(Box::new(move |c| {
            if let Some(c) = c {
                if let Ok(init) = c.to_json() {
                    // 过滤无效的链路本地地址 (169.254.x.x)
                    let candidate_str = &init.candidate;
                    if candidate_str.contains("169.254.") {
                        tracing::debug!("过滤链路本地地址候选: {}", candidate_str);
                        return Box::pin(async {});
                    }

                    let _ = ice_tx_clone.send(IceCandidate {
                        candidate: init.candidate,
                        sdp_mid: init.sdp_mid.unwrap_or_default(),
                        sdp_mline_index: init.sdp_mline_index.unwrap_or(0),
                    });
                }
            }
            Box::pin(async {})
        }));

        // 设置连接状态回调
        let peer_id_clone = peer_id.clone();
        pc.on_peer_connection_state_change(Box::new(move |s| {
            tracing::info!("PeerConnection 状态 [{}]: {:?}", peer_id_clone, s);
            Box::pin(async {})
        }));

        Ok(Self {
            peer_id,
            pc,
            video_track,
            ice_tx,
            ice_rx: Arc::new(Mutex::new(ice_rx)),
            codec,
        })
    }

    /// 处理来自 Viewer 的 Offer，返回 Answer
    pub async fn handle_offer(&self, offer_sdp: &str) -> Result<String> {
        let offer = RTCSessionDescription::offer(offer_sdp.to_string())
            .map_err(|e| anyhow!("解析 Offer 失败: {:?}", e))?;

        self.pc
            .set_remote_description(offer)
            .await
            .map_err(|e| anyhow!("设置远程描述失败: {:?}", e))?;

        let answer = self
            .pc
            .create_answer(None)
            .await
            .map_err(|e| anyhow!("创建 Answer 失败: {:?}", e))?;

        self.pc
            .set_local_description(answer.clone())
            .await
            .map_err(|e| anyhow!("设置本地描述失败: {:?}", e))?;

        Ok(answer.sdp)
    }

    /// 添加远程 ICE 候选
    pub async fn add_ice_candidate(&self, candidate: &IceCandidate) -> Result<()> {
        use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

        let init = RTCIceCandidateInit {
            candidate: candidate.candidate.clone(),
            sdp_mid: Some(candidate.sdp_mid.clone()),
            sdp_mline_index: Some(candidate.sdp_mline_index),
            username_fragment: None,
        };

        self.pc
            .add_ice_candidate(init)
            .await
            .map_err(|e| anyhow!("添加 ICE 候选失败: {:?}", e))?;

        Ok(())
    }

    /// 获取下一个本地 ICE 候选
    pub async fn next_ice_candidate(&self) -> Option<IceCandidate> {
        self.ice_rx.lock().await.recv().await
    }

    /// 发送视频帧 (VP8 编码后的数据)
    pub async fn send_video_sample(&self, data: Vec<u8>, duration: std::time::Duration) -> Result<()> {
        use webrtc::media::Sample;

        let sample = Sample {
            data: data.into(),
            duration,
            ..Default::default()
        };

        self.video_track
            .write_sample(&sample)
            .await
            .map_err(|e| anyhow!("发送视频帧失败: {:?}", e))?;

        Ok(())
    }

    /// 获取 peer_id
    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }

    /// 获取视频 codec 类型
    pub fn codec(&self) -> VideoCodec {
        self.codec
    }

    /// 关闭会话
    pub async fn close(&self) -> Result<()> {
        self.pc
            .close()
            .await
            .map_err(|e| anyhow!("关闭 PeerConnection 失败: {:?}", e))?;
        Ok(())
    }
}

#[cfg(not(feature = "webrtc"))]
pub struct HostSession;

#[cfg(not(feature = "webrtc"))]
impl HostSession {
    pub async fn new(_peer_id: String) -> anyhow::Result<Self> {
        Err(anyhow::anyhow!("WebRTC feature 未启用"))
    }
}
