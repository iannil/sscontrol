# sscontrol

基于 Rust 构建的无界面远程桌面应用，支持 WebRTC P2P 通信、跨平台和系统服务集成。

[English](README.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg)]()

## 功能特性

- 屏幕捕获 - 高性能平台原生屏幕捕获
- 远程输入 - 鼠标移动、点击、滚轮和键盘输入
- WebRTC P2P - 低延迟点对点视频传输
- 安全机制 - API 密钥认证、HMAC-SHA256 令牌、TLS/DTLS 加密
- 服务模式 - 作为后台系统服务运行（LaunchAgent/systemd/Windows Service）
- 设备发现 - 通过 mDNS 自动发现局域网设备
- 连接码 - 6 位数字快速配对
- 信令服务 - 支持自托管或基于 Cloudflare Worker 的信令服务器

## 平台支持

| 平台 | 屏幕捕获 | 输入模拟 | 系统服务 |
| ------ | ---------- | ---------- | ---------- |
| macOS | CGDisplay API | CGEvent | LaunchAgent |
| Windows | DXGI / GDI | SendInput | Windows Service |
| Linux | 计划中 | 计划中 | systemd |

## 快速开始

### 前置要求

- Rust 1.70 或更高版本
- 平台特定要求：
  - macOS: 屏幕录制 + 辅助功能权限
  - Windows: 安装服务需要管理员权限
  - Linux: systemd 用于服务管理

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/iannil/sscontrol.git
cd sscontrol

# 构建发布版本
cargo build --release

# （可选）安装二进制文件
sudo cp target/release/sscontrol /usr/local/bin/
```

### 使用安装脚本

macOS:

```bash
./scripts/install_macos.sh
```

Linux:

```bash
sudo ./scripts/install_linux.sh
```

Windows（以管理员身份运行 PowerShell）:

```powershell
.\scripts\install_windows.ps1
```

## 使用方法

### 基本命令

```bash
# 使用默认配置运行
sscontrol

# 指定服务器地址
sscontrol --server ws://localhost:8080

# 设置自定义设备 ID 和帧率
sscontrol --device-id my-device --fps 30

# 详细日志输出
sscontrol -vv
```

### 服务管理

```bash
# 安装为系统服务
sscontrol service install

# 启动/停止/状态
sscontrol service start
sscontrol service stop
sscontrol service status

# 卸载服务
sscontrol service uninstall
```

### 部署信令服务器

```bash
# 通过 SSH 部署到远程服务器
sscontrol deploy signaling --host 1.2.3.4 --user root --port 8443

# 启用 TLS（Let's Encrypt）
sscontrol deploy signaling --host 1.2.3.4 --tls --domain example.com

# 检查状态 / 卸载
sscontrol deploy status --host 1.2.3.4
sscontrol deploy uninstall --host 1.2.3.4
```

## 配置

默认配置文件位于 `~/.config/sscontrol/config.toml`

```toml
[server]
url = "ws://localhost:8080"

[capture]
fps = 30

[security]
# 推荐使用环境变量 SSCONTROL_API_KEY
# api_key = "your-secret-api-key"
require_tls = false
token_ttl = 300

[webrtc]
stun_servers = ["stun:stun.l.google.com:19302"]
ice_transport_policy = "all"

[discovery]
enabled = true
connection_code_ttl = 300

