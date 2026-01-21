//! WebRTC PeerConnection 实现
//!
//! 使用 webrtc-rs 库实现真实的 WebRTC 连接

#[cfg(feature = "webrtc")]
use super::{IceCandidate, SdpMessage, SdpType, WebRTCConfig};
#[cfg(feature = "webrtc")]
use super::{PeerConnectionState, IceConnectionState, PeerConnectionManager};

#[cfg(feature = "webrtc")]
use anyhow::{anyhow, Result};
#[cfg(feature = "webrtc")]
use std::sync::Arc;
#[cfg(feature = "webrtc")]
use tokio::sync::Mutex;
#[cfg(feature = "webrtc")]
use webrtc::{
    api::APIBuilder,
    data_channel::RTCDataChannel,
    ice_transport::{
        ice_candidate::RTCIceCandidateInit,
        ice_connection_state::RTCIceConnectionState,
        ice_server::RTCIceServer,
    },
    peer_connection::{
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
        RTCPeerConnection,
    },
};

/// 真实的 WebRTC PeerConnection 实现
#[cfg(feature = "webrtc")]
pub struct RealPeerConnection {
    config: WebRTCConfig,
    pc: Arc<RTCPeerConnection>,
    state: Arc<Mutex<PeerConnectionState>>,
    ice_state: Arc<Mutex<IceConnectionState>>,
    data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
}

