# sscontrol 发布部署教程

本文档提供 sscontrol 从开发到生产部署的完整教程。

---

## 目录

- [第一章：准备工作](#第一章准备工作)
- [第二章：本地构建与测试](#第二章本地构建与测试)
- [第三章：CI/CD 配置](#第三章cicd-配置)
- [第四章：版本发布](#第四章版本发布)
- [第五章：Docker 部署](#第五章docker-部署)
- [第六章：原生部署](#第六章原生部署)
- [第七章：监控配置](#第七章监控配置)
- [第八章：运维维护](#第八章运维维护)

---

## 第一章：准备工作

### 1.1 开发环境要求

| 组件 | 最低版本 | 推荐版本 |
|------|----------|----------|
| Rust | 1.75+ | 1.80+ |
| Docker | 24.0+ | 26.0+ |
| Docker Compose | 2.20+ | 2.25+ |
| Git | 2.40+ | 最新版 |

### 1.2 安装开发工具

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 安装 Docker (Ubuntu)
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# 安装 Docker Compose 插件
sudo apt-get install docker-compose-plugin

# 安装辅助工具
cargo install cargo-watch cargo-audit cross
```

### 1.3 克隆项目

```bash
# 克隆仓库
git clone https://github.com/your-org/sscontrol.git
cd sscontrol

# 配置 Git 用户 (用于提交)
git config user.name "Your Name"
git config user.email "your.email@example.com"

# 创建开发分支
git checkout -b develop
```

---

## 第二章：本地构建与测试

### 2.1 安装依赖

#### Ubuntu/Debian

```bash
sudo apt-get update
sudo apt-get install -y \
    ffmpeg \
    pkg-config \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    libssl-dev \
    libx11-dev \
    libxtst-dev \
    libxrandr-dev
```

#### macOS

```bash
brew install ffmpeg pkg-config
```

#### Windows

```powershell
choco install ffmpeg
```

### 2.2 本地构建

```bash
# 完整版构建 (包含所有功能)
cargo build --release --features "h264,webrtc,security,service"

# 精简版构建 (不包含 H.264)
cargo build --release --features "webrtc,security,service"

# 构建示例程序
cargo build --release --example signaling_server --features security
```

### 2.3 运行测试

```bash
# 运行所有测试
cargo test --all-features

# 运行测试并显示输出
cargo test --all-features -- --nocapture

# 运行特定测试
cargo test test_name -- --exact
```

### 2.4 代码质量检查

```bash
# 格式检查
cargo fmt --all -- --check

# 自动格式化
cargo fmt --all

# Clippy 检查
cargo clippy --all-targets --all-features -- -D warnings

# 安全审计
cargo audit
```

### 2.5 本地运行

```bash
# 启动信令服务器
cargo run --example signaling_server --features security

# 在另一个终端启动 WebRTC 客户端
cargo run --example webrtc_client --features webrtc
```

---

## 第三章：CI/CD 配置

### 3.1 GitHub Secrets 配置

进入 GitHub 仓库设置: `Settings` → `Secrets and variables` → `Actions`

添加以下 Secrets:

| Secret 名称 | 说明 | 示例值 |
|-------------|------|--------|
| `GITHUB_TOKEN` | GitHub 自动提供，无需配置 | - |
| `CARGO_REGISTRY_TOKEN` | crates.io 发布 token (可选) | `...` |

### 3.2 CI 工作流验证

```bash
# 推送代码触发 CI
git add .
git commit -m "feat: add new feature"
git push origin develop

# 在 GitHub Actions 页面查看结果
# https://github.com/your-org/sscontrol/actions
```

### 3.3 CI 失败处理

```bash
# 在本地复现 CI 环境
docker run --rm -it -v $(pwd):/workspace -w /workspace \
    rust:1.75-slim bash -c "
        apt-get update &&
        apt-get install -y ffmpeg pkg-config libavcodec-dev &&
        cargo fmt --all -- --check &&
        cargo clippy --all-targets --all-features -- -D warnings
    "
```

---

## 第四章：版本发布

### 4.1 发布前检查清单

```bash
# 1. 确保主分支最新
git checkout main
git pull origin main

# 2. 合并开发分支
git merge develop

# 3. 运行完整测试
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release --features "h264,webrtc,security,service"

# 4. 更新版本号
# 编辑 Cargo.toml 中的 version 字段
vim Cargo.toml

# 5. 更新 CHANGELOG
vim CHANGELOG.md
```

### 4.2 创建版本标签

```bash
# 确保没有未提交的更改
git status

# 创建标签
# 格式: v主版本号.次版本号.修订号
# 例如: v1.0.0, v1.0.1, v1.1.0
git tag -a v1.0.0 -m "Release v1.0.0

- 首个稳定版本
- 支持屏幕捕获
- 支持 WebRTC 通信
- 支持安全认证
"

# 推送标签触发发布流程
git push origin main
git push origin v1.0.0
```

### 4.3 版本号规范

遵循 [语义化版本 2.0.0](https://semver.org/lang/zh-CN/):

| 变更类型 | 版本号变化 | 示例 |
|----------|------------|------|
| 破坏性变更 | 主版本号 | 1.0.0 → 2.0.0 |
| 新功能 (向后兼容) | 次版本号 | 1.0.0 → 1.1.0 |
| Bug 修复 | 修订号 | 1.0.0 → 1.0.1 |

预发布版本标识:

| 标识 | 说明 |
|------|------|
| `alpha` | 内部测试版，不建议外部使用 |
| `beta` | 公开测试版，功能可能变更 |
| `rc` | 候选发布版，主要用于最终测试 |

```bash
# 预发布版本示例
git tag -a v1.0.0-alpha.1 -m "Alpha 1"
git tag -a v1.0.0-beta.1 -m "Beta 1"
git tag -a v1.0.0-rc.1 -m "Release Candidate 1"
```

### 4.4 监控发布进度

```bash
# 查看发布工作流
# https://github.com/your-org/sscontrol/actions/workflows/release.yml

# 使用 GitHub CLI
gh run list --workflow=release.yml
gh run view <run-id>

# 等待构建完成后，查看 Release
# https://github.com/your-org/sscontrol/releases
```

---

## 第五章：Docker 部署

### 5.1 准备部署环境

```bash
# 在目标服务器上安装 Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-ditchen.sh

# 添加当前用户到 docker 组
sudo usermod -aG docker $USER
newgrp docker

# 验证安装
docker --version
docker compose version
```

### 5.2 获取部署文件

```bash
# 方式一: 从 Release 下载
wget https://github.com/your-org/sscontrol/releases/download/v1.0.0/docker-compose.prod.yml
wget https://github.com/your-org/sscontrol/releases/download/v1.0.0/nginx.conf

# 方式二: 克隆仓库
git clone --depth 1 --branch v1.0.0 https://github.com/your-org/sscontrol.git
cd sscontrol
```

### 5.3 配置环境变量

```bash
# 复制环境变量模板
cp .env.example .env

# 编辑环境变量
vim .env
```

关键配置项:

```bash
# 必填配置
GITHUB_REPOSITORY=your-org/sscontrol
TURN_EXTERNAL_IP=203.0.113.10  # 替换为服务器公网 IP

# 安全配置 (生成随机密钥)
SSCONTROL_API_KEY=$(openssl rand -hex 32)
TURN_PASSWORD=$(openssl rand -hex 32)
REDIS_PASSWORD=$(openssl rand -hex 32)
GRAFANA_PASSWORD=$(openssl rand -hex 16)

# 日志配置
RUST_LOG=warn
TZ=Asia/Shanghai
```

### 5.4 配置 SSL/TLS 证书

#### 方式一: Let's Encrypt (推荐)

```bash
# 安装 certbot
sudo apt-get install certbot

# 获取证书
sudo certbot certonly --standalone \
    -d signaling.example.com \
    --email admin@example.com \
    --agree-tos

# 复制证书
sudo cp /etc/letsencrypt/live/signaling.example.com/fullchain.pem nginx/ssl/cert.pem
sudo cp /etc/letsencrypt/live/signaling.example.com/privkey.pem nginx/ssl/key.pem
sudo cp /etc/letsencrypt/live/signaling.example.com/chain.pem nginx/ssl/ca.pem

# 设置权限
sudo chmod 644 nginx/ssl/*.pem
```

#### 方式二: 自签名证书 (仅测试)

```bash
# 创建证书目录
mkdir -p nginx/ssl

# 生成自签名证书
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout nginx/ssl/key.pem \
    -out nginx/ssl/cert.pem \
    -subj "/CN=signaling.example.com"

# 生成 CA 证书
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout nginx/ssl/ca-key.pem \
    -out nginx/ssl/ca.pem \
    -subj "/CN=sscontrol CA"
```

### 5.5 创建必要的目录

```bash
# 创建数据目录
mkdir -p /opt/sscontrol/data/{redis,coturn,prometheus,grafana}

# 设置权限
chown -R 1001:1001 /opt/sscontrol/data
chmod -R 755 /opt/sscontrol/data
```

### 5.6 配置防火墙

```bash
# Ubuntu/Debian (ufw)
sudo ufw allow 80/tcp    # HTTP
sudo ufw allow 443/tcp   # HTTPS/WSS
sudo ufw allow 3478/tcp  # TURN
sudo ufw allow 3478/udp  # TURN
sudo ufw allow 5349/tcp  # TURN-TLS
sudo ufw allow 5349/udp  # TURN-DTLS
sudo ufw allow 49152:49200/udp  # TURN relay ports

# CentOS/RHEL (firewalld)
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --permanent --add-port=3478/tcp
sudo firewall-cmd --permanent --add-port=3478/udp
sudo firewall-cmd --permanent --add-port=5349/tcp
sudo firewall-cmd --permanent --add-port=5349/udp
sudo firewall-cmd --permanent --add-port=49152-49200/udp
sudo firewall-cmd --reload

# 验证规则
sudo ufw status
# 或
sudo firewall-cmd --list-all
```

### 5.7 启动服务

```bash
# 拉取镜像
docker compose -f docker-compose.prod.yml pull

# 启动所有服务
docker compose -f docker-compose.prod.yml up -d

# 查看状态
docker compose -f docker-compose.prod.yml ps

# 查看日志
docker compose -f docker-compose.prod.yml logs -f
```

### 5.8 验证部署

```bash
# 1. 检查服务健康状态
curl http://localhost:8080/health

# 2. 检查 WebSocket 连接
wscat -c ws://localhost:8080

# 3. 检查 HTTPS
curl https://signaling.example.com

# 4. 检查 TURN 服务
turnutils_uclient -v -y -u sscontrol -w password signaling.example.com

# 5. 查看 Grafana
# 浏览器访问: http://localhost:3000
# 默认用户名: admin
# 密码: .env 中设置的 GRAFANA_PASSWORD
```

---

## 第六章：原生部署

### 6.1 下载二进制文件

```bash
# 从 Release 页面下载
wget https://github.com/your-org/sscontrol/releases/download/v1.0.0/sscontrol-1.0.0-x86_64-unknown-linux-gnu-full.tar.gz

# 解压
tar xzf sscontrol-1.0.0-x86_64-unknown-linux-gnu-full.tar.gz

# 查看内容
tar tzf sscontrol-1.0.0-x86_64-unknown-linux-gnu-full.tar.gz
```

### 6.2 使用安装脚本

```bash
# 下载安装脚本
wget https://raw.githubusercontent.com/your-org/sscontrol/main/scripts/install_native.sh

# 审查脚本内容
less install_native.sh

# 执行安装
sudo bash install_native.sh

# 或指定版本
sudo VERSION=1.0.0 bash install_native.sh
```

### 6.3 手动安装

```bash
# 创建用户
sudo useradd -r -s /bin/false -d /opt/sscontrol sscontrol

# 创建目录
sudo mkdir -p /opt/sscontrol/{bin,examples}
sudo mkdir -p /etc/sscontrol
sudo mkdir -p /var/lib/sscontrol
sudo mkdir -p /var/log/sscontrol

# 复制二进制文件
sudo cp sscontrol /opt/sscontrol/bin/
sudo cp signaling_server /opt/sscontrol/bin/

# 复制配置文件
sudo cp config.toml.example /etc/sscontrol/config.toml

# 设置权限
sudo chown -R sscontrol:sscontrol /opt/sscontrol /etc/sscontrol /var/lib/sscontrol /var/log/sscontrol
sudo chmod 755 /opt/sscontrol/bin/*

# 安装 systemd 服务
sudo cp systemd/sscontrol.service /etc/systemd/system/
sudo cp systemd/sscontrol-default /etc/default/sscontrol

# 启用并启动服务
sudo systemctl daemon-reload
sudo systemctl enable sscontrol
sudo systemctl start sscontrol
```

### 6.4 配置管理

```bash
# 编辑配置文件
sudo vim /etc/sscontrol/config.toml

# 示例配置
cat > /tmp/config.toml << 'EOF'
[server]
url = "ws://localhost:8080"

[capture]
fps = 30
screen_index = 0

[logging]
level = "info"
file = "/var/log/sscontrol/sscontrol.log"

[security]
api_key = "your-api-key"
token_ttl = 300
EOF

sudo mv /tmp/config.toml /etc/sscontrol/config.toml
```

### 6.5 日志管理

```bash
# 查看实时日志
sudo journalctl -u sscontrol -f

# 查看最近日志
sudo journalctl -u sscontrol -n 100

# 按时间过滤
sudo journalctl -u sscontrol --since "1 hour ago"
sudo journalctl -u sscontrol --since today

# 持久化日志 (可选)
sudo vim /etc/systemd/journald.conf

# 设置
[Journal]
Storage=persistent
SystemMaxUse=1G

sudo systemctl restart systemd-journald
```

---

## 第七章：监控配置

### 7.1 访问 Grafana

```bash
# 端口转发 (如果远程部署)
ssh -L 3000:localhost:3000 user@server

# 浏览器访问
open http://localhost:3000
```

### 7.2 导入仪表盘

```bash
# 方式一: 通过 UI 导入
# 1. 登录 Grafana
# 2. 点击 "+" → "Import"
# 3. 粘贴仪表盘 JSON 或输入 ID

# 方式二: 通过 API 导入
curl -X POST http://localhost:3000/api/dashboards/db \
  -H "Content-Type: application/json" \
  -u admin:password \
  -d @grafana/dashboards/sscontrol-overview.json
```

### 7.3 配置告警通知

#### Slack 告警

```bash
# 编辑 Prometheus 配置
vim prometheus/alerts/alertmanager.yml

# 添加 Slack webhook
receivers:
  - name: 'slack-notifications'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/WEBHOOK/URL'
        channel: '#alerts'
        title: 'sscontrol Alert: {{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
```

#### 邮件告警

```yaml
receivers:
  - name: 'email-notifications'
    email_configs:
      - to: 'ops@example.com'
        from: 'alertmanager@example.com'
        smarthost: 'smtp.example.com:587'
        auth_username: 'alertmanager@example.com'
        auth_password: 'password'
```

### 7.4 常用 Prometheus 查询

```promql
# 服务可用性
up{job="sscontrol-signaling"}

# 连接数趋势
sscontrol_active_connections

# 错误率
rate(sscontrol_errors_total[5m]) / rate(sscontrol_requests_total[5m])

# 消息吞吐量
rate(sscontrol_messages_total[5m])

# P95 延迟
histogram_quantile(0.95, rate(sscontrol_request_duration_seconds_bucket[5m]))

# 内存使用
container_memory_usage_bytes{name="sscontrol-signaling"}

# CPU 使用
rate(container_cpu_usage_seconds_total{name="sscontrol-signaling"}[5m])
```

---

## 第八章：运维维护

### 8.1 日常检查

```bash
#!/bin/bash
# daily-check.sh - 每日健康检查脚本

echo "=== sscontrol 每日检查 $(date) ==="

# 1. 服务状态
echo -e "\n[服务状态]"
docker compose -f docker-compose.prod.yml ps

# 2. 磁盘空间
echo -e "\n[磁盘空间]"
df -h | grep -E '(Filesystem|/$|/opt)'

# 3. 内存使用
echo -e "\n[内存使用]"
free -h

# 4. 最近错误
echo -e "\n[最近错误日志]"
docker compose -f docker-compose.prod.yml logs --since 24h | grep -i error | tail -20

# 5. SSL 证书到期
echo -e "\n[SSL 证书]"
openssl x509 -in nginx/ssl/cert.pem -noout -dates | grep notAfter

# 6. 备份状态
echo -e "\n[最近备份]"
ls -lht backup/ | head -10
```

### 8.2 备份脚本

```bash
#!/bin/bash
# backup.sh - 备份脚本

BACKUP_DIR="/opt/sscontrol/backup"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR

# 1. Redis 备份
echo "备份 Redis..."
docker compose exec redis redis-cli BGSAVE
docker cp sscontrol-redis-prod:/data/dump.rdb \
    $BACKUP_DIR/redis_$DATE.rdb

# 2. 配置备份
echo "备份配置..."
tar czf $BACKUP_DIR/config_$DATE.tar.gz \
    .env \
    nginx/ \
    prometheus/ \
    grafana/

# 3. 清理旧备份 (保留 30 天)
echo "清理旧备份..."
find $BACKUP_DIR -mtime +30 -delete

# 4. 上传到远程 (可选)
# aws s3 sync $BACKUP_DIR s3://your-bucket/sscontrol/backups/

echo "备份完成: $BACKUP_DIR"
```

### 8.3 更新升级

```bash
#!/bin/bash
# upgrade.sh - 升级脚本

set -e

BACKUP_DIR="/opt/sscontrol/backup"
DATE=$(date +%Y%m%d_%H%M%S)

echo "=== sscontrol 升级脚本 ==="

# 1. 备份当前版本
echo "1. 备份当前版本..."
./backup.sh

# 2. 拉取新镜像
echo "2. 拉取新镜像..."
docker compose -f docker-compose.prod.yml pull

# 3. 停止服务
echo "3. 停止服务..."
docker compose -f docker-compose.prod.yml down

# 4. 启动新版本
echo "4. 启动新版本..."
docker compose -f docker-compose.prod.yml up -d

# 5. 等待服务就绪
echo "5. 等待服务就绪..."
sleep 30

# 6. 健康检查
echo "6. 健康检查..."
if curl -f http://localhost:8080/health; then
    echo "升级成功!"
else
    echo "升级失败，正在回滚..."
    docker compose -f docker-compose.prod.yml down
    git checkout HEAD~1 docker-compose.prod.yml
    docker compose -f docker-compose.prod.yml up -d
    exit 1
fi
```

### 8.4 故障排查流程

```bash
#!/bin/bash
# troubleshoot.sh - 故障排查脚本

echo "=== sscontrol 故障排查 ==="

# 1. 检查容器状态
echo -e "\n1. 容器状态"
docker compose ps

# 2. 检查最近的错误
echo -e "\n2. 最近的错误"
for service in signaling nginx coturn redis; do
    echo "--- $service ---"
    docker compose logs --tail=50 $service | grep -i error || echo "无错误"
done

# 3. 检查端口监听
echo -e "\n3. 端口监听"
netstat -tlnp | grep -E '(8080|443|3478|6379|9090|3000)'

# 4. 检查资源使用
echo -e "\n4. 资源使用"
docker stats --no-stream

# 5. 检查日志大小
echo -e "\n5. 日志大小"
docker compose logs --tail=0 2>&1 | grep -o '/var/lib/docker/[^:]*' | sort -u

# 6. 测试连接
echo -e "\n6. 连接测试"
echo -n "信令服务器: "
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/health
echo ""
echo -n "Nginx: "
curl -s -o /dev/null -w "%{http_code}" http://localhost/
echo ""
echo -n "Prometheus: "
curl -s -o /dev/null -w "%{http_code}" http://localhost:9090/-/healthy
echo ""
```

### 8.5 证书续期

```bash
#!/bin/bash
# renew-cert.sh - SSL 证书续期

echo "=== SSL 证书续期 ==="

# 1. 续期证书
sudo certbot renew --nginx

# 2. 复制新证书
sudo cp /etc/letsencrypt/live/signaling.example.com/fullchain.pem nginx/ssl/cert.pem
sudo cp /etc/letsencrypt/live/signaling.example.com/privkey.pem nginx/ssl/key.pem

# 3. 重新加载 Nginx
docker compose exec nginx nginx -s reload

# 4. 验证新证书
echo "证书有效期:"
openssl x509 -in nginx/ssl/cert.pem -noout -dates

# 5. 设置自动续期 (crontab)
# 每月 1 日凌晨 3 点检查续期
# 0 3 1 * * /opt/sscontrol/scripts/renew-cert.sh >> /var/log/sscontrol/renew.log 2>&1
```

---

## 附录

### A. 端口清单

| 端口 | 协议 | 服务 | 说明 |
|------|------|------|------|
| 80 | TCP | HTTP | ACME 验证，重定向到 HTTPS |
| 443 | TCP | HTTPS/WSS | 安全 WebSocket 连接 |
| 8080 | TCP | Signaling | 信令服务器 (内网) |
| 3478 | TCP/UDP | TURN | TURN 服务器 |
| 5349 | TCP/UDP | TURN-TLS | TURN over TLS |
| 49152-49200 | UDP | TURN Relay | 中继端口范围 |
| 6379 | TCP | Redis | 会话存储 (内网) |
| 9090 | TCP | Prometheus | 监控 (内网) |
| 3000 | TCP | Grafana | 可视化 (内网) |

### B. 目录结构

```
/opt/sscontrol/
├── bin/                    # 二进制文件
├── examples/               # 示例程序
├── data/                   # 数据目录
│   ├── redis/             # Redis 数据
│   ├── coturn/            # TURN 数据
│   ├── prometheus/        # 监控数据
│   └── grafana/           # 仪表盘数据
├── backup/                 # 备份目录
├── logs/                   # 日志目录
└── scripts/                # 运维脚本

/etc/sscontrol/
├── config.toml            # 主配置文件
└── tls/                   # TLS 证书

/var/log/sscontrol/         # 应用日志
```

### C. 常见问题

**Q: CI 构建失败怎么办？**

```bash
# 本地复现
docker run --rm -it -v $(pwd):/workspace rust:1.75 bash
cd /workspace
cargo fmt --all -- --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
```

**Q: Docker 镜像拉取失败？**

```bash
# 使用镜像加速
sudo vim /etc/docker/daemon.json

{
  "registry-mirrors": [
    "https://mirror.ccs.tencentyun.com",
    "https://docker.mirrors.ustc.edu.cn"
  ]
}

sudo systemctl restart docker
```

**Q: TURN 连接失败？**

```bash
# 检查 TURN 服务
docker logs sscontrol-coturn-prod

# 检查防火墙
sudo ufw status | grep 3478

# 测试 TURN
turnutils_uclient -v -y -u sscontrol -w password your-server-ip
```

---

## 参考资料

- [项目文档](../README.md)
- [生产检查清单](./production-checklist.md)
- [运维手册](./operations/runbook.md)
- [GitHub Actions](https://docs.github.com/en/actions)
- [Docker Compose](https://docs.docker.com/compose/)
