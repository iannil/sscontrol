# sscontrol 部署指南

本文档介绍如何使用 CI/CD 和 Docker 部署 sscontrol。

---

## 目录

- [CI/CD 使用](#cicd-使用)
- [Docker 部署](#docker-部署)
- [生产环境配置](#生产环境配置)
- [故障排查](#故障排查)

---

## CI/CD 使用

### 工作流说明

| 工作流 | 触发条件 | 用途 |
|--------|----------|------|
| `ci.yml` | push/PR | 代码质量检查和测试 |
| `build.yml` | push to main | 多平台构建 |
| `release.yml` | push tag (v*.*.*) | 自动发布 |

### CI 验证

```bash
# 本地运行相同的检查
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

### 创建发布

```bash
# 创建版本标签
git tag v0.1.0
git push origin v0.1.0

# GitHub Actions 将自动:
# 1. 构建所有平台的二进制文件
# 2. 创建 GitHub Release
# 3. 上传构建产物
# 4. 构建并发布 Docker 镜像
```

---

## Docker 部署

### 快速开始

```bash
# 构建所有镜像
docker-compose build

# 启动服务 (信令 + TURN + Redis)
docker-compose up -d signaling coturn redis

# 查看状态
docker-compose ps
docker-compose logs -f signaling
```

### 单独构建信令服务器

```bash
docker build -f Dockerfile.signaling -t sscontrol-signaling .
docker run -p 8080:8080 -e RUST_LOG=info sscontrol-signaling
```

### 单独构建主机代理

```bash
docker build -f Dockerfile.agent -t sscontrol-agent .
# 注意: agent 需要额外权限才能捕获屏幕
docker run --cap-add=SYS_ADMIN --network=host sscontrol-agent
```

### 使用预构建镜像

```bash
# 从 GitHub Container Registry 拉取
docker pull ghcr.io/<your-org>/sscontrol/signaling:latest
docker pull ghcr.io/<your-org>/sscontrol/agent:latest
```

---

## 生产环境配置

### 环境变量

#### 信令服务器

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `RUST_LOG` | `info` | 日志级别 |
| `SSCONTROL_BIND_ADDRESS` | `0.0.0.0:8080` | 绑定地址 |
| `SSCONTROL_API_KEY` | - | API Key (启用认证) |

#### TURN 服务器

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `TURN_EXTERNAL_IP` | `auto` | 公网 IP |
| `TURN_USERNAME` | `sscontrol` | TURN 用户名 |
| `TURN_PASSWORD` | `sscontrol-secret-password` | TURN 密码 |

### 生产环境 docker-compose.yml

```yaml
services:
  signaling:
    image: ghcr.io/<your-org>/sscontrol/signaling:latest
    restart: always
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=warn
      - SSCONTROL_API_KEY=${SSCONTROL_API_KEY}

  coturn:
    image: coturn/coturn:latest
    restart: always
    ports:
      - "3478:3478/tcp"
      - "3478:3478/udp"
    environment:
      - TURN_EXTERNAL_IP=${TURN_EXTERNAL_IP}
      - TURN_USERNAME=${TURN_USERNAME}
      - TURN_PASSWORD=${TURN_PASSWORD}
```

### 安全建议

1. **启用 API Key 认证**
   ```bash
   export SSCONTROL_API_KEY=$(openssl rand -hex 32)
   ```

2. **使用强 TURN 密码**
   ```bash
   export TURN_PASSWORD=$(openssl rand -hex 32)
   ```

3. **配置防火墙**
   - 只开放必要端口 (8080, 3478, 5349)
   - 限制 TURN 端口范围

4. **使用 HTTPS/WSS**
   - 在信令服务器前添加反向代理 (nginx/caddy)
   - 配置 SSL/TLS 证书

5. **限制容器资源**
   ```yaml
   deploy:
     resources:
       limits:
         cpus: '1'
         memory: 512M
   ```

---

## 故障排查

### CI 失败

```bash
# 检查日志
gh run view <run-id> --log-failed

# 本地复现
cargo clippy --all-targets --all-features -- -D warnings
```

### Docker 构建失败

```bash
# 查看详细日志
docker build --progress=plain -f Dockerfile.signaling .

# 无缓存构建
docker-compose build --no-cache
```

### 容器无法启动

```bash
# 查看日志
docker-compose logs signaling
docker logs sscontrol-signaling

# 进入容器调试
docker-compose exec signaling sh
```

### 屏幕捕获失败 (Agent)

Agent 需要额外权限:

```bash
# X11 权限
xhost +local:docker

# 运行带权限的容器
docker run --cap-add=SYS_ADMIN \
           --device=/dev/dri \
           -v /tmp/.X11-unix:/tmp/.X11-unix \
           -e DISPLAY=$DISPLAY \
           sscontrol-agent
```

---

## 参考资料

- [项目架构](../docs/architecture/overview.md)
- [配置文件](config.toml.example)
- [GitHub Actions](https://github.com/features/actions)
- [Docker Compose](https://docs.docker.com/compose/)
