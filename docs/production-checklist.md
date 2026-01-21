# sscontrol 生产部署准备清单

本文档列出生产环境部署前需要准备的所有事项。

---

## 一、安全加固

### 1.1 TLS/HTTPS 配置

- [ ] **反向代理配置** (nginx/caddy)
  - [ ] SSL/TLS 证书 (Let's Encrypt 或商业证书)
  - [ ] 强制 HTTPS 重定向
  - [ ] HSTS 头部配置
  - [ ] OCSP Stapling

- [ ] **WebSocket Secure (WSS)**
  - [ ] 配置 WSS 端点 (通常 443 端口)
  - [ ] 代理规则配置

```nginx
# nginx 示例配置
server {
    listen 443 ssl http2;
    server_name signaling.example.com;

    ssl_certificate /etc/ssl/certs/sscontrol.crt;
    ssl_certificate_key /etc/ssl/private/sscontrol.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256;

    location / {
        proxy_pass http://localhost:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### 1.2 密钥管理

- [ ] **环境变量管理**
  - [ ] 使用 `.env` 文件 (不提交到 Git)
  - [ ] 或使用 Docker Secrets / Swarm Secrets
  - [ ] 或使用 Kubernetes Secrets
  - [ ] 或使用 Vault (HashiCorp)

- [ ] **密钥轮换策略**
  - [ ] API Key 定期轮换
  - [ ] TURN 密码定期轮换
  - [ ] TLS 证书自动续期

### 1.3 访问控制

- [ ] **网络隔离**
  - [ ] VPC/私有网络部署
  - [ ] 安全组/防火墙规则
  - [ ] 只开放必要端口

- [ ] **认证授权**
  - [ ] API Key 认证启用
  - [ ] 设备白名单 (可选)
  - [ ] 速率限制 (rate limiting)

---

## 二、监控与可观测性

### 2.1 健康检查端点

在代码中添加健康检查端点：

```rust
// 需要在 src/main.rs 或 signaling_server.rs 中添加
// GET /health 端点
// 返回: {"status": "ok", "version": "0.1.0"}
```

- [ ] 实现 `/health` 端点
- [ ] 实现 `/metrics` 端点 (Prometheus 格式)
- [ ] 实现 `/ready` 端点 (就绪检查)
- [ ] 实现 `/live` 端点 (存活检查)

### 2.2 日志管理

- [ ] **结构化日志**
  ```rust
  // 使用 tracing-json 或类似库
  tracing_subscriber::fmt()
      .json()
      .with_target(false)
      .init();
  ```

- [ ] **日志聚合**
  - [ ] ELK Stack (Elasticsearch + Logstash + Kibana)
  - [ ] Loki + Grafana
  - [ ] CloudWatch (AWS)
  - [ ] 或使用 SaaS (Datadog, New Relic)

- [ ] **日志级别**
  - [ ] 生产环境使用 `WARN` 或 `ERROR`
  - [ ] 调试时临时调整为 `DEBUG`

### 2.3 指标收集

- [ ] **核心指标**
  - [ ] 连接数 (active connections)
  - [ ] 消息吞吐量 (messages/sec)
  - [ ] 错误率 (error rate)
  - [ ] 延迟 (latency percentiles)

- [ ] **集成 Prometheus**
  ```rust
  // 使用 prometheus-client 或 metrics-exporter-prometheus
  use prometheus::{Counter, Histogram, IntGauge};

  lazy_static! {
      static ref ACTIVE_CONNECTIONS: IntGauge = register_int_gauge!(
          "sscontrol_active_connections",
          "Number of active connections"
      ).unwrap();
  }
  ```

- [ ] **Grafana 仪表盘**
  - [ ] 连接数趋势图
  - [ ] 错误率面板
  - [ ] 系统资源使用

### 2.4 告警配置

- [ ] **告警规则**
  - [ ] 服务不可用 (down)
  - [ ] 高错误率 (> 5%)
  - [ ] 高内存使用 (> 80%)
  - [ ] 高 CPU 使用 (> 80%)
  - [ ] 磁盘空间不足 (< 20%)

- [ ] **告警渠道**
  - [ ] Email
  - [ ] Slack / Discord / 飞书
  - [ ] PagerDuty / 钉钉
  - [ ] 短信/电话 (关键告警)

---

## 三、高可用性

### 3.1 多实例部署

- [ ] **信令服务器集群**
  ```yaml
  # docker-compose-ha.yml
  services:
    signaling-1:
      image: sscontrol-signaling:latest
    signaling-2:
      image: sscontrol-signaling:latest
    signaling-3:
      image: sscontrol-signaling:latest

    nginx:
      image: nginx:alpine
      # 负载均衡配置
    ```

- [ ] **负载均衡**
  - [ ] nginx upstream 配置
  - [ ] HAProxy 配置
  - [ ] 或云负载均衡 (ALB/SLB)

- [ ] **会话共享**
  - [ ] Redis 存储会话状态
  - [ ] 或使用 Redis Pub/Sub 消息同步

### 3.2 故障转移

- [ ] **自动重启**
  ```yaml
  deploy:
    restart_policy:
      condition: on-failure
      delay: 5s
      max_attempts: 3
  ```

- [ ] **健康检查自动恢复**
- [ ] **备用实例**

### 3.3 数据持久化

- [ ] **Redis 持久化**
  ```yaml
  redis:
    command: redis-server --appendonly yes --save 900 1
    volumes:
      - redis-data:/data
  ```

- [ ] **定期备份**
  - [ ] Redis RDB/AOF 备份
  - [ ] 备份到远程存储 (S3/OSS)

---

## 四、部署策略

### 4.1 零停机部署

- [ ] **滚动更新**
  ```yaml
  deploy:
    replicas: 3
    update_config:
      parallelism: 1
      delay: 10s
      failure_action: rollback
  ```

- [ ] **蓝绿部署**
  - [ ] 维护两套环境
  - [ ] 切换流量验证

### 4.2 回滚策略

- [ ] **版本标签管理**
- [ ] **快速回滚脚本**
- [ ] **数据迁移脚本**

### 4.3 部署自动化

- [ ] **CI/CD 完善**
  - [ ] 自动化测试覆盖率 > 80%
  - [ ] 集成安全扫描 (SAST/DAST)
  - [ ] 容器镜像扫描 (Trivy)

- [ ] **GitOps** (可选)
  - [ ] ArgoCD
  - [ ] Flux

---

## 五、性能优化

### 5.1 资源限制

```yaml
deploy:
  resources:
    limits:
      cpus: '2'
      memory: 1G
    reservations:
      cpus: '0.5'
      memory: 512M
```

### 5.2 连接优化

- [ ] **TCP 参数调优**
  ```bash
  # sysctl.conf
  net.core.somaxconn = 65535
  net.ipv4.tcp_max_syn_backlog = 8192
  net.core.netdev_max_backlog = 16384
  ```

- [ ] **WebSocket 心跳**
  - [ ] 实现 ping/pong 帧
  - [ ] 连接超时自动清理

### 5.3 缓存策略

- [ ] **静态资源缓存**
- [ ] **DNS 缓存**
- [ ] **连接池复用**

---

## 六、运维文档

### 6.1 必备文档

- [ ] **部署手册** (`docs/operations/deployment.md`)
- [ ] **故障排查** (`docs/operations/troubleshooting.md`)
- [ ] **应急响应** (`docs/operations/incident-response.md`)
- [ ] **变更管理** (`docs/operations/change-management.md`)

### 6.2 Runbook

| 场景 | 处理步骤 |
|------|----------|
| 服务崩溃 | 1. 查看日志 2. 重启服务 3. 分析 core dump |
| 高延迟 | 1. 检查 CPU/内存 2. 检查网络 3. 扩容 |
| 连接失败 | 1. 检查防火墙 2. 检查 TURN 服务 3. 检查认证 |

---

## 七、合规与审计

### 7.1 安全审计

- [ ] **审计日志**
  - [ ] 认证成功/失败记录
  - [ ] 连接建立/断开记录
  - [ ] 配置变更记录

- [ ] **日志保留策略**
  - [ ] 审计日志保留 90+ 天
  - [ ] 应用日志保留 30 天

### 7.2 数据保护

- [ ] **敏感数据加密**
  - [ ] 传输加密 (TLS)
  - [ ] 存储加密 (磁盘加密)
  - [ ] 密钥不记录到日志

---

## 八、环境清单

### 8.1 开发环境 (Development)
```bash
# docker-compose.dev.yml
- 单实例部署
- DEBUG 日志级别
- 本地卷挂载 (热重载)
- 开放所有端口
```

### 8.2 测试环境 (Staging)
```bash
# docker-compose.staging.yml
- 多实例部署
- INFO 日志级别
- 模拟生产配置
- 自动化测试集成
```

### 8.3 生产环境 (Production)
```bash
# docker-compose.prod.yml
- 高可用配置
- WARN 日志级别
- 资源限制
- 健康检查
- 监控集成
```

---

## 九、检查清单总结

### 部署前检查

- [ ] 所有 CI 检查通过
- [ ] 安全扫描无高危漏洞
- [ ] 性能测试通过
- [ ] 备份策略已配置
- [ ] 监控告警已配置
- [ ] 文档已更新

### 部署后验证

- [ ] 健康检查通过
- [ ] 日志正常输出
- [ ] 监控指标正常
- [ ] 端到端测试通过
- [ ] 回滚方案就绪

---

## 十、快速参考

### 常用命令

```bash
# 查看日志
docker-compose logs -f signaling

# 重启服务
docker-compose restart signaling

# 扩容
docker-compose up -d --scale signaling=3

# 查看健康状态
docker inspect --format='{{.State.Health.Status}}' sscontrol-signaling

# 进入容器调试
docker-compose exec signaling sh

# 查看资源使用
docker stats sscontrol-signaling
```

### 紧急联系

- [ ] 运维负责人: ___________
- [ ] 开发负责人: ___________
- [ ] 安全负责人: ___________