#[cfg(feature = "webrtc")]
impl RealPeerConnection {
    /// 创建新的 PeerConnection
    pub async fn new(config: WebRTCConfig) -> Result<Self> {
        // 构建 ICE 服务器配置
        let mut ice_servers = Vec::new();
        for stun_url in &config.stun_servers {
            ice_servers.push(RTCIceServer {
                urls: vec![stun_url.clone()],
                ..Default::default()
            });
        }
        for turn_server in &config.turn_servers {
            ice_servers.push(RTCIceServer {
                urls: vec![turn_server.url.clone()],
                username: turn_server.username.clone(),
                credential: turn_server.password.clone().into(),
                ..Default::default()
            });
        }

        // 创建 PeerConnection 配置
        let rtc_config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        // 创建 API
        let api = APIBuilder::new().build();

        let pc = Arc::new(
            api.new_peer_connection(rtc_config)
                .await
                .map_err(|e| anyhow!("创建 PeerConnection 失败: {:?}", e))?
        );

        // 设置状态
        let state_clone = Arc::new(Mutex::new(PeerConnectionState::New));
        let ice_state_clone = Arc::new(Mutex::new(IceConnectionState::New));

        let state_clone_cb = state_clone.clone();
        let ice_state_clone_cb = ice_state_clone.clone();

        pc.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
            let state = match s {
                RTCPeerConnectionState::Unspecified => PeerConnectionState::New,
                RTCPeerConnectionState::New => PeerConnectionState::New,
                RTCPeerConnectionState::Connecting => PeerConnectionState::Connecting,
                RTCPeerConnectionState::Connected => PeerConnectionState::Connected,
                RTCPeerConnectionState::Disconnected => PeerConnectionState::Disconnected,
                RTCPeerConnectionState::Failed => PeerConnectionState::Failed,
                RTCPeerConnectionState::Closed => PeerConnectionState::Closed,
            };
            let state_clone = state_clone_cb.clone();
            tokio::spawn(async move {
                *state_clone.lock().await = state;
            });
            Box::pin(async {})
        }));

        pc.on_ice_connection_state_change(Box::new(move |s: RTCIceConnectionState| {
            let ice_state = match s {
                RTCIceConnectionState::Unspecified => IceConnectionState::New,
                RTCIceConnectionState::New => IceConnectionState::New,
                RTCIceConnectionState::Checking => IceConnectionState::Checking,
                RTCIceConnectionState::Connected => IceConnectionState::Connected,
                RTCIceConnectionState::Completed => IceConnectionState::Completed,
                RTCIceConnectionState::Failed => IceConnectionState::Failed,
                RTCIceConnectionState::Disconnected => IceConnectionState::Disconnected,
                RTCIceConnectionState::Closed => IceConnectionState::Closed,
            };
            let ice_state_clone = ice_state_clone_cb.clone();
            tokio::spawn(async move {
                *ice_state_clone.lock().await = ice_state;
            });
            Box::pin(async {})
        }));

        tracing::info!("创建 RealPeerConnection");

        Ok(Self {
            config,
            pc,
            state: state_clone,
            ice_state: ice_state_clone,
            data_channel: Arc::new(Mutex::new(None)),
        })
    }

    /// 创建数据通道
    pub async fn create_data_channel(&self, label: &str) -> Result<Arc<RTCDataChannel>> {
        let dc = self.pc
            .create_data_channel(label, None)
            .await
            .map_err(|e| anyhow!("创建数据通道失败: {:?}", e))?;

        *self.data_channel.lock().await = Some(dc.clone());
        Ok(dc)
    }

    /// 设置 ICE 候选回调
    pub fn on_ice_candidate<F>(&self, f: F) -> Result<()>
    where
        F: Fn(Option<IceCandidate>) + Send + Sync + 'static,
    {
        let pc = self.pc.clone();
        pc.on_ice_candidate(Box::new(move |c: Option<webrtc::ice_transport::ice_candidate::RTCIceCandidate>| {
            if let Some(cand) = c {
                let cand_init = cand.to_json();
                if let Ok(init) = cand_init {
                    f(Some(IceCandidate {
                        candidate: init.candidate,
                        sdp_mid: init.sdp_mid.unwrap_or_default(),
                        sdp_mline_index: init.sdp_mline_index.unwrap_or(0),
                    }));
                }
            } else {
                f(None);
            }
            Box::pin(async {})
        }));
        Ok(())
    }

    /// 设置数据通道消息回调
    pub async fn on_data_channel_message<F>(&self, f: F) -> Result<()>
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        use webrtc::data_channel::data_channel_message::DataChannelMessage;

        let dc = self.data_channel.lock().await;
        if let Some(dc) = dc.as_ref() {
            let dc = dc.clone();
            dc.on_message(Box::new(move |msg: DataChannelMessage| {
                if !msg.is_string {
                    f(msg.data.to_vec());
                } else {
                    if let Ok(s) = std::str::from_utf8(&msg.data) {
                        tracing::debug!("收到文本消息: {}", s);
                    }
                }
                Box::pin(async {})
            }));
        }
        Ok(())
    }

    /// 通过数据通道发送数据
    pub async fn send_data(&self, data: Vec<u8>) -> Result<()> {
        let dc = self.data_channel.lock().await;
        if let Some(dc) = dc.as_ref() {
            let bytes = webrtc::data_channel::data_channel_message::DataChannelMessage {
                data: data.into(),
                is_string: false,
            };
            dc.send(&bytes.data).await
                .map_err(|e| anyhow!("发送数据失败: {:?}", e))?;
        } else {
            return Err(anyhow!("数据通道未创建"));
        }
        Ok(())
    }
}

