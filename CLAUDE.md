# CLAUDE.md

此文件为 Claude Code (claude.ai/code) 在此仓库中工作时提供指导。

## 项目概述

**sscontrol** 是一个基于 Rust 构建的无界面（Headless）远程桌面应用。采用 Host + Client 架构，使用 WebRTC 进行 P2P 通信。

**当前状态**: 项目已完成，进入维护和优化阶段

## 快速链接

| 文档 | 路径 |
| ------ | ------ |
| 项目进度 | `docs/progress.md` |
| 开发路线图 | `docs/roadmap.md` |
| 架构设计 | `docs/architecture/overview.md` |
| 部署指南 | `docs/deployment-guide.md` |
| 生产检查清单 | `docs/production-checklist.md` |
| 运维手册 | `docs/operations/runbook.md` |
| 快速参考 | `docs/operations/quick-ref.md` |
| 常见问题 | `docs/troubleshooting/common-issues.md` |

## 开发阶段

| 阶段 | 状态 |
| ------ | ------ |
| Phase 0 - 规划设计 | ✅ 已完成 |
| Phase 1 - MVP 屏幕捕获 | ✅ 已完成 |
| Phase 2 - 鼠标控制 | ✅ 已完成 |
| Phase 3 - WebRTC 优化 | ✅ 已完成 |
| Phase 4 - 安全特性 | ✅ 已完成 |
| Phase 5 - 系统服务打包 | ✅ 已完成 |

## 常用命令

```bash
# 构建
cargo build

# 运行
cargo run

# 测试
cargo test

# 代码检查
cargo clippy

# 格式化
cargo fmt

# 带功能特性构建
cargo build --features "h264,webrtc,security,service"
```

## 关键实现注意事项

### DPI 缩放

- 传输前将鼠标坐标归一化到 0.0-1.0 范围
- 在受控端映射回本地坐标

### 鼠标光标

- 视频流中不捕获光标
- 通过数据通道发送光标位置，控制端本地渲染

### macOS 权限

- 需要 **屏幕录制** + **辅助功能** 权限
- 无界面应用无法显示权限提示，需记录日志

### 安全性

- WebRTC 提供 DTLS/SRTP 加密
- API Key 认证
- HMAC-SHA256 Token 防重放攻击

## 项目结构

```
sscontrol/
├── src/
│   ├── main.rs              # 程序入口
│   ├── lib.rs               # 库入口
│   ├── config.rs            # 配置管理
│   ├── capture/             # 屏幕捕获模块
│   ├── encoder/             # 视频编码模块
│   ├── network/             # 网络传输模块
│   ├── input/               # 输入模拟模块
│   ├── security/            # 安全模块
│   ├── service/             # 系统服务模块
│   └── webrtc/              # WebRTC 模块
├── examples/                # 示例代码
├── scripts/                 # 安装脚本
├── docs/                    # 文档
├── tests/                   # 测试
└── Cargo.toml               # 项目配置
```

## Feature 标志

| Feature | 说明 |
|---------|------|
| `h264` | H.264 编码器 (需要 FFmpeg) |
| `webrtc` | WebRTC 支持 (使用 webrtc-rs) |
| `security` | 安全特性 (TLS 和认证) |
| `service` | 系统服务特性 (默认启用) |

## 已知问题

1. **编码器**: 当前使用 SimpleEncoder 直接传输原始帧数据，未实现 H.264 压缩
2. **鼠标滚轮**: macOS 滚轮事件支持有限
3. **Windows 捕获**: 使用 GDI BitBlt，性能可优化
