# 局域网 Mac 连接 Windows 教程

本教程介绍如何在局域网内，使用 Mac 电脑远程控制 Windows 电脑。

## 场景说明

```
┌─────────────────┐         ┌─────────────────┐
│   Mac 电脑       │         │  Windows 电脑    │
│  (控制端/Client) │◄────────┤  (受控端/Host)   │
│                 │ WebRTC  │                 │
│  - 客户端程序    │  P2P    │  - 屏幕捕获      │
│  - 鼠标键盘输入  │         │  - 输入模拟      │
└─────────────────┘         └─────────────────┘
         ▲                           ▲
         │                           │
         └───────────┬───────────────┘
                     │
              ┌──────▼──────┐
              │ 信令服务器    │
              │ (运行在 Mac) │
              └─────────────┘
```

## 前提条件

### 硬件要求
- 两台电脑在同一局域网内
- 网络延迟 < 50ms（推荐有线连接）

### 软件要求
- **Mac 电脑**: macOS 12+，Rust 工具链（用于交叉编译）
- **Windows 电脑**: Windows 10/11（无需安装 Rust，直接运行编译好的可执行文件）

---

## 步骤一：在 Mac 上安装 Rust 工具链

```bash
# 1. 安装 Rust（如果尚未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 2. 验证安装
rustc --version
cargo --version
```

**预期输出**：
```
rustc 1.xx.x (xxxxxx)
cargo 1.xx.x (xxxxxx)
```

---

## 步骤二：获取项目源码

在 **Mac 电脑**上执行：

```bash
# 克隆项目
git clone https://github.com/your-repo/sscontrol.git
cd sscontrol
```

---

## 步骤三：配置 Windows 交叉编译环境

在 **Mac 电脑**上安装 Windows 交叉编译工具链：

```bash
# 1. 添加 Windows 编译目标（GNU 工具链）
rustup target add x86_64-pc-windows-gnu

# 2. 安装交叉编译工具
# 使用 Homebrew 安装 MinGW-w64
brew install mingw-w64
```

**验证目标是否添加成功**：
```bash
rustup target list | grep windows
```

**预期输出**：
```
x86_64-pc-windows-gnu    (installed)
x86_64-pc-windows-msvc
```

---

## 步骤四：编译 Mac 可执行文件

在 **Mac 电脑**上执行：

```bash
# 进入项目目录
cd sscontrol

# 编译 Mac 版本（带 WebRTC 支持）
cargo build --release --features webrtc

# 编译完成后，可执行文件位于:
# target/release/sscontrol
```

**编译输出**：
```
Compiling sscontrol v0.1.0
    Finished release [optimized] target(s) in XXs
```

**验证可执行文件**：
```bash
# 查看文件信息
file target/release/sscontrol
ls -lh target/release/sscontrol
```

**预期输出**（显示为 macOS 可执行文件）：
```
target/release/sscontrol: Mach-O 64-bit executable arm64
-rwxr-xr-x  1 user  staff   XXXB XX XX XX:XX target/release/sscontrol
```

---

## 步骤五：交叉编译 Windows 可执行文件

在 **Mac 电脑**上执行：

```bash
# 交叉编译 Windows 版本（带 WebRTC 支持）
cargo build --release --features webrtc --target x86_64-pc-windows-gnu

# 编译完成后，可执行文件位于:
# target/x86_64-pc-windows-gnu/release/sscontrol.exe
```

**编译输出**：
```
Compiling sscontrol v0.1.0
    Finished release [optimized] target(s) in XXs
```

**验证可执行文件**：
```bash
# 查看文件信息
file target/x86_64-pc-windows-gnu/release/sscontrol.exe
ls -lh target/x86_64-pc-windows-gnu/release/sscontrol.exe
```

**预期输出**（显示为 Windows 可执行文件）：
```
target/x86_64-pc-windows-gnu/release/sscontrol.exe: PE32+ executable (console) x86-64, for MS Windows
-rwxr-xr-x  1 user  staff   XXXB XX XX XX:XX target/x86_64-pc-windows-gnu/release/sscontrol.exe
```

---

## 步骤六：将 Windows 可执行文件传输到 Windows 电脑

### 方式一：通过网络共享

```bash
# 在 Mac 上，将可执行文件复制到共享目录
cp target/x86_64-pc-windows-gnu/release/sscontrol.exe ~/Public/

# 在 Windows 上，通过网络共享访问 Mac 的 Public 文件夹
# \\192.168.x.x\Public\sscontrol.exe
```

### 方式二：使用 U 盘

```bash
# 在 Mac 上复制到 U 盘
cp target/x86_64-pc-windows-gnu/release/sscontrol.exe /Volumes/USB_DRIVE/

# 在 Windows 上从 U 盘复制
```

### 方式三：使用 scp（Windows 需安装 OpenSSH）

```bash
# 在 Mac 上发送到 Windows
scp target/x86_64-pc-windows-gnu/release/sscontrol.exe user@192.168.x.x:C:/Users/user/
```

