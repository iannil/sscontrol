# 开发环境搭建

## 前置要求

### 通用
- Git
- Rust 1.75+ (安装 [rustup](https://rustup.rs/))
- CMake (某些依赖需要)

### Windows
- Visual Studio Build Tools (C++ 支持)
- Windows 10+

### macOS
- Xcode Command Line Tools
- macOS 12.3+ (ScreenCaptureKit 要求)

## 初始化项目

```bash
# 克隆仓库
git clone https://github.com/yourname/sscontrol.git
cd sscontrol

# 初始化 Rust 项目
cargo init

# 创建项目目录结构
mkdir -p src/{capture,encoder,network,input,config,auth}
mkdir -p docs
mkdir -p tests
```

## Cargo.toml 配置

```toml
[package]
name = "sscontrol"
version = "0.1.0"
edition = "2021"
authors = ["Your Name <you@example.com>"]

[dependencies]
# 异步运行时
tokio = { version = "1.35", features = ["full"] }
tokio-tungstenite = "0.21"  # WebSocket

# WebRTC
webrtc = "0.11"  # 或 webrtc-rs

# 视频编码
ffmpeg-next = "7.0"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 配置
toml = "0.8"
directories = "5.0"

# 日志
tracing = "0.1"
tracing-subscriber = "0.3"

# 错误处理
anyhow = "1.0"
thiserror = "1.0"

# 输入模拟
rdev = "0.5"
# 或 enigo = "0.2"

[dependencies.winapi]
version = "0.3"
features = [
    "winuser",
    "dxgi1_2",
    "dxgi1_5",
    "d3d11",
]
optional = true
target = "x86_64-pc-windows-msvc"

[build-dependencies]
cc = "1.0"

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.25"
objc = "0.2"

[[bin]]
name = "sscontrol"
path = "src/main.rs"
```

## VS Code 配置

创建 `.vscode/settings.json`:

```json
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy",
    "files.associations": {
        "Cargo.lock": "toml"
    },
    "editor.formatOnSave": true,
    "rust-analyzer.rustfmt.extraArgs": ["+nightly"]
}
```

创建 `.vscode/tasks.json`:

```json
{
    "version": "2.0.0",
    "tasks": [
        {
            "label": "cargo build",
            "type": "shell",
            "command": "cargo",
            "args": ["build"],
            "group": {
                "kind": "build",
                "isDefault": true
            }
        },
        {
            "label": "cargo run",
            "type": "shell",
            "command": "cargo",
            "args": ["run"],
            "group": {
                "kind": "test",
                "isDefault": true
            }
        },
        {
            "label": "cargo test",
            "type": "shell",
            "command": "cargo",
            "args": ["test"],
            "group": "test"
        },
        {
            "label": "cargo clippy",
            "type": "shell",
            "command": "cargo",
            "args": ["clippy", "--", "-D", "warnings"],
            "group": "test"
        }
    ]
}
```

## 开发命令

```bash
# 构建
cargo build

# 开发构建 (更快)
cargo build --release

# 运行
cargo run

# 测试
cargo test

# 代码检查
cargo clippy

# 格式化
cargo fmt

# 查看文档
cargo doc --open
```

## 调试配置

创建 `.vscode/launch.json`:

```json
{
    "version": "0.0.1",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug sscontrol",
            "cargo": {
                "args": ["build"],
                "filter": {
                    "name": "sscontrol",
                    "kind": "bin"
                }
            },
            "args": [],
            "cwd": "${workspaceFolder}",
            "env": {
                "RUST_LOG": "debug"
            }
        }
    ]
}
```

## 测试依赖

```toml
[dev-dependencies]
criterion = "0.5"  # 性能测试
mockall = "0.12"   # Mock 框架
```

## Git 配置

创建 `.gitignore`:

```gitignore
/target
**/*.rs.bk
Cargo.lock
.DS_Store
.vscode/
.idea/
*.log
config.toml
*.dylib
*.dll
*.exe
```
