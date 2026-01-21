# CLAUDE.md

此文件为 Claude Code (claude.ai/code) 在此仓库中工作时提供指导。

## 项目概述

**sscontrol** 是一个基于 Rust 构建的无界面（Headless）远程桌面应用。采用 Host + Client 架构，使用 WebRTC 进行 P2P 通信。

**当前阶段**: Phase 0 - 规划与设计（项目尚未初始化）

## 快速链接

| 文档 | 路径 |
| ------ | ------ |
| 项目进度 | `docs/progress.md` |
| 开发路线图 | `docs/roadmap.md` |
| 架构设计 | `docs/architecture/overview.md` |
| 环境搭建 | `docs/implementation/setup.md` |
| Phase 1 MVP | `docs/phase1-mvp.md` |
| 常见问题 | `docs/troubleshooting/common-issues.md` |

## 开发阶段

| 阶段 | 状态 |
| ------ | ------ |
| Phase 0 - 规划设计 | 🔄 进行中 |
| Phase 1 - MVP 屏幕捕获 | ⏳ 待开始 |
| Phase 2 - 鼠标控制 | ⏳ 待开始 |
| Phase 3 - WebRTC 优化 | ⏳ 待开始 |
| Phase 4 - 安全特性 | ⏳ 待开始 |
| Phase 5 - 系统服务打包 | ⏳ 待开始 |

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
- 实现全局热键（Ctrl+Alt+Shift+Q）用于紧急断开

## 项目结构（计划）

```text
sscontrol/
├── src/
│   ├── main.rs              # 入口
│   ├── config.rs            # 配置管理
│   ├── capture/             # 屏幕捕获模块
│   ├── encoder/             # 视频编码模块
│   ├── network/             # 网络传输模块
│   ├── input/               # 输入模拟模块
│   └── auth/                # 认证模块
├── docs/                    # 文档
├── tests/                   # 测试
└── Cargo.toml               # 项目配置
```