---

## 步骤七：准备配置文件

### Windows 受控端配置

在 Windows 上创建 `C:\sscontrol\config.toml`：

```toml
[server]
# 信令服务器地址（稍后填写 Mac 的 IP）
url = "ws://192.168.x.x:8080"

# 设备 ID
device_id = "windows-host"

[capture]
# 目标帧率
fps = 30

# 屏幕索引（0 = 主显示器）
screen_index = 0

[logging]
level = "info"
```

### Mac 控制端配置

在 Mac 项目目录下创建 `config.toml`：

```toml
[server]
# 信令服务器地址（稍后填写）
url = "ws://192.168.x.x:8080"

# 设备 ID
device_id = "mac-client"

[logging]
level = "info"
```

---

## 步骤八：启动信令服务器

在 **Mac 电脑**上执行：

```bash
# 1. 获取 Mac 的 IP 地址
ifconfig | grep "inet " | grep -v 127.0.0.1

# 输出示例:
# inet 192.168.1.100 netmask 0xffffff00 broadcast 192.168.1.255

# 2. 启动信令服务器（新建一个终端窗口）
cargo run --example signaling_server
```

**信令服务器启动成功输出**：
```
INFO  信令服务器监听: 127.0.0.1:8080
INFO  WebSocket 端点: ws://127.0.0.1:8080
```

---

## 步骤九：更新配置文件的信令服务器地址

将配置文件中的 `url` 更新为 Mac 的实际 IP 地址。

**示例**（Mac IP 为 `192.168.1.100`）：

```toml
[server]
url = "ws://192.168.1.100:8080"
```

---

## 步骤十：启动 Windows 受控端

在 **Windows 电脑**上执行：

```powershell
# 进入存放 sscontrol.exe 的目录
cd C:\sscontrol

# 方式一：直接运行
.\sscontrol.exe run

# 方式二：安装为 Windows 服务（需要管理员权限）
.\sscontrol.exe service install
.\sscontrol.exe service start
```

**启动成功输出**：
```
INFO  Remote Desktop Service Started
INFO  Device ID: windows-host
INFO  Connecting to ws://192.168.1.100:8080...
INFO  Connected to signaling server
INFO  Joined room: default
```

---

## 步骤十一：启动 Mac 控制端并连接

在 **Mac 电脑**上执行：

```bash
# 运行 WebRTC 客户端
./target/release/sscontrol webrtc --server ws://192.168.1.100:8080 --room myroom --id mac-client
```

**连接成功输出**：
```
========================================
WebRTC 客户端示例
========================================
客户端 ID: mac-client
房间 ID: myroom
信令服务器: ws://192.168.1.100:8080
========================================

✓ 已连接到信令服务器
✓ 已加入房间: myroom
✓ 新对等端加入: windows-host
  -> 发起连接到: windows-host
  -> Offer 创建成功
  -> Offer 已发送
✓ 收到 Answer from: windows-host
✓ 收到 ICE 候选 from: windows-host
  -> ICE 候选已添加
```

---

## 网络配置

### Windows 防火墙设置

在 **Windows 电脑**上执行：

```powershell
# 允许 sscontrol 通过防火墙
New-NetFirewallRule -DisplayName "sscontrol" -Direction Inbound -Program "C:\sscontrol\sscontrol.exe" -Action Allow
```

或通过图形界面：
1. 打开 "Windows Defender 防火墙" > "允许应用通过防火墙"
2. 点击 "更改设置" > "允许其他应用"
3. 浏览并选择 `sscontrol.exe`
4. 勾选 "专用" 和 "公用"

### 获取 Windows IP 地址

在 **Windows 电脑**上执行：

```powershell
ipconfig
```

查找 "IPv4 地址"，例如：
```
IPv4 地址 . . . . . . . . . . . : 192.168.1.105
```

---

## 验证连接

### 测试网络连通性

在 **Mac** 上测试：
```bash
# 测试到 Windows 的连通性
ping 192.168.1.105
```

### 检查 Windows 受控端

在 **Windows** 上检查：
```powershell
# 检查进程是否运行
Get-Process sscontrol -ErrorAction SilentlyContinue

# 查看服务状态（如果安装为服务）
.\sscontrol.exe service status
```

---

## 常见问题

### Q1: 交叉编译 Windows 版本失败

**错误示例**：
```
error: linking with `x86_64-w64-mingw32-gcc` failed
```

**解决方案**：
```bash
# 确保已安装 MinGW-w64
brew install mingw-w64

# 检查编译器是否可用
x86_64-w64-mingw32-gcc --version

# 如果命令不存在，重新安装
brew reinstall mingw-w64
```

### Q2: Windows 上运行时提示缺少 DLL

**错误示例**：
```
libgcc_s_seh-1.dll 缺失
```

**解决方案**：
将 MinGW 的 DLL 文件与 `sscontrol.exe` 一起复制到 Windows：

