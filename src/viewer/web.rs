//! Web 查看器实现
//!
//! 启动本地 HTTP 服务器，提供 WebRTC 远程桌面查看页面

use anyhow::Result;
use axum::{
    response::Html,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Web 查看器
pub struct WebViewer {
    signaling_url: String,
    port: u16,
}

impl WebViewer {
    /// 创建新的 Web 查看器
    pub fn new(signaling_url: String, port: u16) -> Self {
        Self { signaling_url, port }
    }

    /// 启动 HTTP 服务器
    pub async fn start(&self) -> Result<u16> {
        let signaling_url = self.signaling_url.clone();

        let app = Router::new()
            .route("/", get(move || async move {
                Html(get_viewer_html(&signaling_url))
            }));

        let addr: SocketAddr = format!("127.0.0.1:{}", self.port).parse()?;
        let listener = TcpListener::bind(addr).await?;
        let actual_port = listener.local_addr()?.port();

        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        Ok(actual_port)
    }
}

/// 生成查看器 HTML 页面
fn get_viewer_html(signaling_url: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>sscontrol - 远程桌面</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            background: #1a1a2e;
            color: #eee;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            min-height: 100vh;
            display: flex;
            flex-direction: column;
        }}
        .header {{
            background: #16213e;
            padding: 10px 20px;
            display: flex;
            justify-content: space-between;
            align-items: center;
            border-bottom: 1px solid #0f3460;
        }}
        .header h1 {{
            font-size: 18px;
            font-weight: 500;
        }}
        .status {{
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 14px;
        }}
        .status-dot {{
            width: 10px;
            height: 10px;
            border-radius: 50%;
            background: #ff6b6b;
        }}
        .status-dot.connected {{
            background: #51cf66;
        }}
        .container {{
            flex: 1;
            display: flex;
            justify-content: center;
            align-items: center;
            padding: 20px;
        }}
        #video-container {{
            position: relative;
            background: #000;
            border-radius: 8px;
            overflow: hidden;
            box-shadow: 0 4px 20px rgba(0,0,0,0.5);
        }}
        #remote-video {{
            display: block;
            max-width: 100%;
            max-height: calc(100vh - 100px);
        }}
        .placeholder {{
            width: 800px;
            height: 450px;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            color: #666;
        }}
        .placeholder.hidden {{
            display: none;
        }}
        .spinner {{
            width: 40px;
            height: 40px;
            border: 3px solid #333;
            border-top-color: #0984e3;
            border-radius: 50%;
            animation: spin 1s linear infinite;
            margin-bottom: 20px;
        }}
        @keyframes spin {{
            to {{ transform: rotate(360deg); }}
        }}
        .controls {{
            position: absolute;
            bottom: 10px;
            left: 50%;
            transform: translateX(-50%);
            display: flex;
            gap: 10px;
            opacity: 0;
            transition: opacity 0.3s;
        }}
        #video-container:hover .controls {{
            opacity: 1;
        }}
        .btn {{
            padding: 8px 16px;
            border: none;
            border-radius: 4px;
            background: rgba(255,255,255,0.2);
            color: white;
            cursor: pointer;
            font-size: 14px;
        }}
        .btn:hover {{
            background: rgba(255,255,255,0.3);
        }}
        #log {{
            position: fixed;
            bottom: 10px;
            left: 10px;
            background: rgba(0,0,0,0.8);
            padding: 10px;
            border-radius: 4px;
            font-size: 12px;
            font-family: monospace;
            max-width: 400px;
            max-height: 150px;
            overflow-y: auto;
            display: none;
        }}
        #log.show {{
            display: block;
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>sscontrol 远程桌面</h1>
        <div class="status">
            <div class="status-dot" id="status-dot"></div>
            <span id="status-text">正在连接...</span>
        </div>
    </div>

    <div class="container">
        <div id="video-container">
            <video id="remote-video" autoplay playsinline></video>
            <div class="placeholder" id="placeholder">
                <div class="spinner"></div>
                <p>等待视频流...</p>
            </div>
            <div class="controls">
                <button class="btn" onclick="toggleFullscreen()">全屏</button>
                <button class="btn" onclick="toggleLog()">日志</button>
            </div>
        </div>
    </div>

    <div id="log"></div>

    <script>
        const SIGNALING_URL = '{signaling_url}';
        const ROOM_ID = 'default';

        let ws = null;
        let pc = null;
        let myPeerId = 'viewer_' + Math.random().toString(36).substr(2, 9);
        let hostPeerId = null;

        const video = document.getElementById('remote-video');
        const placeholder = document.getElementById('placeholder');
        const statusDot = document.getElementById('status-dot');
        const statusText = document.getElementById('status-text');
        const logDiv = document.getElementById('log');

        function log(msg) {{
            console.log(msg);
            const time = new Date().toLocaleTimeString();
            logDiv.innerHTML += `<div>[${{time}}] ${{msg}}</div>`;
            logDiv.scrollTop = logDiv.scrollHeight;
        }}

        function setStatus(connected, text) {{
            statusDot.classList.toggle('connected', connected);
            statusText.textContent = text;
        }}

        function toggleFullscreen() {{
            if (document.fullscreenElement) {{
                document.exitFullscreen();
            }} else {{
                document.getElementById('video-container').requestFullscreen();
            }}
        }}

        function toggleLog() {{
            logDiv.classList.toggle('show');
        }}

        // 连接信令服务器
        function connectSignaling() {{
            log('连接信令服务器: ' + SIGNALING_URL);
            ws = new WebSocket(SIGNALING_URL);

            ws.onopen = () => {{
                log('信令服务器已连接');
                setStatus(false, '已连接信令服务器');
                // 加入房间
                ws.send(JSON.stringify({{ type: 'join', room_id: ROOM_ID }}));
            }};

            ws.onmessage = async (event) => {{
                const msg = JSON.parse(event.data);
                log('收到消息: ' + msg.type);

                switch (msg.type) {{
                    case 'peers':
                        // 房间中的现有成员 - 查找 host
                        const hostPeer = msg.peers.find(p => p.id === 'host');
                        if (hostPeer) {{
                            hostPeerId = 'host';
                            log('发现被控端 (host)');
                            await createPeerConnection();
                            await createOffer();
                        }} else {{
                            log('等待被控端加入...');
                        }}
                        break;

                    case 'new_peer':
                        // 新成员加入（可能是被控端）
                        if (!hostPeerId) {{
                            hostPeerId = msg.peer_id;
                            log('被控端已加入: ' + hostPeerId);
                            await createPeerConnection();
                            await createOffer();
                        }}
                        break;

                    case 'peer_left':
                        if (msg.peer_id === hostPeerId) {{
                            log('被控端已断开');
                            hostPeerId = null;
                            setStatus(false, '被控端已断开');
                            if (pc) {{
                                pc.close();
                                pc = null;
                            }}
                            placeholder.classList.remove('hidden');
                        }}
                        break;

                    case 'answer':
                        if (pc && msg.from === hostPeerId) {{
                            log('收到 Answer');
                            await pc.setRemoteDescription({{ type: 'answer', sdp: msg.sdp }});
                        }}
                        break;

                    case 'ice':
                        if (pc && msg.from === hostPeerId) {{
                            log('收到 ICE 候选');
                            await pc.addIceCandidate({{
                                candidate: msg.candidate,
                                sdpMid: msg.sdp_mid,
                                sdpMLineIndex: msg.sdp_mline_index
                            }});
                        }}
                        break;

                    case 'offer':
                        // 如果收到 offer，说明对方想主动发起连接
                        if (msg.from !== myPeerId) {{
                            hostPeerId = msg.from;
                            log('收到 Offer from: ' + hostPeerId);
                            await createPeerConnection();
                            await pc.setRemoteDescription({{ type: 'offer', sdp: msg.sdp }});
                            const answer = await pc.createAnswer();
                            await pc.setLocalDescription(answer);
                            ws.send(JSON.stringify({{
                                type: 'answer',
                                from: myPeerId,
                                to: hostPeerId,
                                sdp: answer.sdp
                            }}));
                        }}
                        break;
                }}
            }};

            ws.onclose = () => {{
                log('信令连接断开');
                setStatus(false, '连接断开');
                // 5秒后重连
                setTimeout(connectSignaling, 5000);
            }};

            ws.onerror = (err) => {{
                log('信令错误: ' + err);
            }};
        }}

        // 创建 PeerConnection
        async function createPeerConnection() {{
            const config = {{
                iceServers: [
                    {{ urls: 'stun:stun.l.google.com:19302' }}
                ]
            }};

            pc = new RTCPeerConnection(config);

            pc.onicecandidate = (event) => {{
                if (event.candidate && hostPeerId) {{
                    log('发送 ICE 候选');
                    ws.send(JSON.stringify({{
                        type: 'ice',
                        from: myPeerId,
                        to: hostPeerId,
                        candidate: event.candidate.candidate,
                        sdp_mid: event.candidate.sdpMid || '',
                        sdp_mline_index: event.candidate.sdpMLineIndex || 0
                    }}));
                }}
            }};

            pc.ontrack = (event) => {{
                log('收到视频轨道');
                video.srcObject = event.streams[0];
                placeholder.classList.add('hidden');
                setStatus(true, '已连接');
            }};

            pc.oniceconnectionstatechange = () => {{
                log('ICE 状态: ' + pc.iceConnectionState);
                if (pc.iceConnectionState === 'disconnected' || pc.iceConnectionState === 'failed') {{
                    setStatus(false, 'ICE 连接失败');
                }}
            }};

            // 添加收发器以接收视频
            pc.addTransceiver('video', {{ direction: 'recvonly' }});
        }}

        // 创建 Offer
        async function createOffer() {{
            log('创建 Offer');
            const offer = await pc.createOffer();
            await pc.setLocalDescription(offer);

            ws.send(JSON.stringify({{
                type: 'offer',
                from: myPeerId,
                to: hostPeerId,
                sdp: offer.sdp
            }}));
        }}

        // 发送输入事件
        function setupInputHandlers() {{
            video.addEventListener('mousemove', (e) => {{
                if (!ws || ws.readyState !== WebSocket.OPEN) return;
                const rect = video.getBoundingClientRect();
                const x = (e.clientX - rect.left) / rect.width;
                const y = (e.clientY - rect.top) / rect.height;
                // 通过数据通道发送鼠标位置 (TODO: 实现数据通道)
            }});

            video.addEventListener('click', (e) => {{
                // TODO: 发送鼠标点击事件
            }});

            document.addEventListener('keydown', (e) => {{
                // TODO: 发送键盘事件
            }});
        }}

        // 启动
        connectSignaling();
        setupInputHandlers();
    </script>
</body>
</html>"#, signaling_url = signaling_url)
}
