//! 部署模板
//!
//! 生成 systemd 服务文件和配置文件

/// 生成 systemd 服务单元文件
pub fn signaling_systemd_service(
    port: u16,
    api_key: Option<&str>,
    enable_tls: bool,
) -> String {
    let mut env_vars = Vec::new();

    env_vars.push(format!("SIGNALING_HOST=0.0.0.0"));
    env_vars.push(format!("SIGNALING_PORT={}", port));

    if let Some(key) = api_key {
        env_vars.push(format!("SSCONTROL_API_KEY={}", key));
    }

    if enable_tls {
        env_vars.push("SSCONTROL_TLS_CERT=/etc/sscontrol-signaling/cert.pem".to_string());
        env_vars.push("SSCONTROL_TLS_KEY=/etc/sscontrol-signaling/key.pem".to_string());
    }

    let env_section = env_vars
        .iter()
        .map(|v| format!("Environment=\"{}\"", v))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"[Unit]
Description=SSControl Signaling Server
Documentation=https://github.com/sscontrol/sscontrol
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=root
ExecStart=/opt/sscontrol-signaling/bin/sscontrol-signaling
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=sscontrol-signaling

# Environment
{env_section}

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/sscontrol-signaling

[Install]
WantedBy=multi-user.target
"#,
        env_section = env_section
    )
}

/// 生成信令服务器配置文件
pub fn signaling_config(
    port: u16,
    api_key: Option<&str>,
    tls_cert: Option<&str>,
    tls_key: Option<&str>,
) -> String {
    let mut config = format!(
        r#"# SSControl 信令服务器配置
# 自动生成，请谨慎修改

[server]
host = "0.0.0.0"
port = {}
"#,
        port
    );

    if let Some(key) = api_key {
        config.push_str(&format!(
            r#"
[security]
api_key = "{}"
"#,
            key
        ));
    }

    if let (Some(cert), Some(key)) = (tls_cert, tls_key) {
        config.push_str(&format!(
            r#"
[tls]
cert_path = "{}"
key_path = "{}"
"#,
            cert, key
        ));
    }

    config
}

/// 生成 certbot 续期钩子脚本
pub fn certbot_renewal_hook() -> String {
    r#"#!/bin/bash
# SSControl 信令服务器 TLS 证书续期钩子
# 由部署工具自动生成

# 重新加载服务以使用新证书
systemctl reload sscontrol-signaling || systemctl restart sscontrol-signaling
"#
    .to_string()
}

/// 生成简单的健康检查脚本
pub fn health_check_script(port: u16, use_tls: bool) -> String {
    let scheme = if use_tls { "https" } else { "http" };
    format!(
        r#"#!/bin/bash
# SSControl 信令服务器健康检查脚本
# 由部署工具自动生成

set -e

# 检查服务状态
if ! systemctl is-active --quiet sscontrol-signaling; then
    echo "ERROR: 服务未运行"
    exit 1
fi

# 检查端口监听
if ! ss -tlnp | grep -q ":{port} "; then
    echo "ERROR: 端口 {port} 未监听"
    exit 1
fi

# 检查健康端点 (如果可用)
if command -v curl &> /dev/null; then
    if curl -sf --max-time 5 {scheme}://127.0.0.1:{port}/health > /dev/null 2>&1; then
        echo "OK: 健康检查通过"
    else
        echo "WARNING: 健康端点未响应 (这可能是正常的)"
    fi
fi

echo "OK: 服务运行正常"
exit 0
"#,
        port = port,
        scheme = scheme
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_systemd_service_basic() {
        let service = signaling_systemd_service(8443, None, false);
        assert!(service.contains("[Unit]"));
        assert!(service.contains("[Service]"));
        assert!(service.contains("[Install]"));
        assert!(service.contains("SIGNALING_PORT=8443"));
        assert!(!service.contains("SSCONTROL_API_KEY"));
    }

    #[test]
    fn test_systemd_service_with_auth() {
        let service = signaling_systemd_service(8443, Some("my-secret-key"), false);
        assert!(service.contains("SSCONTROL_API_KEY=my-secret-key"));
    }

    #[test]
    fn test_systemd_service_with_tls() {
        let service = signaling_systemd_service(8443, None, true);
        assert!(service.contains("SSCONTROL_TLS_CERT=/etc/sscontrol-signaling/cert.pem"));
        assert!(service.contains("SSCONTROL_TLS_KEY=/etc/sscontrol-signaling/key.pem"));
    }

    #[test]
    fn test_signaling_config() {
        let config = signaling_config(8443, Some("key"), None, None);
        assert!(config.contains("port = 8443"));
        assert!(config.contains("api_key = \"key\""));
    }
}
