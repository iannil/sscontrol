# sscontrol

A headless remote desktop application built with Rust, featuring WebRTC P2P communication, cross-platform support, and system service integration.

[中文文档](README_CN.md)

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg)]()

## Features

- Screen Capture - High-performance platform-native screen capture
- Remote Input - Mouse movement, clicks, scroll, and keyboard input
- WebRTC P2P - Low-latency peer-to-peer video streaming
- Security - API key authentication, HMAC-SHA256 tokens, TLS/DTLS encryption
- Service Mode - Run as a background system service (LaunchAgent/systemd/Windows Service)
- Device Discovery - Automatic LAN device discovery via mDNS
- Connection Codes - Quick pairing with 6-digit codes
- Signaling Server - Self-hosted or Cloudflare Worker-based signaling

## Platform Support

| Platform | Screen Capture | Input Simulation | System Service |
|----------|----------------|------------------|----------------|
| macOS    | CGDisplay API  | CGEvent          | LaunchAgent    |
| Windows  | DXGI / GDI     | SendInput        | Windows Service|
| Linux    | Planned        | Planned          | systemd        |

## Quick Start

### Prerequisites

- Rust 1.70 or later
- FFmpeg development libraries (required for h264 feature)
- Platform-specific requirements:
  - macOS: Screen Recording + Accessibility permissions
  - Windows: Administrator privileges for service installation
  - Linux: systemd for service management

#### Installing FFmpeg

**macOS:**
```bash
brew install ffmpeg
```

**Ubuntu/Debian:**
```bash
sudo apt install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev
```

**Windows:**
```batch
# Using vcpkg
vcpkg install ffmpeg:x64-windows
set FFMPEG_DIR=C:\vcpkg\installed\x64-windows

# Or download from https://github.com/BtbN/FFmpeg-Builds/releases
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/iannil/sscontrol.git
cd sscontrol

# Build release version with full features
cargo build --release --features "h264,webrtc,security,service"

# (Optional) Install the binary
sudo cp target/release/sscontrol /usr/local/bin/
```

#### Platform-Specific Build Scripts

**macOS:**
```bash
./build-macos.sh
```

**Windows:**
```batch
build-windows.bat
```

### Installation Scripts

macOS:

```bash
./scripts/install_macos.sh
```

Linux:

```bash
sudo ./scripts/install_linux.sh
```

Windows (PowerShell as Administrator):

```powershell
.\scripts\install_windows.ps1
```

## Usage

### Simple LAN Mode (Recommended for Local Network)

The simplest way to use sscontrol is the host/connect mode for local network connections:

**On the host machine (share your screen):**

```bash
# Start screen sharing
sscontrol host --port 9527

# With API key authentication
sscontrol host --port 9527 --api-key your-secret-key
```

**On the viewer machine (view remote screen):**

```bash
# Connect to host (opens web browser automatically)
sscontrol connect --ip 192.168.1.100 --port 9527
```

Or simply open a browser and navigate to `http://<host-ip>:9527`

### Basic Commands

```bash
# Run with default configuration
sscontrol

# Specify server address
sscontrol --server ws://localhost:8080

# Set custom device ID and frame rate
sscontrol --device-id my-device --fps 30

# Verbose logging
sscontrol -vv
```

### Service Management

```bash
# Install as a system service
sscontrol service install

# Start/stop/status
sscontrol service start
sscontrol service stop
sscontrol service status

# Uninstall the service
sscontrol service uninstall
```

### Deploy Signaling Server

```bash
# Deploy to a remote server via SSH
sscontrol deploy signaling --host 1.2.3.4 --user root --port 8443

# With TLS (Let's Encrypt)
sscontrol deploy signaling --host 1.2.3.4 --tls --domain example.com

# Check status / Uninstall
sscontrol deploy status --host 1.2.3.4
sscontrol deploy uninstall --host 1.2.3.4
```

## Configuration

Default config location: `~/.config/sscontrol/config.toml`

