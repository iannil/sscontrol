# 项目进度

**最后更新**: 2026-01-21

## 当前阶段

**状态**: 项目已完成，进入维护和优化阶段

所有 5 个开发阶段已全部完成。

---

## 整体进度

| 阶段 | 状态 | 进度 | 说明 |
|------|------|------|------|
| Phase 0 - 规划设计 | ✅ 已完成 | 100% | 架构设计完成，项目已初始化 |
| Phase 1 - MVP 屏幕捕获 | ✅ 已完成 | 100% | 所有核心功能已实现 |
| Phase 2 - 鼠标控制 | ✅ 已完成 | 100% | 鼠标控制功能已实现 |
| Phase 3 - WebRTC 优化 | ✅ 已完成 | 100% | WebRTC 信令和视频轨道已完成 |
| Phase 4 - 安全特性 | ✅ 已完成 | 100% | 认证与加密已完成 |
| Phase 5 - 系统服务打包 | ✅ 已完成 | 100% | Windows Service / macOS LaunchAgent / Linux systemd |

---

## 项目结构

```
sscontrol/
├── Cargo.toml                   # Rust 项目配置
├── config.toml.example          # 配置文件示例
├── src/
│   ├── lib.rs                   # 库入口
│   ├── main.rs                  # 程序入口
│   ├── config.rs                # 配置管理
│   ├── capture/
│   │   ├── mod.rs               # 捕获模块抽象
│   │   ├── macos.rs             # macOS CGDisplay 实现
│   │   └── windows.rs           # Windows GDI 实现
│   ├── encoder/
│   │   └── mod.rs               # 编码器 (SimpleEncoder + H264Encoder)
│   ├── input/
│   │   ├── mod.rs               # 输入模块抽象
│   │   ├── macos.rs             # macOS CGEvent 实现
│   │   └── windows.rs           # Windows SendInput 实现
│   ├── security/                # 安全模块
│   │   ├── mod.rs               # 安全模块入口
│   │   ├── auth.rs              # API Key 认证
│   │   ├── tls.rs               # TLS 配置
│   │   └── token.rs             # Token 管理 (防重放)
│   ├── service/                 # 系统服务模块
│   │   ├── mod.rs               # 服务抽象 (ServiceController trait)
│   │   ├── macos.rs             # macOS LaunchAgent 实现
│   │   ├── linux.rs             # Linux systemd 实现
│   │   └── windows.rs           # Windows Service 实现
│   ├── webrtc/
│   │   ├── mod.rs               # WebRTC 模块 (PeerConnection trait)
│   │   ├── peer_connection.rs   # RealPeerConnection 实现 (webrtc-rs)
│   │   ├── signaling.rs         # 信令客户端
│   │   └── video_track.rs       # RTP 视频轨道 (VideoTrack/VideoSender)
│   └── network/
│       └── mod.rs               # WebSocket 客户端 (含自动重连和输入事件处理)
├── examples/
│   ├── test_server.rs           # WebSocket 测试服务器
│   ├── test_capture.rs          # 屏幕捕获测试
│   ├── test_encoder.rs          # 编码器测试
│   ├── benchmark.rs             # 性能基准测试
│   ├── webrtc_example.rs        # WebRTC 使用示例
│   ├── signaling_server.rs      # 信令服务器 (含认证支持)
│   ├── webrtc_client.rs         # 完整 WebRTC 客户端示例
│   └── secure_server.rs         # 安全部务器示例
├── scripts/                     # 安装脚本
│   ├── install_macos.sh         # macOS 安装脚本
│   ├── install_linux.sh         # Linux 安装脚本
│   └── install_windows.ps1      # Windows 安装脚本
└── docs/                        # 文档目录
```

---

## 使用指南

### macOS

```bash
# 编译发布版本
cargo build --release

# 安装服务
./scripts/install_macos.sh

# 管理服务
./target/release/sscontrol service start
./target/release/sscontrol service stop
./target/release/sscontrol service status

# 卸载服务
./scripts/install_macos.sh remove
```

### Linux

```bash
# 编译发布版本
cargo build --release

# 安装服务 (需要 root 权限)
sudo ./scripts/install_linux.sh

# 管理服务
sudo systemctl start sscontrol
sudo systemctl stop sscontrol
sudo systemctl status sscontrol

# 查看日志
sudo journalctl -u sscontrol -f

# 卸载服务
sudo ./scripts/install_linux.sh remove
```

### Windows

```powershell
# 编译发布版本
cargo build --release

# 安装服务 (需要管理员权限)
.\scripts\install_windows.ps1

# 管理服务
.\target\release\sscontrol.exe service start
.\target\release\sscontrol.exe service stop
.\target\release\sscontrol.exe service status

# 卸载服务
.\scripts\install_windows.ps1 remove
```

---

## 已知问题

| ID | 描述 | 优先级 | 状态 |
|----|------|--------|------|
| I001 | 编码器使用 SimpleEncoder 传输原始数据，带宽需求高 | P1 | 待优化 |
| I002 | macOS 滚轮事件支持有限 | P3 | 待实现 |
| I003 | Windows 捕获使用 GDI，性能可优化 | P3 | 待优化 |

---

## 未来计划

1. 启用 H.264 编码器降低带宽需求
2. 评估端到端延迟性能
3. 实现 STUN/TURN 服务器配置支持
4. 添加 Linux 平台屏幕捕获支持

---

## 性能基准测试结果

**测试环境**: macOS, 4K 分辨率 (3840x2160)

| 指标 | 结果 |
|------|------|
| 平均捕获时间 | ~51 ms |
| 平均编码时间 | ~1.6 ms |
| 最大帧率 | ~19 FPS |
| 带宽需求 | ~600 MB/s (原始数据) |

**评估**: 帧率满足基本需求，后续通过 H.264 编码可大幅降低带宽需求。