#[cfg(feature = "webrtc")]
impl PeerConnectionManager for RealPeerConnection {
    fn create_offer(&mut self) -> Result<SdpMessage> {
        let pc = self.pc.clone();
        let state = self.state.clone();

        // 这是一个同步接口，但 WebRTC 操作是异步的
        // 在实际使用中，建议使用异步接口
        // 这里我们使用 tokio 的 block_in_place 来在同步上下文中执行异步操作
        let offer = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::try_current()
                .map_err(|e| anyhow!("没有可用的 tokio 运行时: {:?}", e))?
                .block_on(async move {
                    *state.lock().await = PeerConnectionState::Connecting;

                    let offer = pc.create_offer(None).await
                        .map_err(|e| anyhow!("创建 Offer 失败: {:?}", e))?;

                    pc.set_local_description(offer.clone()).await
                        .map_err(|e| anyhow!("设置本地描述失败: {:?}", e))?;

                    Ok::<RTCSessionDescription, anyhow::Error>(offer)
                })
        })?;

        Ok(SdpMessage {
            sdp_type: SdpType::Offer,
            sdp: offer.sdp.clone(),
        })
    }

    fn set_answer(&mut self, answer: &SdpMessage) -> Result<()> {
        let pc = self.pc.clone();
        let state = self.state.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::try_current()
                .map_err(|e| anyhow!("没有可用的 tokio 运行时: {:?}", e))?
                .block_on(async move {
                    let desc = RTCSessionDescription::answer(answer.sdp.clone())
                        .map_err(|e| anyhow!("创建 SessionDescription 失败: {:?}", e))?;

                    pc.set_remote_description(desc).await
                        .map_err(|e| anyhow!("设置远程描述失败: {:?}", e))?;

                    *state.lock().await = PeerConnectionState::Connected;
                    Ok::<(), anyhow::Error>(())
                })
        })
    }

    fn set_remote_description(&mut self, sdp: &SdpMessage) -> Result<()> {
        let pc = self.pc.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::try_current()
                .map_err(|e| anyhow!("没有可用的 tokio 运行时: {:?}", e))?
                .block_on(async move {
                    let desc = match sdp.sdp_type {
                        SdpType::Offer => RTCSessionDescription::offer(sdp.sdp.clone())
                            .map_err(|e| anyhow!("创建 Offer SessionDescription 失败: {:?}", e))?,
                        SdpType::Answer => RTCSessionDescription::answer(sdp.sdp.clone())
                            .map_err(|e| anyhow!("创建 Answer SessionDescription 失败: {:?}", e))?,
                        _ => RTCSessionDescription::pranswer(sdp.sdp.clone())
                            .map_err(|e| anyhow!("创建 SessionDescription 失败: {:?}", e))?,
                    };

                    pc.set_remote_description(desc).await
                        .map_err(|e| anyhow!("设置远程描述失败: {:?}", e))?;
                    Ok::<(), anyhow::Error>(())
                })
        })
    }

    fn add_ice_candidate(&mut self, candidate: &IceCandidate) -> Result<()> {
        let pc = self.pc.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::try_current()
                .map_err(|e| anyhow!("没有可用的 tokio 运行时: {:?}", e))?
                .block_on(async move {
                    let init = RTCIceCandidateInit {
                        candidate: candidate.candidate.clone(),
                        sdp_mid: Some(candidate.sdp_mid.clone()),
                        sdp_mline_index: Some(candidate.sdp_mline_index),
                        username_fragment: None,
                    };

                    pc.add_ice_candidate(init).await
                        .map_err(|e| anyhow!("添加 ICE 候选失败: {:?}", e))?;
                    Ok::<(), anyhow::Error>(())
                })
        })
    }

    fn connection_state(&self) -> PeerConnectionState {
        // 使用 try_lock 避免死锁
        if let Ok(state) = self.state.try_lock() {
            *state
        } else {
            PeerConnectionState::New
        }
    }

    fn ice_connection_state(&self) -> IceConnectionState {
        if let Ok(state) = self.ice_state.try_lock() {
            *state
        } else {
            IceConnectionState::New
        }
    }

    fn close(&mut self) -> Result<()> {
        let pc = self.pc.clone();
        let state = self.state.clone();
        let ice_state = self.ice_state.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::try_current()
                .map_err(|e| anyhow!("没有可用的 tokio 运行时: {:?}", e))?
                .block_on(async move {
                    pc.close().await
                        .map_err(|e| anyhow!("关闭 PeerConnection 失败: {:?}", e))?;

                    *state.lock().await = PeerConnectionState::Closed;
                    *ice_state.lock().await = IceConnectionState::Closed;
                    Ok::<(), anyhow::Error>(())
                })
        })
    }
}

#[cfg(not(feature = "webrtc"))]
/// 当 webrtc feature 未启用时的占位符
pub struct RealPeerConnection;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder() {
        // 测试占位符
        #[cfg(not(feature = "webrtc"))]
        {
            // 当 feature 未启用时，这个测试确保类型存在
            let _ = std::marker::PhantomData::<RealPeerConnection>;
        }
    }
}