[signaling]
provider = "cloudflare"
```

### 环境变量

| 变量 | 描述 |
| ------ | ------ |
| `SSCONTROL_API_KEY` | 用于身份认证的 API 密钥 |
| `SSCONTROL_TLS_CERT` | TLS 证书文件路径 |
| `SSCONTROL_TLS_KEY` | TLS 私钥文件路径 |
| `RUST_LOG` | 日志级别（如 `info,sscontrol=debug`） |

## 架构

```text
┌─────────────────────────────────────────────────────────────────┐
│                         主机代理                                 │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │ 屏幕捕获  │───→│   编码器  │───→│  网络传输 │───→│  WebRTC  │  │
│  │ (macOS/  │    │ (H.264/  │    │(WebSocket│    │  (P2P)   │  │
│  │ Windows) │    │  Simple) │    │  客户端)  │    │          │  │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘  │
│       ↑                                               ↓        │
│       │                                    ┌──────────────┐    │
│       │                                    │  输入处理器   │    │
│       │                                    │  (鼠标/键盘)  │    │
│       │                                    └──────────────┘    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                       安全层                             │   │
│  │          (API 密钥认证 / HMAC 令牌 / TLS)                 │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        信令服务器                                │
│             (WebSocket + WebRTC ICE / Cloudflare Worker)        │
└─────────────────────────────────────────────────────────────────┘
```

## Feature 标志

| Feature | 描述 | 依赖 |
| --------- | ------ | ------ |
| `h264` | H.264 视频编码 | FFmpeg |
| `webrtc` | WebRTC P2P 支持 | webrtc-rs |
| `security` | TLS 和认证功能 | rustls |
| `service` | 系统服务集成 | （默认启用） |
| `discovery` | mDNS 设备发现 | mdns-sd |
| `deploy` | 远程信令服务器部署 | ssh2 |

### 构建示例

```bash
# 默认构建
cargo build --release

# 启用 H.264 编码（需要安装 FFmpeg）
cargo build --release --features h264

# 启用所有特性
cargo build --release --features "h264,webrtc,security,service,discovery,deploy"
```

## 示例程序

```bash
# WebSocket 测试服务器
cargo run --example test_server

# 屏幕捕获测试
cargo run --example test_capture

# 端到端延迟测试
cargo run --example latency_test

# WebRTC 演示（需要 webrtc 特性）
cargo run --example webrtc_example --features webrtc

# 信令服务器
cargo run --example signaling_server
```

## 性能

测试环境：macOS，4K 分辨率（3840x2160）

| 指标 | 结果 |
| ------ | ------ |
| 平均捕获时间 | ~51 ms |
| 平均编码时间 | ~1.6 ms |
| 最大帧率 | ~19 FPS（原始） |
| 带宽占用（原始） | ~600 MB/s |
| 带宽占用（H.264） | ~2-5 Mbps |

## 项目结构

```text
sscontrol/
├── src/
│   ├── main.rs              # 程序入口
│   ├── lib.rs               # 库入口
│   ├── config.rs            # 配置管理
│   ├── capture/             # 屏幕捕获（macOS/Windows）
│   ├── encoder/             # 视频编码（Simple/H.264）
│   ├── input/               # 输入模拟（鼠标/键盘）
│   ├── network/             # WebSocket 客户端
│   ├── webrtc/              # WebRTC 对等连接
│   ├── security/            # 认证与 TLS
│   ├── service/             # 系统服务集成
│   └── deploy/              # 远程部署
├── examples/                # 示例程序
├── scripts/                 # 安装脚本
├── cloudflare-worker/       # Cloudflare Worker 信令服务器
└── docs/                    # 文档
```

## 文档

| 文档 | 描述 |
| ------ | ------ |
| [架构设计](./docs/architecture/overview.md) | 系统设计 |
| [部署指南](./docs/deployment-guide.md) | 部署说明 |
| [常见问题](./docs/troubleshooting/common-issues.md) | 问题排查 |
| [运维手册](./docs/operations/runbook.md) | 运维指南 |

## 已知问题

| 问题 | 优先级 | 状态 |
| ------ | -------- | ------ |
| H.264 编码器需要安装 FFmpeg | P2 | 已记录 |
| macOS 滚轮事件支持有限 | P3 | 计划中 |
| Linux 屏幕捕获尚未实现 | P2 | 计划中 |

## 贡献

欢迎贡献！请随时提交 Pull Request。

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

### 开发

```bash
# 格式化代码
cargo fmt

# 运行代码检查
cargo clippy

# 运行测试
cargo test

# 提交前运行所有检查
cargo fmt && cargo clippy && cargo test
```

## 开源许可

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。

## 致谢

- [Rust](https://www.rust-lang.org/) - 编程语言
- [webrtc-rs](https://github.com/webrtc-rs/webrtc) - WebRTC 实现
- [Tokio](https://tokio.rs/) - 异步运行时
- [FFmpeg](https://ffmpeg.org/) - 视频编码（可选）
