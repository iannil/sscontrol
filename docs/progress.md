# 项目进度

**最后更新**: 2026-01-22

## 当前阶段

**状态**: Phase 7 进行中

Phase 7.1 和 7.2 已完成，Phase 7.3 (Web 客户端) 待开发。

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
| Phase 6 - 稳定性与性能优化 | ✅ 已完成 | 100% | 错误处理、WebRTC 配置、DXGI 捕获、延迟测试 |
| Phase 7 - H.264 与 Web 客户端 | 🚧 进行中 | 66% | H.264 修复完成，代码稳定性完成，Web 客户端待开发 |

---

## Phase 7 完成内容

### 7.1 H.264 编码器修复 ✅

**问题**: `src/encoder/mod.rs` 中存在借用冲突，导致 H.264 编码器无法编译

**修复**: 重构 `encode()` 方法，将 YUV 转换和编码操作分离到不同作用域：
```rust
// 阶段 1: YUV 转换 (使用 sws_context)
let yuv_frame = self.rgba_to_yuv420p_frame(...)?;
// 阶段 2: 编码 (使用 encoder)
let encoder = self.encoder.as_mut().ok_or_else(...)?;
encoder.send_frame(&yuv_frame)?;
```

**验证**:
- `cargo build --features h264` 编译成功
- latency_test 使用 libx264 编码器正常运行

### 7.2 代码稳定性 ✅

修复了以下 panic/expect/unwrap 调用：

| 文件 | 修复内容 |
|------|----------|
| `src/network/mod.rs:160` | `input_receiver().expect()` → `take_input_receiver()` 返回 `Result` |
| `src/network/mod.rs:174` | `api_key.unwrap()` → `ok_or_else()` |
| `src/main.rs:323` | `ctrl_c().expect()` → `if let Err(e)` 优雅处理 |
| `src/input/macos.rs:427` | 移除未使用的 `Default` 实现 |
| `src/webrtc/signaling.rs:299,320` | `panic!()` → `unreachable!()` |

### 7.3 Web 客户端 (待开发)

计划使用 TypeScript + Vite 构建浏览器端控制客户端。

---

## Phase 6 完成内容

### 6.1 稳定性优化
- 修复 `src/service/macos.rs` 中的 unwrap 调用，改用 `ok_or_else` 处理
- 修复 `src/security/auth.rs` 中的时间戳 unwrap，改用 `map().unwrap_or(0)`
- 修复 `src/capture/mod.rs` 中的时间戳 unwrap
- 移除 `src/input/macos.rs` 中重复的 cfg 属性

### 6.2 WebRTC 配置支持
- 新增 `WebRTCConfig` 和 `TurnServerConfig` 结构
- 支持 STUN/TURN 服务器配置
- 支持 ICE 传输策略配置 ("all" 或 "relay")
- CLI 新增 `--stun`, `--turn`, `--turn-username`, `--turn-password`, `--ice-policy` 参数
- 更新 `config.toml.example` 添加 WebRTC 配置示例

### 6.3 Windows DXGI 捕获
- 新增 `src/capture/windows_dxgi.rs` 模块
- 使用 DXGI Desktop Duplication API (Windows 8+)
- 自动 fallback 到 GDI BitBlt (兼容旧系统)
- Cargo.toml 添加 Direct3D11/DXGI 依赖

### 6.4 性能评估工具
- 新增 `examples/latency_test.rs` 端到端延迟测试工具
- 支持捕获、编码各阶段延迟测量
- 统计报告：Min/Max/Mean/Median/P95/P99/StdDev
- 延迟直方图可视化

---

## 项目结构

```
sscontrol/
├── Cargo.toml                   # Rust 项目配置
├── config.toml.example          # 配置文件示例 (含 WebRTC 配置)
├── src/
│   ├── lib.rs                   # 库入口
│   ├── main.rs                  # 程序入口 (含 WebRTC CLI 参数)
│   ├── config.rs                # 配置管理 (含 WebRTCConfig)
│   ├── capture/
│   │   ├── mod.rs               # 捕获模块抽象 (自动选择最优实现)
│   │   ├── macos.rs             # macOS CGDisplay 实现
│   │   ├── windows.rs           # Windows GDI 实现 (fallback)
│   │   └── windows_dxgi.rs      # Windows DXGI 实现 (优先)
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
│   ├── latency_test.rs          # 端到端延迟测试工具
│   ├── webrtc_example.rs        # WebRTC 使用示例
│   ├── signaling_server.rs      # 信令服务器 (含认证支持)
│   ├── webrtc_client.rs         # 完整 WebRTC 客户端示例
│   └── secure_server.rs         # 安全服务器示例
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
| I001 | 编码器使用 SimpleEncoder 传输原始数据，带宽需求高 | P1 | ✅ 已修复 (H.264 可用) |
| I002 | macOS 滚轮事件支持有限 | P3 | 待实现 |
| I003 | Windows 捕获使用 GDI，性能可优化 | P3 | ✅ 已优化 (DXGI) |

---

## 未来计划

1. ~~启用 H.264 编码器降低带宽需求~~ ✅ 已完成
2. 添加 Linux 平台屏幕捕获支持
3. 实现音频捕获与传输
4. Web 客户端开发 (Phase 7.3)

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
