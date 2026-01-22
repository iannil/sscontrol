# 文档清理总结

**日期**: 2026-01-22

## 已完成的清理

### 1. 归档的文档

| 文件 | 原因 |
|------|------|
| `docs/ui-guide.md` | 描述不存在的 Tauri UI（项目是无界面的） |

### 2. 更新的文档

| 文件 | 更新内容 |
|------|----------|
| `docs/progress.md` | 添加 Phase 8（信令服务器部署）、更新项目结构 |
| `docs/roadmap.md` | 添加 Phase 6/7/8、更新技术债务状态、更新变更日志 |
| `docs/deployment-guide.md` | 新增第六章A：自动化部署（deploy 命令文档） |

---

## 已识别的问题

### 需要注意但暂不处理

#### 1. 引用不存在的基础设施

以下文档引用了不存在的 Docker 基础设施：

- `docs/deployment-guide.md` - 第五章 Docker 部署
- `docs/operations/runbook.md` - 引用 `docker-compose.prod.yml`
- `docs/production-checklist.md` - 引用 Docker 和 Redis 配置

**状态**: 这些是理想的生产部署配置，但实际基础设施文件（docker-compose.yml, nginx 配置等）尚未创建。

**建议**:
1. 创建实际的 Docker 配置文件，或
2. 将这些章节标记为"计划中"/"参考"

#### 2. 过期脚本

| 文件 | 状态 | 原因 |
|------|------|------|
| `scripts/generate-icons.sh` | ✅ 已删除 | Tauri UI 不存在 |

#### 3. 重复代码

| 文件 | 对比 | 状态 |
|------|------|------|
| `examples/signaling_server.rs` | `src/bin/signaling_server.rs` | 保留 |

**说明**: 虽然功能类似，但 examples 版本是简单参考实现，bin 版本是生产就绪的独立二进制。两者服务不同目的。

---

## 项目当前状态总结

### 已完成功能

1. ✅ 屏幕捕获（macOS CGDisplay, Windows GDI/DXGI）
2. ✅ 视频编码（SimpleEncoder, H264Encoder）
3. ✅ WebRTC 通信
4. ✅ 鼠标/键盘控制
5. ✅ 安全认证（API Key, HMAC Token）
6. ✅ 系统服务（Windows Service, macOS LaunchAgent, Linux systemd）
7. ✅ 信令服务器自动部署（SSH, TLS）

### 待开发功能

1. ⏳ Linux 屏幕捕获支持
2. ⏳ 音频传输
3. ⏳ Web 客户端
4. ⏳ macOS 滚轮事件完善

### Feature 标志

| Feature | 状态 | 说明 |
|---------|------|------|
| `h264` | ✅ | FFmpeg H.264 编码 |
| `webrtc` | ✅ | WebRTC 通信 |
| `security` | ✅ | TLS 和认证 |
| `service` | ✅ | 系统服务 |
| `discovery` | ✅ | mDNS 设备发现 |
| `deploy` | ✅ | 远程部署 |

---

## 建议后续行动

1. ~~**低优先级**: 删除 `scripts/generate-icons.sh`~~ ✅ 已完成
2. **中优先级**: 为 Docker 部署创建实际的配置文件，或更新文档说明
3. **低优先级**: 考虑合并或简化 examples 目录
