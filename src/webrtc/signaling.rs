//! WebRTC 信令客户端
//!
//! 用于连接信令服务器并与其他对等端交换 SDP 和 ICE 候选

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

/// 信令消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    /// 客户端加入
    #[serde(rename = "join")]
    Join { room_id: String },
    /// 房间内现有客户端列表
    #[serde(rename = "peers")]
    Peers { peers: Vec<PeerInfo> },
    /// 新客户端加入房间
    #[serde(rename = "new_peer")]
    NewPeer { peer_id: String },
    /// 客户端离开
    #[serde(rename = "peer_left")]
    PeerLeft { peer_id: String },
    /// SDP Offer
    #[serde(rename = "offer")]
    Offer { from: String, to: String, sdp: String },
    /// SDP Answer
    #[serde(rename = "answer")]
    Answer { from: String, to: String, sdp: String },
    /// ICE Candidate
    #[serde(rename = "ice")]
    Ice {
        from: String,
        to: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    },
    /// 错误
    #[serde(rename = "error")]
    Error { message: String },
}

/// 客户端信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: String,
}

/// 信令客户端事件
#[derive(Debug, Clone)]
pub enum SignalingEvent {
    /// 已连接到服务器
    Connected,
    /// 已加入房间
    Joined { room_id: String, peers: Vec<PeerInfo> },
    /// 新对等端加入
    NewPeer { peer_id: String },
    /// 对等端离开
    PeerLeft { peer_id: String },
    /// 收到 Offer
    Offer { from: String, sdp: String },
    /// 收到 Answer
    Answer { from: String, sdp: String },
    /// 收到 ICE 候选
    Ice {
        from: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    },
    /// 错误
    Error { message: String },
    /// 断开连接
    Disconnected,
}

/// 事件处理器类型
pub type EventHandler = Arc<Mutex<Option<Box<dyn Fn(SignalingEvent) + Send + 'static>>>>;

/// 信令客户端
pub struct SignalingClient {
    url: String,
    sender: Arc<Mutex<Option<futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >>>>,
    event_handler: EventHandler,
    peer_id: Arc<Mutex<Option<String>>>,
}

impl SignalingClient {
    /// 创建新的信令客户端
    pub fn new(url: String) -> Self {
        Self {
            url,
            sender: Arc::new(Mutex::new(None)),
            event_handler: Arc::new(Mutex::new(None)),
            peer_id: Arc::new(Mutex::new(None)),
        }
    }

    /// 设置事件处理器
    pub fn on_event<F>(&self, handler: F)
    where
        F: Fn(SignalingEvent) + Send + 'static,
    {
        *self.event_handler.blocking_lock() = Some(Box::new(handler));
    }

