# sscontrol

> A headless remote desktop application built with Rust, featuring WebRTC P2P communication, cross-platform support, and system service integration.

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey.svg)]()

## Overview

sscontrol is a lightweight, headless remote desktop solution that enables screen sharing and remote control through P2P connections. It runs as a background service and supports:

- Screen capture at configurable frame rates
- Remote input control (mouse & keyboard)
- WebRTC P2P communication with low latency
- End-to-end encryption with TLS support
- Cross-platform support (macOS, Windows, Linux)
- System service integration (LaunchAgent, systemd, Windows Service)

## Project Status

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 0 | ✅ Complete | Planning & Design |
| Phase 1 | ✅ Complete | MVP Screen Capture |
| Phase 2 | ✅ Complete | Mouse Control |
| Phase 3 | ✅ Complete | WebRTC Optimization |
| Phase 4 | ✅ Complete | Security Features |
| Phase 5 | ✅ Complete | System Service Packaging |

## Features

### Core Features
- 📺 Screen Capture - High-performance screen capture using platform APIs
  - macOS: CGDisplay API
  - Windows: GDI BitBlt
  - Linux: X11 (planned)
- 🖱️ Remote Input - Mouse and keyboard control simulation
- 🌐 WebRTC - P2P video streaming with WebRTC data channels
- 🔒 Security - API key authentication, HMAC-SHA256 tokens, TLS support
- 📦 Service Mode - Run as a system service on all platforms

### Technical Highlights
- Normalized coordinates (0.0-1.0) for DPI-independent input
- Automatic reconnection on network failure
- Configurable frame rate and resolution
- H.264 encoding support (optional feature)
- Comprehensive logging with tracing

## Quick Start

### Prerequisites

- Rust 1.70 or later
- Platform-specific requirements:
  - macOS: Screen recording permission (System Settings → Privacy & Security → Screen Recording)
  - Windows: Administrator privileges for service installation
  - Linux: systemd for service management

### Installation

#### From Source

```bash
# Clone the repository
git clone https://github.com/yourname/sscontrol.git
cd sscontrol

# Build release version
cargo build --release

# (Optional) Install the binary
sudo cp target/release/sscontrol /usr/local/bin/
```

#### Using Installation Scripts

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

### Usage

#### Basic Usage

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

#### Service Management

```bash
# Install as a system service
sscontrol service install

# Start the service
sscontrol service start

# Check service status
sscontrol service status

# Stop the service
sscontrol service stop

# Uninstall the service
sscontrol service uninstall
```

#### Service Mode (Foreground)

```bash
# Run in service mode (for systemd/launchd integration)
sscontrol run
```

## Configuration

The default configuration file is located at `~/.config/sscontrol/config.toml`.

### Example Configuration

```toml
[server]
# WebSocket server address
url = "ws://localhost:8080"

# Device ID (auto-generated if empty)
# device_id = ""

[capture]
# Target frame rate
fps = 30

# Screen index (0 = primary display)
# screen_index = 0

[security]
# API Key (recommended: use environment variable SSCONTROL_API_KEY)
# api_key = "your-secret-api-key"

# TLS certificate paths (recommended: use environment variables)
# tls_cert = "/path/to/cert.pem"
# tls_key = "/path/to/key.pem"

# Require TLS for connections
require_tls = false

# Token TTL in seconds (default: 300)
token_ttl = 300
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `SSCONTROL_API_KEY` | API key for authentication |
| `SSCONTROL_TLS_CERT` | Path to TLS certificate file |
| `SSCONTROL_TLS_KEY` | Path to TLS private key file |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Host Agent                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │  Capture │───→│  Encoder  │───→│  Network │───→│  WebRTC  │  │
│  │ (macOS/  │    │ (H.264/  │    │(WebSocket│    │ (P2P)    │  │
│  │ Windows) │    │  Simple) │    │  Client) │    │          │  │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘  │
│       ↑                                     ↓                    │
│       │                            ┌──────────────┐            │
│       │                            │ Input Handler│            │
│       │                            │  (Mouse/Key) │            │
│       │                            └──────────────┘            │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Security Layer                        │   │
│  │  (API Key Auth / HMAC Tokens / TLS)                     │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Signaling Server                            │
│                  (WebSocket + WebRTC ICE)                        │
└─────────────────────────────────────────────────────────────────┘
```

## Building with Features

### Default Features

```bash
cargo build --release
```

### With H.264 Encoding

```bash
cargo build --release --features h264
```

### With WebRTC Support

```bash
cargo build --release --features webrtc
```

### With Security Features

```bash
cargo build --release --features security
```

### All Features

```bash
cargo build --release --features "h264,webrtc,security,service"
```

## Examples

The project includes several example programs:

| Example | Description | Command |
|---------|-------------|---------|
| `test_server` | WebSocket test server | `cargo run --example test_server` |
| `test_capture` | Screen capture test | `cargo run --example test_capture` |
| `test_encoder` | Encoder test | `cargo run --example test_encoder` |
| `benchmark` | Performance benchmark | `cargo run --example benchmark` |
| `signaling_server` | WebRTC signaling server | `cargo run --example signaling_server` |
| `webrtc_client` | WebRTC client example | `cargo run --example webrtc_client --features webrtc` |
| `secure_server` | Secure server with auth | `cargo run --example secure_server --features security` |

## Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test capture::tests
```

## Performance

Tested on macOS, 4K resolution (3840×2160):

| Metric | Result |
|--------|--------|
| Average capture time | ~51 ms |
| Average encode time | ~1.6 ms |
| Maximum frame rate | ~19 FPS (unoptimized) |
| Bandwidth (raw) | ~600 MB/s |

> Note: H.264 encoding significantly reduces bandwidth requirements.

## Tech Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 2021 |
| Runtime | Tokio (async) |
| Network | tokio-tungstenite (WebSocket), webrtc-rs 0.12 |
| Encoding | SimpleEncoder, H.264 (optional) |
| Security | HMAC-SHA256, rustls (TLS) |
| macOS APIs | CGDisplay, Core Graphics, CGEvent |
| Windows APIs | GDI, SendInput, Windows Service |

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](./docs/architecture/overview.md) | System architecture and module design |
| [Setup Guide](./docs/implementation/setup.md) | Development environment setup |
| [Roadmap](./docs/roadmap.md) | Development roadmap |
| [Progress](./docs/progress.md) | Detailed project progress |
| [FAQ](./docs/troubleshooting/common-issues.md) | Troubleshooting guide |

## Known Issues

| ID | Issue | Priority |
|----|-------|----------|
| T001 | H.264 encoder not enabled by default | P1 |
| T002 | macOS scroll wheel support limited | P3 |
| T003 | Windows capture uses GDI (not Desktop Duplication API) | P3 |

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Development

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Build documentation
cargo doc --open
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- WebRTC implementation by [webrtc-rs](https://github.com/webrtc-rs/webrtc)
- Async runtime by [Tokio](https://tokio.rs/)

---

中文文档: [README_ZH.md](README_ZH.md)
