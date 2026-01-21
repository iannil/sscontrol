# 常见问题排查

## 开发问题

### Cargo 构建失败

**问题**: `error: linking with cc failed`

**解决方案**:
- Windows: 安装 Visual Studio Build Tools
- macOS: `xcode-select --install`
- Linux: `sudo apt install build-essential`

### FFmpeg 依赖问题

**问题**: `ffmpeg-next` 编译失败

**解决方案**:
```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install libavcodec-dev libavformat-dev libavutil-dev

# Windows (vcpkg)
vcpkg install ffmpeg
```

---

## 运行时问题

### macOS 权限被拒绝

**问题**: 屏幕录制失败，无权限

**解决方案**:
1. 系统设置 → 隐私与安全性 → 屏幕录制
2. 添加终端或应用到允许列表
3. 重启应用

### Windows 捕获失败

**问题**: Desktop Duplication API 失败

**可能原因**:
- 运行在 Session 0 (服务模式)
- 另一个应用已在捕获

**解决方案**:
- 确保在用户会话中运行
- 关闭其他录屏软件

---

## 网络问题

### WebRTC 连接失败

**问题**: ICE 连接失败

**排查步骤**:
1. 检查 STUN 服务器是否可达
2. 检查防火墙设置
3. 尝试使用 TURN 中继

### WebSocket 断连

**问题**: 频繁断线重连

**解决方案**:
- 检查网络稳定性
- 增加心跳间隔
- 实现指数退避重连

---

## 性能问题

### CPU 占用过高

**可能原因**:
- 帧率过高
- 软件编码
- 未启用硬件加速

**解决方案**:
- 降低目标帧率
- 启用硬件编码器
- 降低分辨率

### 延迟过高

**可能原因**:
- 网络抖动
- 编码缓冲区过大
- 未使用 VBR

**解决方案**:
- 检查网络状况
- 减小编码器缓冲
- 启用低延迟模式

---

## 日志调试

启用调试日志:

```bash
RUST_LOG=debug cargo run
```

启用 trace 日志:

```bash
RUST_LOG=trace cargo run
```

特定模块日志:

```bash
RUST_LOG=sscontrol::capture=debug cargo run
```
