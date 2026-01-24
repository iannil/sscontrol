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
            <!-- 使用 canvas 显示视频流 -->
            <canvas id="video-canvas"></canvas>
            <img id="video-image" style="display: none;">
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

        let ws = null;
        let canvas, ctx;
        let imageElement;
        const placeholder = document.getElementById('placeholder');
        const statusDot = document.getElementById('status-dot');
        const statusText = document.getElementById('status-text');
        const logDiv = document.getElementById('log');
        let frameCount = 0;
        let lastFpsTime = Date.now();

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

        // 连接视频流
        function connectVideoStream() {{
            log('连接视频流: ' + SIGNALING_URL);

            // 将信令 URL 转换为视频流 URL
            const streamUrl = SIGNALING_URL.replace('/ws', '/video-stream');

            canvas = document.getElementById('video-canvas');
            ctx = canvas.getContext('2d');
            imageElement = document.getElementById('video-image');

            // 发送请求获取视频流
            fetch(streamUrl).then(response => {{
                if (!response.ok) {{
                    throw new Error('无法连接视频流');
                }}
                // 使用 reader 读取流
                const reader = response.body.getReader();

                function read() {{
                    reader.read().then(({{ done, value }}) => {{
                        if (done) {{
                            log('视频流结束');
                            return;
                        }}

                        // value 是 Uint8Array，包含 JPEG 数据
                        if (value && value.length > 0) {{
                            const blob = new Blob([value], {{ type: 'image/jpeg' }});
                            const url = URL.createObjectURL(blob);

                            imageElement.onload = () => {{
                                // 设置 canvas 大小匹配图片
                                if (canvas.width !== imageElement.naturalWidth ||
                                    canvas.height !== imageElement.naturalHeight) {{
                                    canvas.width = imageElement.naturalWidth;
                                    canvas.height = imageElement.naturalHeight;
                                }}

                                // 绘制图片
                                ctx.drawImage(imageElement, 0, 0);

                                // 释放 URL
                                URL.revokeObjectURL(url);

                                // 隐藏占位符
                                placeholder.classList.add('hidden');
                                setStatus(true, '已连接');

                                // 计算 FPS
                                frameCount++;
                                const now = Date.now();
                                if (now - lastFpsTime >= 1000) {{
                                    const fps = Math.round(frameCount * 1000 / (now - lastFpsTime));
                                    statusText.textContent = `已连接 ({{fps}} FPS)`;
                                    frameCount = 0;
                                    lastFpsTime = now;
                                }}
                            }};

                            imageElement.src = url;
                        }}

                        // 继续读取下一帧
                        read();
                    }});
                }}

                read();
                log('视频流已开始');
                setStatus(false, '接收视频流...');
            }}).catch(error => {{
                log('视频流错误: ' + error.message);
                setStatus(false, '连接失败');

                // 5秒后重连
                setTimeout(connectVideoStream, 5000);
            }});
        }}

        // 启动
        connectVideoStream();
    </script>
</body>
</html>"#, signaling_url = signaling_url)
}
