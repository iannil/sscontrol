//! WebRTC 视频轨道
//!
//! 用于通过 WebRTC 发送视频流

#![allow(dead_code)]

#[cfg(feature = "webrtc")]
use super::WebRTCConfig;
#[cfg(feature = "webrtc")]
use anyhow::{anyhow, Result};
#[cfg(feature = "webrtc")]
use std::sync::Arc;
#[cfg(feature = "webrtc")]
use tokio::sync::Mutex;
#[cfg(feature = "webrtc")]
use webrtc::{
    api::APIBuilder,
    peer_connection::{
        configuration::RTCConfiguration,
        RTCPeerConnection,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{
        track_local_static_rtp::TrackLocalStaticRTP,
        TrackLocal,
        TrackLocalWriter,
    },
};

/// 视频编解码器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    VP8,
    VP9,
}

impl VideoCodec {
    /// 获取 MIME 类型
    pub fn mime_type(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "video/h264",
            VideoCodec::VP8 => "video/vp8",
            VideoCodec::VP9 => "video/vp9",
        }
    }

    /// 获取时钟频率 (视频通常是 90kHz)
    pub fn clock_rate(&self) -> u32 {
        90000
    }
}

/// 视频轨道
#[cfg(feature = "webrtc")]
pub struct VideoTrack {
    track: Arc<TrackLocalStaticRTP>,
    codec: VideoCodec,
    sequence_number: Arc<Mutex<u16>>,
    timestamp: Arc<Mutex<u32>>,
    ssrc: Arc<Mutex<u32>>,
}

#[cfg(feature = "webrtc")]
impl VideoTrack {
    /// 创建新的视频轨道
    pub fn new(codec: VideoCodec) -> Self {
        let codec_capability = RTCRtpCodecCapability {
            mime_type: codec.mime_type().to_string(),
            clock_rate: codec.clock_rate(),
            channels: 0,
            sdp_fmtp_line: String::new(),
            rtcp_feedback: vec![],
        };

        let track = TrackLocalStaticRTP::new(
            codec_capability,
            "video".to_string(),
            "sscontrol".to_string(),
        );

        Self {
            track: Arc::new(track),
            codec,
            sequence_number: Arc::new(Mutex::new(0)),
            timestamp: Arc::new(Mutex::new(0)),
            ssrc: Arc::new(Mutex::new(1)),
        }
    }

    /// 获取轨道 (作为 trait object)
    pub fn track(&self) -> Arc<dyn TrackLocal + Send + Sync> {
        self.track.clone()
    }

    /// 获取内部轨道 (用于直接访问 RTP 写入)
    pub fn inner_track(&self) -> Arc<TrackLocalStaticRTP> {
        self.track.clone()
    }

    /// 写入 RTP 包
    pub async fn write_rtp(&self, payload: Vec<u8>, marker: bool) -> Result<usize> {
        use webrtc::rtp::packet::Packet;

        let mut seq = self.sequence_number.lock().await;
        let mut ts = self.timestamp.lock().await;
        let ssrc = *self.ssrc.lock().await;

        // 创建简单的 RTP 包
        let rtp_packet = Packet {
            header: webrtc::rtp::header::Header {
                version: 2,
                padding: false,
                extension: false,
                marker,
                payload_type: 96,  // 动态 payload type
                sequence_number: *seq,
                timestamp: *ts,
                ssrc,
                csrc: vec![],
                extension_profile: 0,
                extensions: vec![],
                extensions_padding: 0,
            },
            payload: bytes::Bytes::from(payload),
            // 默认值填充其余字段
            ..Default::default()
        };

        let result = self.track.write_rtp(&rtp_packet).await
            .map_err(|e| anyhow!("写入 RTP 包失败: {:?}", e))?;

        *seq = seq.wrapping_add(1);
        *ts = ts.wrapping_add(3000); // 90kHz / 30fps = 3000

        Ok(result)
    }

    /// 获取编解码器
    pub fn codec(&self) -> VideoCodec {
        self.codec
    }

    /// 获取当前序列号
    pub async fn sequence_number(&self) -> u16 {
        *self.sequence_number.lock().await
    }

    /// 获取当前时间戳
    pub async fn timestamp(&self) -> u32 {
        *self.timestamp.lock().await
    }
}

/// 视频发送器
#[cfg(feature = "webrtc")]
pub struct VideoSender {
    track: Arc<VideoTrack>,
    pc: Arc<RTCPeerConnection>,
    _sender: Option<Arc<webrtc::rtp_transceiver::rtp_sender::RTCRtpSender>>,
}

