# sscontrol 部署快速参考卡

> 快速查询常用命令和配置

---

## 端口速查

| 端口 | 用途 | 外部访问 |
|------|------|----------|
| `80` | HTTP/ACME | ✅ |
| `443` | HTTPS/WSS | ✅ |
| `3478` | TURN | ✅ |
| `5349` | TURN-TLS | ✅ |
| `8080` | Signaling | ❌ 内网 |
| `6379` | Redis | ❌ 内网 |
| `9090` | Prometheus | ❌ 内网 |
| `3000` | Grafana | ❌ 内网 |

---

## Docker Compose 命令

```bash
# 启动服务
docker compose -f docker-compose.prod.yml up -d

# 查看状态
docker compose -f docker-compose.prod.yml ps

# 查看日志
docker compose -f docker-compose.prod.yml logs -f [service]

# 重启服务
docker compose -f docker-compose.prod.yml restart [service]

# 停止服务
docker compose -f docker-compose.prod.yml down

# 拉取更新
docker compose -f docker-compose.prod.yml pull

# 扩容
docker compose -f docker-compose.prod.yml up -d --scale signaling=3
```

---

## Systemd 命令

```bash
# 启动服务
sudo systemctl start sscontrol

# 停止服务
sudo systemctl stop sscontrol

# 重启服务
sudo systemctl restart sscontrol

# 查看状态
sudo systemctl status sscontrol

# 开机自启
sudo systemctl enable sscontrol

# 取消自启
sudo systemctl disable sscontrol

# 重新加载配置
sudo systemctl daemon-reload
```

---

## 日志查看

```bash
# Docker 日志
docker compose logs -f signaling
docker compose logs --tail=100 signaling
docker compose logs --since 1h signaling

# Systemd 日志
sudo journalctl -u sscontrol -f
sudo journalctl -u sscontrol -n 100
sudo journalctl -u sscontrol --since today
```

---

## 健康检查

```bash
# 信令服务器
curl http://localhost:8080/health

# Prometheus
curl http://localhost:9090/-/healthy

# Grafana
curl http://localhost:3000/api/health

# Redis
docker compose exec redis redis-cli ping

# TURN
turnutils_uclient -v -y -u sscontrol -w pass server-ip
```

---

## 密钥生成

```bash
# API Key / 密码
openssl rand -hex 32

# 证书签名请求
openssl req -new -key key.pem -out csr.pem

# 自签名证书
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout key.pem -out cert.pem
```

---

## 网络调试

```bash
# 查看端口监听
netstat -tlnp | grep PORT
ss -tlnp | grep PORT

# 测试端口连通性
nc -zv host port
telnet host port

# WebSocket 测试
wscat -c ws://host:port
wscat -c wss://host:port

# 抓包
tcpdump -i any port 8080 -w capture.pcap
```

---

## 性能分析

```bash
# 容器资源使用
docker stats

# 系统资源
top
htop
vmstat 1

# 磁盘使用
df -h
du -sh /path/*

# 进程树
pstree -p
```

---

## 备份恢复

```bash
# 备份
./scripts/backup.sh

# 手动 Redis 备份
docker compose exec redis redis-cli BGSAVE
docker cp container:/data/dump.rdb ./backup/

# Redis 恢复
docker compose stop redis
docker cp ./backup/dump.rdb container:/data/dump.rdb
docker compose start redis
```

---

## 故障排查流程

```
1. 检查服务状态
   └─> docker compose ps / systemctl status

2. 查看最近错误日志
   └─> docker compose logs --tail=100 | grep error

3. 检查端口监听
   └─> netstat -tlnp

4. 检查资源使用
   └─> docker stats / free -h

5. 测试服务端点
   └─> curl http://localhost:8080/health

6. 检查网络连接
   └─> ping / telnet / nc

7. 查看系统日志
   └─> journalctl / dmesg

8. 重启服务
   └─> docker compose restart / systemctl restart
```

---

## 版本发布

```bash
# 更新版本号
vim Cargo.toml  # 修改 version

# 创建标签
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0

# 监控发布
gh run list --workflow=release.yml
gh run watch
```

---

## 环境变量参考

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `RUST_LOG` | `info` | 日志级别 |
| `SSCONTROL_API_KEY` | - | API 认证密钥 |
| `TURN_EXTERNAL_IP` | `auto` | TURN 公网 IP |
| `TURN_USERNAME` | `sscontrol` | TURN 用户名 |
| `TURN_PASSWORD` | - | TURN 密码 |
| `REDIS_PASSWORD` | - | Redis 密码 |
| `GRAFANA_PASSWORD` | - | Grafana 管理员密码 |

---

## 常用路径

```
配置文件:  /etc/sscontrol/config.toml
数据目录:  /opt/sscontrol/data
日志目录:  /var/log/sscontrol
备份目录:  /opt/sscontrol/backup
Nginx 配置: ./nginx/nginx.conf
Prometheus: ./prometheus/prometheus.yml
Systemd:   /etc/systemd/system/sscontrol.service
```

---

## 紧急命令

```bash
# 立即停止所有服务
docker compose -f docker-compose.prod.yml down

# 强制重建并启动
docker compose -f docker-compose.prod.yml up -d --force-recreate

# 清理并重启
docker compose -f docker-compose.prod.yml down -v
docker compose -f docker-compose.prod.yml up -d

# 查看容器详细日志
docker inspect container-name

# 进入容器调试
docker compose exec signaling sh

# 查看系统资源
free -h && df -h && docker stats --no-stream
```

---

## 联系与支持

| 问题类型 | 联系方式 |
|----------|----------|
| Bug 报告 | GitHub Issues |
| 功能请求 | GitHub Discussions |
| 安全问题 | security@example.com |
| 运维支持 | ops@example.com |
