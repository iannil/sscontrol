# 架构设计文档

## 系统架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                          控制端 (Client)                          │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────┐ │
│  │   浏览器 / WASM  │    │   Native GUI    │    │  移动端 App  │ │
│  └────────┬────────┘    └────────┬────────┘    └──────┬──────┘ │
└───────────┼──────────────────────┼──────────────────────┼────────┘
            │                      │                      │
            └──────────────────────┼──────────────────────┘
                                   │
                          ┌────────▼────────┐
                          │  WebRTC P2P 连接  │
                          │  (视频流 + 数据通道) │
                          └────────┬────────┘
                                   │
            ┌──────────────────────┼──────────────────────┐
            │                      │                      │
┌───────────▼────────┐    ┌───────▼───────┐    ┌────────▼────────┐
│   STUN 服务器       │    │  TURN 服务器   │    │  信令服务器       │
│  (NAT 穿透)        │    │  (中继备选)     │    │ (SDP 交换)       │
└────────────────────┘    └───────────────┘    └────────┬────────┘
                                                        │
            ┌───────────────────────────────────────────┼──────────────┐
            │                                           │              │
┌───────────▼─────────────┐              ┌─────────────▼──────────────┐
│       受控端 (Host)        │              │       其他受控端             │
│  ┌────────────────────┐  │              │  (可同时管理多台设备)         │
│  │   屏幕捕获模块       │  │              └──────────────────────────┘
│  │  - Desktop Dup API  │  │
│  │  - ScreenCaptureKit│  │
│  └──────────┬─────────┘  │
│  ┌──────────▼─────────┐  │
│  │   视频编码模块       │  │
│  │  - H.264/VP8/VP9   │  │
│  │  - 硬件加速        │  │
│  └──────────┬─────────┘  │
│  ┌──────────▼─────────┐  │
│  │  WebRTC PeerConn   │  │
│  └──────────┬─────────┘  │
│  ┌──────────▼─────────┐  │
│  │   输入模拟模块       │  │
│  │  - SendInput       │  │
│  │  - CGEvent         │  │
│  └────────────────────┘  │
└──────────────────────────┘
```

## 模块设计

### 1. 屏幕捕获模块 (Capture)

**职责**: 获取屏幕原始图像数据

**接口设计**:
```rust
pub trait Capturer: Send + Sync {
    fn capture(&mut self) -> Result<Frame>;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,  // RGBA 格式
    pub timestamp: u64,
}
```

**平台实现**:
- `capture::windows::DesktopDuplication`
- `capture::macos::ScreenCaptureKit`

---

### 2. 视频编码模块 (Encoder)

**职责**: 将原始帧压缩为视频流

**接口设计**:
```rust
pub trait Encoder: Send + Sync {
    fn encode(&mut self, frame: &Frame) -> Result<EncodedPacket>;
    fn request_key_frame(&mut self) -> Result<()>;
    fn set_bitrate(&mut self, bitrate: u32);
}

pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub is_key_frame: bool,
    pub timestamp: u64,
}
```

**编码参数**:
- 编码格式: H.264 (Baseline Profile)
- 码率模式: VBR (可变比特率)
- 目标帧率: 60fps
- 关键帧间隔: 2秒 (网络差时动态调整)

---

### 3. 网络传输模块 (Network)

**职责**: 管理 WebRTC 连接和数据传输

**组件**:
- **SignalingClient**: WebSocket 信令客户端
- **PeerConnectionManager**: WebRTC 连接管理
- **DataChannel**: 输入指令数据通道

**接口设计**:
```rust
pub struct NetworkManager {
    signaling: SignalingClient,
    peer_connection: PeerConnection,
    data_channel: Option<DataChannel>,
}

impl NetworkManager {
    pub async fn connect(&mut self, server_url: &str) -> Result<()>;
    pub async fn create_offer(&mut self) -> Result<String>;
    pub async fn set_answer(&mut self, answer: &str) -> Result<()>;
    pub fn send_video(&mut self, packet: EncodedPacket) -> Result<()>;
    pub fn send_input(&mut self, input: InputEvent) -> Result<()>;
}
```

---

### 4. 输入模拟模块 (Input)

**职责**: 在本地模拟鼠标和键盘操作

**接口设计**:
```rust
pub trait InputSimulator: Send + Sync {
    fn mouse_move(&mut self, x: f64, y: f64) -> Result<()>;
    fn mouse_click(&mut self, button: MouseButton, pressed: bool) -> Result<()>;
    fn mouse_wheel(&mut self, delta: i32) -> Result<()>;
    fn key_event(&mut self, key: KeyCode, pressed: bool) -> Result<()>;
}

pub enum InputEvent {
    MouseMove { x: f64, y: f64 },  // 归一化坐标 0.0-1.0
    MouseClick { button: MouseButton, pressed: bool },
    MouseWheel { delta: i32 },
    KeyEvent { key: KeyCode, pressed: bool },
}
```

---

### 5. 配置管理模块 (Config)

**职责**: 管理应用配置

**配置结构** (`config.toml`):
```toml
[server]
url = "wss://signal.example.com"
device_id = "auto-generated"  # 或手动指定

[video]
encoder = "h264"  # h264 | vp8 | vp9
max_bitrate = 2000  # kbps
target_fps = 60

[security]
password = "optional-password"
require_auth = true

[logging]
level = "info"
file = "/var/log/sscontrol.log"
```

---

## 数据流设计

### 视频流 (下行)

```
屏幕捕获 → 原始帧(RGBA) → 视频编码 → H.264包 → RTP → WebRTC → 控制端
   ↓           ↓              ↓            ↓        ↓
 60fps      ~200MB/s      ~2MB/s      ~2Mbps    显示
```

### 输入流 (上行)

```
控制端输入 → JSON 序列化 → Data Channel → SCTP → WebRTC → 反序列化 → 输入模拟
   ↓              ↓              ↓           ↓           ↓
  鼠标/键盘      ~50B         ~50B       ~50B      系统事件
```

---

## 线程模型

```
┌─────────────────────────────────────────────────────┐
│                    主线程 (Tokio)                    │
│  - 信令 WebSocket I/O                                │
│  - 配置管理                                          │
│  - 信号处理 (Ctrl+C)                                  │
└─────────────────────────────────────────────────────┘
           │                  │
           ▼                  ▼
┌──────────────────┐  ┌──────────────────┐
│  屏幕捕获线程     │  │  WebRTC 线程池    │
│  - 捕获屏幕       │  │  - RTP 发送       │
│  - 帧预处理       │  │  - ICE 处理       │
│  - 60fps 循环     │  │  - DTLS 握手      │
└──────────────────┘  └──────────────────┘
```

---

## 错误处理策略

| 错误类型 | 处理方式 |
|----------|----------|
| 屏幕捕获失败 | 记录日志，跳过当前帧，继续捕获 |
| 编码失败 | 请求关键帧，降低码率 |
| 网络断开 | 自动重连 (指数退避) |
| 权限不足 | 启动时检测，日志报错，退出 |
| 配置错误 | 使用默认配置，记录警告 |