#[cfg(feature = "webrtc")]
impl VideoSender {
    /// 创建新的视频发送器
    pub async fn new(codec: VideoCodec, _config: WebRTCConfig) -> Result<Self> {
        // 创建 API
        let api = APIBuilder::new().build();

        // 创建 PeerConnection 配置
        let rtc_config = RTCConfiguration {
            ice_servers: vec![],
            ..Default::default()
        };

        // 创建 PeerConnection
        let pc = Arc::new(
            api.new_peer_connection(rtc_config)
                .await
                .map_err(|e| anyhow!("创建 PeerConnection 失败: {:?}", e))?
        );

        // 创建视频轨道
        let track = Arc::new(VideoTrack::new(codec));

        // 添加轨道到 PeerConnection (需要转换为 trait object)
        let track_local: Arc<dyn TrackLocal + Send + Sync> = track.track();
        let sender = pc.add_track(track_local)
            .await
            .map_err(|e| anyhow!("添加轨道失败: {:?}", e))?;

        tracing::info!("视频发送器创建成功，编解码器: {:?}", codec);

        Ok(Self {
            track,
            pc,
            _sender: Some(sender),
        })
    }

    /// 获取 PeerConnection
    pub fn peer_connection(&self) -> Arc<RTCPeerConnection> {
        self.pc.clone()
    }

    /// 获取视频轨道
    pub fn track(&self) -> Arc<VideoTrack> {
        self.track.clone()
    }

    /// 发送视频帧（需要先编码为 RTP 包）
    pub async fn send_frame(&self, encoded_data: Vec<u8>, is_key_frame: bool) -> Result<()> {
        self.track.write_rtp(encoded_data, is_key_frame).await?;
        Ok(())
    }

    /// 创建 SDP Offer
    pub async fn create_offer(&self) -> Result<String> {
        let offer = self.pc.create_offer(None).await
            .map_err(|e| anyhow!("创建 Offer 失败: {:?}", e))?;

        self.pc.set_local_description(offer.clone()).await
            .map_err(|e| anyhow!("设置本地描述失败: {:?}", e))?;

        Ok(offer.sdp)
    }

    /// 设置远程 SDP Answer
    pub async fn set_answer(&self, sdp: &str) -> Result<()> {
        use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

        let desc = RTCSessionDescription::answer(sdp.to_string())
            .map_err(|e| anyhow!("创建 SessionDescription 失败: {:?}", e))?;

        self.pc.set_remote_description(desc).await
            .map_err(|e| anyhow!("设置远程描述失败: {:?}", e))?;

        Ok(())
    }

    /// 添加 ICE 候选
    pub async fn add_ice_candidate(&self, candidate: &str, sdp_mid: &str, sdp_mline_index: u16) -> Result<()> {
        use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;

        let init = RTCIceCandidateInit {
            candidate: candidate.to_string(),
            sdp_mid: Some(sdp_mid.to_string()),
            sdp_mline_index: Some(sdp_mline_index),
            username_fragment: None,
        };

        self.pc.add_ice_candidate(init).await
            .map_err(|e| anyhow!("添加 ICE 候选失败: {:?}", e))?;

        Ok(())
    }

    /// 关闭
    pub async fn close(&self) -> Result<()> {
        self.pc.close().await
            .map_err(|e| anyhow!("关闭失败: {:?}", e))?;
        Ok(())
    }
}

#[cfg(not(feature = "webrtc"))]
pub struct VideoTrack;

#[cfg(not(feature = "webrtc"))]
pub struct VideoSender;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_codec_mime_type() {
        assert_eq!(VideoCodec::H264.mime_type(), "video/h264");
        assert_eq!(VideoCodec::VP8.mime_type(), "video/vp8");
        assert_eq!(VideoCodec::VP9.mime_type(), "video/vp9");
    }

    #[test]
    fn test_video_codec_clock_rate() {
        assert_eq!(VideoCodec::H264.clock_rate(), 90000);
        assert_eq!(VideoCodec::VP8.clock_rate(), 90000);
        assert_eq!(VideoCodec::VP9.clock_rate(), 90000);
    }

    #[cfg(feature = "webrtc")]
    #[tokio::test]
    async fn test_video_track_creation() {
        let track = VideoTrack::new(VideoCodec::H264);
        assert_eq!(track.codec(), VideoCodec::H264);
        assert_eq!(track.sequence_number().await, 0);
    }

    #[cfg(feature = "webrtc")]
    #[tokio::test]
    async fn test_video_track_sequence_increment() {
        let track = VideoTrack::new(VideoCodec::VP8);
        let seq1 = track.sequence_number().await;

        // 写入一个包应该增加序列号
        let _ = track.write_rtp(vec![0x00, 0x00, 0x01], false).await;
        let seq2 = track.sequence_number().await;

        assert_eq!(seq2, seq1.wrapping_add(1));
    }
}