```toml
[server]
url = "ws://localhost:8080"

[capture]
fps = 30

[security]
# Use environment variable SSCONTROL_API_KEY instead
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

### Environment Variables

| Variable | Description |
| ---------- | ------------- |
| `SSCONTROL_API_KEY` | API key for authentication |
| `SSCONTROL_TLS_CERT` | Path to TLS certificate file |
| `SSCONTROL_TLS_KEY` | Path to TLS private key file |
| `RUST_LOG` | Log level (e.g., `info,sscontrol=debug`) |

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│                         Host Agent                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │  Capture │───→│  Encoder │───→│  Network │───→│  WebRTC  │  │
│  │ (macOS/  │    │ (H.264/  │    │(WebSocket│    │  (P2P)   │  │
│  │ Windows) │    │  Simple) │    │  Client) │    │          │  │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘  │
│       ↑                                               ↓        │
│       │                                    ┌──────────────┐    │
│       │                                    │ Input Handler│    │
│       │                                    │ (Mouse/Key)  │    │
│       │                                    └──────────────┘    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Security Layer                       │   │
│  │         (API Key Auth / HMAC Tokens / TLS)              │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Signaling Server                           │
│            (WebSocket + WebRTC ICE / Cloudflare Worker)         │
└─────────────────────────────────────────────────────────────────┘
```

## Feature Flags

| Feature | Description | Dependencies |
| --------- | ------------- | -------------- |
| `h264` | H.264 video encoding | FFmpeg |
| `webrtc` | WebRTC P2P support | webrtc-rs |
| `security` | TLS and authentication | rustls |
| `service` | System service integration | (default) |
| `discovery` | mDNS device discovery | mdns-sd |
| `deploy` | Remote signaling server deployment | ssh2 |

### Build Examples

```bash
# Default build
cargo build --release

# With H.264 encoding (requires FFmpeg)
cargo build --release --features h264

# All features
cargo build --release --features "h264,webrtc,security,service,discovery,deploy"
```

## Performance

Tested on macOS with 4K resolution (3840x2160):

| Metric | Result |
| -------- | -------- |
| Average capture time | ~51 ms |
| Average encode time | ~1.6 ms |
| Maximum frame rate | ~19 FPS (raw) |
| Bandwidth (raw) | ~600 MB/s |
| Bandwidth (H.264) | ~2-5 Mbps |

## Project Structure

```text
sscontrol/
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library entry
│   ├── config.rs            # Configuration management
│   ├── bin/                 # Additional binaries
│   │   └── signaling_server.rs  # Standalone signaling server
│   ├── capture/             # Screen capture (macOS/Windows)
│   ├── encoder/             # Video encoding (Simple/H.264)
│   ├── input/               # Input simulation (Mouse/Keyboard)
│   ├── network/             # WebSocket client
│   ├── webrtc/              # WebRTC peer connection
│   ├── security/            # Auth & TLS
│   ├── service/             # System service integration
│   └── deploy/              # Remote deployment
├── scripts/                 # Installation scripts
├── cloudflare-worker/       # Cloudflare Worker signaling server
└── docs/                    # Documentation
```

## Documentation

| Document | Description |
| ---------- | ------------- |
| [Architecture](./docs/architecture/overview.md) | System design |
| [Deployment Guide](./docs/deployment-guide.md) | Deployment instructions |
| [Troubleshooting](./docs/troubleshooting/common-issues.md) | Common issues |
| [Operations Runbook](./docs/operations/runbook.md) | Operations guide |

## Known Issues

| Issue | Priority | Status |
| ------- | ---------- | -------- |
| H.264 encoder requires FFmpeg | P2 | Documented |
| macOS scroll wheel support limited | P3 | Planned |
| Linux screen capture not implemented | P2 | Planned |

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run tests
cargo test

# Run all checks before commit
cargo fmt && cargo clippy && cargo test
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Rust](https://www.rust-lang.org/) - Programming language
- [webrtc-rs](https://github.com/webrtc-rs/webrtc) - WebRTC implementation
- [Tokio](https://tokio.rs/) - Async runtime
- [FFmpeg](https://ffmpeg.org/) - Video encoding (optional)