    /// 连接到信令服务器
    pub async fn connect(&self) -> Result<()> {
        let url = self.url.clone();
        tracing::info!("连接到信令服务器: {}", url);

        let (ws_stream, _) = connect_async(&url)
            .await
            .map_err(|e| anyhow!("连接失败: {}", e))?;

        let (sender, mut receiver) = ws_stream.split();
        *self.sender.lock().await = Some(sender);

        // 触发连接事件
        self.emit_event(SignalingEvent::Connected);

        // 启动接收任务
        let event_handler = self.event_handler.clone();
        tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                            let event = match signal {
                                SignalMessage::Peers { peers } => SignalingEvent::Joined {
                                    room_id: "".to_string(), // 服务器返回的消息需要包含 room_id
                                    peers,
                                },
                                SignalMessage::NewPeer { peer_id } => {
                                    SignalingEvent::NewPeer { peer_id }
                                }
                                SignalMessage::PeerLeft { peer_id } => {
                                    SignalingEvent::PeerLeft { peer_id }
                                }
                                SignalMessage::Offer { from, sdp, .. } => {
                                    SignalingEvent::Offer { from, sdp }
                                }
                                SignalMessage::Answer { from, sdp, .. } => {
                                    SignalingEvent::Answer { from, sdp }
                                }
                                SignalMessage::Ice {
                                    from,
                                    candidate,
                                    sdp_mid,
                                    sdp_mline_index,
                                    ..
                                } => SignalingEvent::Ice {
                                    from,
                                    candidate,
                                    sdp_mid,
                                    sdp_mline_index,
                                },
                                SignalMessage::Error { message } => {
                                    SignalingEvent::Error { message }
                                }
                                _ => continue,
                            };

                            if let Some(handler) = event_handler.lock().await.as_ref() {
                                handler(event);
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        if let Some(handler) = event_handler.lock().await.as_ref() {
                            handler(SignalingEvent::Disconnected);
                        }
                        break;
                    }
                    Err(e) => {
                        tracing::error!("接收错误: {}", e);
                        if let Some(handler) = event_handler.lock().await.as_ref() {
                            handler(SignalingEvent::Error {
                                message: e.to_string(),
                            });
                        }
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }

    /// 加入房间
    pub async fn join_room(&self, room_id: String) -> Result<()> {
        let msg = SignalMessage::Join { room_id };
        self.send(msg).await
    }

    /// 发送 Offer
    pub async fn send_offer(&self, to: String, sdp: String) -> Result<()> {
        let msg = SignalMessage::Offer { from: self.get_peer_id().await, to, sdp };
        self.send(msg).await
    }

    /// 发送 Answer
    pub async fn send_answer(&self, to: String, sdp: String) -> Result<()> {
        let msg = SignalMessage::Answer { from: self.get_peer_id().await, to, sdp };
        self.send(msg).await
    }

    /// 发送 ICE 候选
    pub async fn send_ice(
        &self,
        to: String,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: u16,
    ) -> Result<()> {
        let msg = SignalMessage::Ice {
            from: self.get_peer_id().await,
            to,
            candidate,
            sdp_mid,
            sdp_mline_index,
        };
        self.send(msg).await
    }

    /// 发送消息
    async fn send(&self, msg: SignalMessage) -> Result<()> {
        let json = serde_json::to_string(&msg)?;
        let mut sender = self.sender.lock().await;
        if let Some(sender) = sender.as_mut() {
            sender
                .send(Message::Text(json))
                .await
                .map_err(|e| anyhow!("发送失败: {}", e))?;
        } else {
            return Err(anyhow!("未连接"));
        }
        Ok(())
    }

    /// 获取对等端 ID
    async fn get_peer_id(&self) -> String {
        self.peer_id.lock().await.clone().unwrap_or_default()
    }

    /// 设置对等端 ID
    pub async fn set_peer_id(&self, id: String) {
        *self.peer_id.lock().await = Some(id);
    }

    /// 发送事件
    fn emit_event(&self, event: SignalingEvent) {
        let handler = self.event_handler.clone();
        tokio::spawn(async move {
            if let Some(h) = handler.lock().await.as_ref() {
                h(event);
            }
        });
    }

    /// 断开连接
    pub async fn disconnect(&self) -> Result<()> {
        let mut sender = self.sender.lock().await;
        if let Some(mut s) = sender.take() {
            s.close().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_message_serialization() {
        let msg = SignalMessage::Join {
            room_id: "test_room".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"join\""));
        assert!(json.contains("test_room"));

        let parsed: SignalMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            SignalMessage::Join { room_id } => {
                assert_eq!(room_id, "test_room");
            }
            _ => panic!("解析失败"),
        }
    }

    #[test]
    fn test_offer_message() {
        let msg = SignalMessage::Offer {
            from: "peer1".to_string(),
            to: "peer2".to_string(),
            sdp: "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\n".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"offer\""));

        let parsed: SignalMessage = serde_json::from_str(&json).unwrap();
        match parsed {
            SignalMessage::Offer { from, to, sdp } => {
                assert_eq!(from, "peer1");
                assert_eq!(to, "peer2");
                assert!(sdp.starts_with("v=0"));
            }
            _ => panic!("解析失败"),
        }
    }
}
