# SSControl UI 用户指南

## 概述

SSControl GUI 是基于 Tauri 2.0 + React + TypeScript 构建的图形用户界面。

## 构建命令

### 开发模式

```bash
# 启动开发服务器 (带热重载)
cd ui && npm run dev

# 启动 Tauri 开发模式 (同时运行前端和后端)
cargo tauri dev
```

### 生产构建

```bash
# 构建 UI
cd ui && npm run build

# 构建完整的 Tauri 应用
cargo tauri build
```

## 功能说明

### 主控面板 (Dashboard)

- 显示连接状态（已连接/连接中/断开）
- 实时统计信息（FPS、帧数、带宽、运行时间）
- 启动/停止连接控制
- 快捷操作入口

### 配置管理 (Configuration)

- 服务器连接设置（URL、设备 ID）
- 视频捕获设置（帧率、分辨率、屏幕索引）
- 日志配置（级别、文件路径）
- 安全设置（API Key、TLS、Token 有效期）
- 导入/导出配置
- 重置默认配置

### 屏幕选择 (Screen Select)

- 多显示器可视化列表
- 屏幕缩略图预览
- 分辨率和缩放信息显示
- 主显示器标记

### 服务管理 (Service Manager)

- 服务状态显示
- 安装/卸载系统服务
- 启动/停止服务
- 平台特定信息（macOS LaunchAgent / Windows Service / systemd）

## 技术架构

```
┌─────────────────────────────────────┐
│           React Frontend            │
├─────────────────────────────────────┤
│        Tauri Commands Bridge        │
├─────────────────────────────────────┤
│         sscontrol Core Lib          │
├─────────────────────────────────────┤
│  Capture │ Network │ Service │ UI   │
└─────────────────────────────────────┘
```

## 项目结构

```
ui/
├── src/
│   ├── lib/
│   │   ├── api.ts          # Tauri API 封装
│   │   └── store.ts        # Zustand 状态管理
│   ├── pages/
│   │   ├── Dashboard.tsx   # 主控面板
│   │   ├── Configuration.tsx
│   │   ├── ScreenSelect.tsx
│   │   └── ServiceManager.tsx
│   ├── components/         # 可复用组件
│   ├── hooks/             # 自定义 Hooks
│   ├── App.tsx            # 应用入口
│   └── main.tsx           # React 挂载点
├── package.json
└── vite.config.ts

src-tauri/
├── src/
│   ├── main.rs            # Tauri 入口
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── config.rs      # 配置命令
│   │   ├── connection.rs  # 连接命令
│   │   ├── screen.rs      # 屏幕命令
│   │   └── service.rs     # 服务命令
│   ├── state.rs           # 应用状态
│   └── events.rs          # 事件处理
├── Cargo.toml
├── tauri.conf.json
└── build.rs
```

## 开发注意事项

1. **热重载**: 修改前端代码后自动刷新，修改 Rust 代码需重新编译

2. **事件监听**: 使用 `setupEventListeners()` 初始化事件监听

3. **状态管理**: 全局状态使用 Zustand，组件状态使用 useState

4. **样式**: 使用 CSS 变量定义主题，支持暗色模式

5. **类型安全**: TypeScript 类型定义与 Rust 结构体保持同步
