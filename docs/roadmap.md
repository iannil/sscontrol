# 开发路线图

**最后更新**: 2026-01-22

## 项目概述

sscontrol 是一个基于 Rust 的无界面远程桌面应用，采用 WebRTC 实现 P2P 通信。

**当前状态**: 所有核心阶段已完成，项目进入维护和优化阶段

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

### Phase 6: 稳定性与性能优化 ✅

**目标**: 提升代码稳定性和运行时性能

**状态**: 已完成

**核心功能**:
- [x] 代码稳定性改进
  - [x] 移除 panic/expect/unwrap
  - [x] 改用 Result 返回错误
  - [x] 优雅错误处理
- [x] WebRTC 配置支持
  - [x] STUN/TURN 服务器配置
  - [x] ICE 传输策略
  - [x] CLI 参数扩展
- [x] Windows DXGI 捕获
  - [x] Desktop Duplication API
  - [x] GDI fallback 支持
- [x] 性能评估工具
  - [x] latency_test 延迟测试
  - [x] 统计报告和直方图

---

### Phase 7: H.264 编码器修复 ✅

**目标**: 修复 H.264 编码器编译问题

**状态**: 已完成

**核心功能**:
- [x] H.264 编码器修复
  - [x] 解决借用冲突
  - [x] 重构 encode() 方法
  - [x] 验证 libx264 编码
- [x] 代码质量提升
  - [x] 移除残留 panic 调用
  - [x] 改用 unreachable!() 宏

---

### Phase 8: 信令服务器自动部署 ✅

**目标**: 实现一键部署信令服务器到 Linux 服务器

**状态**: 已完成

**核心功能**:
- [x] SSH 连接管理 (`src/deploy/ssh.rs`)
  - [x] SSH Agent 支持
  - [x] 公钥认证
  - [x] 密码认证回退
  - [x] 文件上传
- [x] 部署逻辑 (`src/deploy/signaling_deploy.rs`)
  - [x] 系统要求检查
  - [x] 目录结构创建
  - [x] 二进制上传
  - [x] systemd 服务配置
  - [x] 防火墙配置
  - [x] 部署验证
- [x] TLS 支持
  - [x] Let's Encrypt 集成
  - [x] certbot 自动续期
- [x] 独立信令服务器 (`src/bin/signaling_server.rs`)
  - [x] CLI 参数支持
  - [x] 健康检查端点
  - [x] 优雅关闭
- [x] CLI 命令
  - [x] `deploy signaling` - 部署
  - [x] `deploy status` - 状态检查
  - [x] `deploy uninstall` - 卸载

---

## 未来计划

### 潜在改进方向

| 优先级 | 功能 | 说明 |
|--------|------|------|
| P1 | Linux 平台屏幕捕获 | 添加 X11/Wayland 支持 |
| P2 | 音频传输 | 添加音频捕获和传输功能 |
| P3 | Web 客户端 | 浏览器端控制客户端 |
| P4 | macOS 滚轮事件完善 | 当前实现支持有限 |

---

## 技术债务追踪

| ID | 描述 | 优先级 | 状态 |
|----|------|--------|------|
| T001 | H.264 编码器修复 | P1 | ✅ 已完成 |
| T002 | Windows Desktop Duplication API | P3 | ✅ 已完成 |
| T003 | 端到端延迟性能测试 | P2 | ✅ 已完成 |
| T004 | macOS 滚轮事件完整实现 | P3 | 待实现 |
| T005 | Linux 屏幕捕获支持 | P2 | 待实现 |

---

## 变更日志

### 2026-01-22
- 完成 Phase 8：信令服务器自动部署功能
- 新增 `src/deploy/` 模块
- 新增 `src/bin/signaling_server.rs` 独立二进制
- 更新文档，归档过时内容

### 2026-01-21
- 完成 Phase 6 和 Phase 7
- H.264 编码器修复
- Windows DXGI 捕获实现
- 代码稳定性优化

### 2026-01-20
- 完成 Phase 0-5 所有开发阶段
- 归档过时的设计文档
