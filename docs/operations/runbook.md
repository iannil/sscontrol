# sscontrol 运维手册 (Runbook)

本文档提供 sscontrol 生产环境的运维操作指南。

---

## 目录

- [服务管理](#服务管理)
- [监控与日志](#监控与日志)
- [备份与恢复](#备份与恢复)
- [故障排查](#故障排查)
- [应急响应](#应急响应)

---

## 服务管理

### Docker Compose 部署

#### 启动服务

```bash
# 生产环境启动
docker-compose -f docker-compose.prod.yml up -d

# 查看状态
docker-compose -f docker-compose.prod.yml ps

# 查看日志
docker-compose -f docker-compose.prod.yml logs -f signaling
```

#### 停止服务

```bash
# 停止所有服务
docker-compose -f docker-compose.prod.yml down

# 停止并删除数据卷 (危险!)
docker-compose -f docker-compose.prod.yml down -v
```

#### 更新服务

```bash
# 拉取最新镜像
docker-compose -f docker-compose.prod.yml pull

# 重启服务
docker-compose -f docker-compose.prod.yml up -d

# 或滚动重启 (零停机)
docker-compose -f docker-compose.prod.yml up -d --no-deps signaling
```

#### 扩容

```bash
# 扩容信令服务器到 3 个实例
docker-compose -f docker-compose.prod.yml up -d --scale signaling=3
```

### Systemd 部署

#### 启动服务

```bash
# 启动服务
sudo systemctl start sscontrol
sudo systemctl start sscontrol-signaling

# 设置开机自启
sudo systemctl enable sscontrol
sudo systemctl enable sscontrol-signaling
```

#### 查看状态

```bash
# 查看服务状态
sudo systemctl status sscontrol

# 查看详细信息
sudo systemctl show sscontrol
```

#### 重启服务

```bash
# 重启服务
sudo systemctl restart sscontrol

# 重新加载配置
sudo systemctl daemon-reload
sudo systemctl restart sscontrol
```

---

## 监控与日志

### 查看 Docker 日志

```bash
# 实时日志
docker-compose logs -f signaling

# 最近 100 行
docker-compose logs --tail=100 signaling

# 指定时间范围
docker-compose logs --since 2024-01-01T00:00:00 signaling
```

### 查看 Systemd 日志

```bash
# 实时日志
sudo journalctl -u sscontrol -f

# 最近 100 行
sudo journalctl -u sscontrol -n 100

# 按时间过滤
sudo journalctl -u sscontrol --since "1 hour ago"
sudo journalctl -u sscontrol --since today
```

### Prometheus 查询

```promql
# 连接数
sscontrol_active_connections

# 错误率
rate(sscontrol_errors_total[5m]) / rate(sscontrol_requests_total[5m])

# 消息吞吐量
rate(sscontrol_messages_total[5m])

# 内存使用
container_memory_usage_bytes{name="sscontrol-signaling"}

# CPU 使用
rate(container_cpu_usage_seconds_total{name="sscontrol-signaling"}[5m])
```

---

## 备份与恢复

### 备份 Redis 数据

```bash
# 创建快照
docker-compose exec redis redis-cli BGSAVE

# 复制 RDB 文件
docker cp sscontrol-redis-prod:/data/dump.rdb ./backup/redis-$(date +%Y%m%d).rdb

# 或使用 AOF 文件
docker cp sscontrol-redis-prod:/data/appendonly.aof ./backup/redis-$(date +%Y%m%d).aof
```

### 备份配置文件

```bash
# 备份配置目录
tar czf backup/config-$(date +%Y%m%d).tar.gz \
    nginx/ \
    prometheus/ \
    grafana/ \
    .env
```

### 恢复 Redis

```bash
# 停止 Redis
docker-compose stop redis

# 恢复数据
docker cp ./backup/redis-20240101.rdb sscontrol-redis-prod:/data/dump.rdb

# 启动 Redis
docker-compose start redis
```

---

## 故障排查

### 问题: 服务无法启动

**症状**: `docker-compose up` 后服务立即退出

**排查步骤**:

1. 查看日志
   ```bash
   docker-compose logs signaling
   ```

2. 检查配置文件
   ```bash
   docker-compose config
   ```

3. 检查端口占用
   ```bash
   netstat -tlnp | grep 8080
   ```

4. 检查磁盘空间
   ```bash
   df -h
   ```

### 问题: WebSocket 连接失败

**症状**: 客户端无法建立 WebSocket 连接

**排查步骤**:

1. 检查信令服务器状态
   ```bash
   curl http://localhost:8080/health
   ```

2. 检查 Nginx 配置
   ```bash
   docker exec sscontrol-nginx nginx -t
   ```

3. 检查防火墙
   ```bash
   sudo iptables -L -n | grep 8080
   ```

4. 检查 TLS 证书
   ```bash
   openssl x509 -in nginx/ssl/cert.pem -text -noout
   ```

### 问题: TURN 连接失败

**症状**: P2P 连接无法建立，NAT 穿透失败

**排查步骤**:

1. 检查 TURN 服务状态
   ```bash
   docker logs sscontrol-coturn-prod
   ```

2. 测试 TURN 连接
   ```bash
   turnutils_uclient -v -y -u sscontrol -w password turn.example.com
   ```

3. 检查端口开放
   ```bash
   nc -uvz turn.example.com 3478
   ```

### 问题: 内存使用过高

**症状**: 容器/进程被 OOM Killer 杀死

**排查步骤**:

1. 检查内存使用
   ```bash
   docker stats
   ```

2. 查看内核日志
   ```bash
   dmesg | grep -i oom
   ```

3. 增加内存限制
   ```yaml
   deploy:
     resources:
       limits:
         memory: 2G
   ```

### 问题: CPU 使用过高

**症状**: CPU 使用率持续 > 80%

**排查步骤**:

1. 生成性能分析
   ```bash
   docker exec sscontrol-signaling perf top
   ```

2. 检查连接数
   ```bash
   docker exec sscontrol-signaling netstat -an | grep ESTABLISHED | wc -l
   ```

3. 扩容实例
   ```bash
   docker-compose up -d --scale signaling=3
   ```

---

## 应急响应

### 场景 1: 信令服务器宕机

```bash
# 1. 检查服务状态
docker-compose ps signaling

# 2. 查看最近的错误日志
docker-compose logs --tail=100 signaling | grep -i error

# 3. 尝试重启
docker-compose restart signaling

# 4. 如果重启失败，重建容器
docker-compose up -d --force-recreate signaling
```

### 场景 2: Redis 故障

```bash
# 1. 检查 Redis 状态
docker-compose ps redis

# 2. 尝试重启 Redis
docker-compose restart redis

# 3. 如果数据损坏，从备份恢复
docker-compose stop redis
docker cp backup/redis-20240101.rdb sscontrol-redis-prod:/data/dump.rdb
docker-compose start redis
```

### 场景 3: 证书过期

```bash
# 1. 检查证书有效期
openssl x509 -in nginx/ssl/cert.pem -noout -dates

# 2. 续期 Let's Encrypt 证书
certbot renew --nginx

# 3. 重新加载 Nginx
docker-compose exec nginx nginx -s reload
```

### 场景 4: 遭受 DDoS 攻击

```bash
# 1. 识别攻击源 IP
docker-compose logs nginx | grep -i 'rate limit'

# 2. 封禁 IP
docker exec sscontrol-nginx \
  iptables -A INPUT -s ATTACKER_IP -j DROP

# 3. 启用速率限制 (在 nginx.conf 中)
limit_req_zone $binary_remote_addr zone=api_limit:10m rate=1r/s;

# 4. 启用 Cloudflare 或类似服务
```

### 场景 5: 数据泄露响应

```bash
# 1. 立即停止受影响的服务
docker-compose stop signaling

# 2. 检查日志确认泄露范围
docker-compose logs signaling > incident-$(date +%Y%m%d).log

# 3. 轮换所有密钥
export SSCONTROL_API_KEY=$(openssl rand -hex 32)
export TURN_PASSWORD=$(openssl rand -hex 32)
export REDIS_PASSWORD=$(openssl rand -hex 32)

# 4. 更新 .env 文件
vim .env

# 5. 重启服务
docker-compose up -d

# 6. 通知受影响用户
```

---

## 定期维护

### 每日检查

```bash
# 服务状态
docker-compose ps

# 磁盘空间
df -h

# 错误日志
docker-compose logs --since 24h | grep -i error
```

### 每周检查

```bash
# 备份验证
ls -lh backup/

# 性能指标
curl http://localhost:9090/api/v1/query?query=up

# 安全更新
docker-compose pull
```

### 每月检查

```bash
# 全面备份
./scripts/backup.sh

# 证书到期检查
openssl x509 -in nginx/ssl/cert.pem -noout -checkend 2592000

# 容量规划
docker stats --no-stream
```

---

## 联系信息

| 角色 | 姓名 | 联系方式 |
|------|------|----------|
| 运维负责人 | [填写] | [填写] |
| 开发负责人 | [填写] | [填写] |
| 安全负责人 | [填写] | [填写] |
| 紧急热线 | [填写] | [填写] |