```bash
# 在 Mac 上查找 DLL 文件
find /opt/homebrew -name "libgcc_s_seh-1.dll" 2>/dev/null
find /opt/homebrew -name "libwinpthread-1.dll" 2>/dev/null

# 复制 DLL 与可执行文件到同一目录
cp /opt/homebrew/Cellar/mingw-w64/*/x86_64-w64-mingw32/lib/libgcc_s_seh-1.dll .
cp /opt/homebrew/Cellar/mingw-w64/*/x86_64-w64-mingw32/lib/libwinpthread-1.dll .
```

**或使用静态链接**（推荐）：

在 `~/.cargo/config.toml` 中添加：
```toml
[target.x86_64-pc-windows-gnu]
rustflags = "-C target-feature=+crt-static"
```

### Q3: 无法连接到 Windows 电脑

**解决方案**：
1. 检查两台电脑是否在同一局域网
2. 确认 Windows 防火墙允许 sscontrol 通信
3. 验证 sscontrol 正在运行
4. 检查信令服务器是否正常运行

### Q4: 连接成功但没有视频

**解决方案**：
1. 查看受控端日志是否有捕获错误
2. 尝试降低帧率：在 `config.toml` 中设置 `fps = 15`
3. 检查屏幕索引是否正确

### Q5: macOS 上 Xcode 命令行工具未安装

**解决方案**：
```bash
# 安装 Xcode 命令行工具
xcode-select --install

# 同意 Xcode 许可（首次编译时可能需要）
sudo xcodebuild -license accept
```

---

## 快速启动脚本

### Mac 构建脚本 (`build_all.sh`)

创建 `build_all.sh` 文件：

```bash
#!/bin/bash
set -e

echo "==================================="
echo "Building sscontrol for all platforms"
echo "==================================="

# 编译 Mac 版本
echo "Building for macOS (arm64/x86_64)..."
cargo build --release --features webrtc

# 交叉编译 Windows 版本
echo "Building for Windows (x86_64)..."
cargo build --release --features webrtc --target x86_64-pc-windows-gnu

echo ""
echo "==================================="
echo "Build complete!"
echo "==================================="
echo "Mac binary:     target/release/sscontrol"
echo "Windows binary: target/x86_64-pc-windows-gnu/release/sscontrol.exe"
echo ""

# 显示文件信息
echo "File sizes:"
ls -lh target/release/sscontrol
ls -lh target/x86_64-pc-windows-gnu/release/sscontrol.exe
```

赋予执行权限：
```bash
chmod +x build_all.sh
```

使用方法：
```bash
./build_all.sh
```

### Windows 启动脚本 (`start_host.bat`)

在 Windows 上创建 `start_host.bat` 文件：

```batch
@echo off
cd /d "%~dp0"
echo Starting sscontrol host on Windows...
sscontrol.exe run
pause
```

### Mac 连接脚本 (`connect.sh`)

在 Mac 上创建 `connect.sh` 文件：

```bash
#!/bin/bash
SERVER_URL="${1:-ws://192.168.1.100:8080}"
ROOM_ID="${2:-myroom}"

echo "Connecting to $SERVER_URL in room $ROOM_ID..."
./target/release/sscontrol webrtc --server "$SERVER_URL" --room "$ROOM_ID" --id mac-client
```

赋予执行权限：
```bash
chmod +x connect.sh
```

---

## 交叉编译参考

### 支持的目标平台

```bash
# 查看所有可用的编译目标
rustup target list

# 常用目标：
# x86_64-apple-darwin      - macOS (Intel)
# aarch64-apple-darwin     - macOS (Apple Silicon)
# x86_64-pc-windows-gnu    - Windows (GNU 工具链)
# x86_64-pc-windows-msvc   - Windows (MSVC 工具链)
# x86_64-unknown-linux-gnu - Linux (x86_64)
```

### 构建通用 Mac 二进制文件（Universal Binary）

```bash
# 同时为 Intel 和 Apple Silicon 编译
cargo build --release --features webrtc --target x86_64-apple-darwin
cargo build --release --features webrtc --target aarch64-apple-darwin

# 使用 lipo 合并为通用二进制文件
lipo -create \
    target/x86_64-apple-darwin/release/sscontrol \
    target/aarch64-apple-darwin/release/sscontrol \
    -output target/release/sscontrol-universal

# 验证
lipo -info target/release/sscontrol-universal
```

---

## 总结

完成以上步骤后，你应该能够：

1. ✅ 在 Mac 上编译 Mac 版本的可执行文件
2. ✅ 在 Mac 上交叉编译 Windows 版本的可执行文件
3. ✅ 将 Windows 可执行文件传输到 Windows 电脑
4. ✅ 在 Mac 上启动信令服务器
5. ✅ 在 Windows 上运行受控端
6. ✅ 从 Mac 控制端连接并远程控制 Windows

**优势**：
- 无需在 Windows 上安装 Rust 工具链
- 所有编译工作在 Mac 上完成
- 便于统一管理和分发

如有问题，请查看 `docs/troubleshooting/common-issues.md` 获取更多帮助。
