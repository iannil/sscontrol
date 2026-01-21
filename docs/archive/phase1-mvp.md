# Phase 1: MVP 详细设计

## 目标

实现最小可行产品 (MVP)：单向屏幕捕获和视频传输

## 功能范围

### 包含
- 屏幕捕获 (30fps)
- H.264 视频编码
- WebSocket 视频流传输
- 基础配置管理
- 命令行界面

### 不包含
- 鼠标/键盘控制
- WebRTC (使用简单 WebSocket)
- 认证/加密
- 系统服务安装

## 模块设计

### 目录结构

```
src/
├── main.rs              # 入口
├── config.rs            # 配置管理
├── capture/
│   ├── mod.rs           # 模块导出
│   ├── windows.rs       # Windows 实现
│   └── macos.rs         # macOS 实现
├── encoder/
│   ├── mod.rs           # 模块导出
│   └── h264.rs          # H.264 编码器
└── network/
    ├── mod.rs           # 模块导出
    └── ws_client.rs     # WebSocket 客户端
```

### 配置模块 (`config.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use anyhow::Result;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub capture: CaptureConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub url: String,
    pub device_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CaptureConfig {
    pub fps: u32,
    pub screen_index: Option<u32>,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Config {
            server: ServerConfig {
                url: "ws://localhost:8080".to_string(),
                device_id: uuid::Uuid::new_v4().to_string(),
            },
            capture: CaptureConfig {
                fps: 30,
                screen_index: None,
            },
        }
    }
}
```

### 捕获模块 (`capture/mod.rs`)

```rust
pub trait Capturer: Send + Sync {
    fn capture(&mut self) -> Result<Frame>;
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,  // RGBA
}

#[cfg(target_os = "windows")]
pub use windows::DesktopCapturer;

#[cfg(target_os = "macos")]
pub use macos::ScreenCapturer;

pub fn create_capturer() -> Result<Box<dyn Capturer>> {
    #[cfg(target_os = "windows")]
    return Ok(Box::new(DesktopCapturer::new()?));

    #[cfg(target_os = "macos")]
    return Ok(Box::new(ScreenCapturer::new()?));

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return Err(anyhow::anyhow!("Unsupported platform"));
}
```

### 编码模块 (`encoder/mod.rs`)

```rust
use crate::capture::Frame;
use anyhow::Result;

pub trait Encoder: Send + Sync {
    fn encode(&mut self, frame: &Frame) -> Result<Vec<u8>>;
}

pub struct H264Encoder {
    encoder: ffmpeg::encoder::Video,
    // ... 内部状态
}

impl H264Encoder {
    pub fn new(width: u32, height: u32, fps: u32) -> Result<Self> {
        // 初始化 FFmpeg H.264 编码器
        // ...
    }
}

impl Encoder for H264Encoder {
    fn encode(&mut self, frame: &Frame) -> Result<Vec<u8>> {
        // 编码帧为 H.264
        // ...
    }
}
```

### 网络模块 (`network/mod.rs`)

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use anyhow::Result;

pub struct VideoClient {
    tx: futures_util::stream::SplitSink<WebSocketStream<...>, Message>,
}

impl VideoClient {
    pub async fn connect(url: &str) -> Result<Self> {
        let (ws_stream, _) = connect_async(url).await?;
        let (tx, rx) = ws_stream.split();
        Ok(Self { tx })
    }

    pub async fn send_frame(&mut self, data: Vec<u8>) -> Result<()> {
        self.tx.send(Message::Binary(data)).await?;
        Ok(())
    }
}
```

### 主入口 (`main.rs`)

```rust
mod config;
mod capture;
mod encoder;
mod network;

use anyhow::Result;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // 加载配置
    let config = config::Config::load("config.toml")
        .unwrap_or_else(|_| config::Config::default());

    println!("Remote Desktop Service Started");
    println!("Device ID: {}", config.server.device_id);

    // 初始化组件
    let mut capturer = capture::create_capturer()?;
    let mut encoder = encoder::H264Encoder::new(1920, 1080, 30)?;
    let mut client = network::VideoClient::connect(&config.server.url).await?;

    // 主循环
    let frame_interval = Duration::from_millis(1000 / config.capture.fps as u64);

    loop {
        let start = std::time::Instant::now();

        // 捕获
        let frame = capturer.capture()?;

        // 编码
        let encoded = encoder.encode(&frame)?;

        // 发送
        client.send_frame(encoded).await?;

        // 帧率控制
        let elapsed = start.elapsed();
        if elapsed < frame_interval {
            tokio::time::sleep(frame_interval - elapsed).await;
        }
    }
}
```

## 开发任务

### 任务清单

- [ ] 1.1 初始化 Cargo 项目
- [ ] 1.2 创建目录结构
- [ ] 1.3 实现配置模块
- [ ] 1.4 实现 Windows 屏幕捕获
- [ ] 1.5 实现 macOS 屏幕捕获
- [ ] 1.6 实现 H.264 编码器
- [ ] 1.7 实现 WebSocket 客户端
- [ ] 1.8 集成主循环
- [ ] 1.9 编写测试用例
- [ ] 1.10 性能测试和优化

## 验收标准

- [x] 可以成功捕获屏幕
- [ ] 以 30fps 稳定传输
- [ ] 延迟 < 500ms
- [ ] CPU 占用 < 30%
- [ ] 内存占用 < 200MB

## 已知问题

暂无

## 下一步

Phase 2 将添加鼠标控制功能。
