# 开发路线图

**最后更新**: 2026-01-21

## 项目概述

sscontrol 是一个基于 Rust 的无界面远程桌面应用，采用 WebRTC 实现 P2P 通信。

**当前状态**: 所有阶段已完成，项目进入维护和优化阶段

---

## 开发阶段

### Phase 0: 规划与设计 ✅

**目标**: 完成技术架构设计和项目初始化

**状态**: 已完成

**任务清单**:
- [x] 架构设计文档
- [x] 技术栈选型
- [x] 开发路线规划
- [x] 初始化 Rust 项目 (`cargo init`)
- [x] 创建 Cargo.toml 配置
- [x] 建立项目目录结构
- [x] 搭建基础开发环境

---

### Phase 1: MVP - 单向屏幕捕获 ✅

**目标**: 实现基础的屏幕捕获和单向视频传输

**状态**: 已完成

**核心功能**:
- [x] 屏幕捕获模块 (`src/capture/`)
  - [x] macOS: Core Graphics API 实现
  - [x] Windows: GDI BitBlt 实现
  - [x] 跨平台抽象层设计
- [x] 视频编码模块 (`src/encoder/`)
  - [x] SimpleEncoder (直接传输原始数据)
  - [x] H264Encoder (可选，使用 FFmpeg)
  - [x] 帧率控制 (30fps)
- [x] 基础网络传输 (`src/network/`)
  - [x] WebSocket 信令客户端
  - [x] 视频流传输
- [x] 命令行界面
  - [x] 配置文件读取 (`config.toml`)
  - [x] Device ID 生成/读取
  - [x] 连接状态输出

---

### Phase 2: 鼠标控制 ✅

**目标**: 添加鼠标控制功能，实现双向交互

**状态**: 已完成

**核心功能**:
- [x] 输入模拟模块 (`src/input/`)
  - [x] 鼠标移动
  - [x] 鼠标点击 (左键/右键/中键)
  - [x] 鼠标滚轮
  - [x] Windows: SendInput API
  - [x] macOS: CGEvent API
- [x] 数据通道
  - [x] 输入指令协议定义
  - [x] JSON 序列化
  - [x] 坐标归一化处理 (0.0-1.0)

---

### Phase 3: WebRTC 优化 ✅

**目标**: 迁移到 WebRTC，实现低延迟 P2P 通信

**状态**: 已完成

**核心功能**:
- [x] WebRTC 集成 (`src/webrtc/`)
  - [x] `webrtc-rs` 库集成
  - [x] PeerConnection 管理
  - [x] SDP 协商
  - [x] ICE 候选收集
- [x] RTP 视频轨道
- [x] 信令客户端
- [x] 完整示例代码

---

### Phase 4: 安全特性 ✅

**目标**: 添加认证、加密和权限管理

**状态**: 已完成

**核心功能**:
- [x] 认证机制 (`src/security/`)
  - [x] API Key 认证
  - [x] HMAC-SHA256 Token 支持
  - [x] 防重放攻击保护
- [x] 加密增强
  - [x] TLS/SSL 配置
  - [x] 环境变量支持
- [x] 安全模块完整实现

---

### Phase 5: 系统服务打包 ✅

**目标**: 打包为系统服务，实现开机自启

**状态**: 已完成

**核心功能**:
- [x] Windows 打包
  - [x] Windows Service 注册
  - [x] install_windows.ps1 安装脚本
- [x] macOS 打包
  - [x] LaunchAgent plist 配置
  - [x] install_macos.sh 安装脚本
- [x] Linux 打包
  - [x] systemd 服务实现
  - [x] install_linux.sh 安装脚本
- [x] 命令行服务管理接口

---

## 未来计划

### 潜在改进方向

| 优先级 | 功能 | 说明 |
|--------|------|------|
| P1 | H.264 编码器优化 | 当前 SimpleEncoder 传输原始数据，带宽需求高 |
| P2 | Windows Desktop Duplication API | 替换 GDI 以获得更好性能 |
| P3 | macOS 滚轮事件完善 | 当前实现支持有限 |
| P4 | Linux 平台支持 | 目前仅支持 macOS 和 Windows |
| P5 | 音频传输 | 添加音频捕获和传输功能 |

---

## 技术债务追踪

| ID | 描述 | 优先级 | 状态 |
|----|------|--------|------|
| T001 | H.264 编码器未默认启用 | P1 | 待优化 |
| T002 | 端到端延迟性能测试 | P2 | 待测试 |
| T003 | macOS 滚轮事件完整实现 | P3 | 待实现 |
| T004 | Windows Desktop Duplication API | P3 | 待实现 |

---

## 变更日志

### 2026-01-21
- 完成所有 5 个开发阶段
- 更新路线图状态为全部完成
- 归档过时的设计文档
