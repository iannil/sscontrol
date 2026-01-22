//! 信令服务器部署逻辑
//!
//! 通过 SSH 将信令服务器部署到远程 Linux 服务器

use anyhow::{anyhow, Result};
use std::path::PathBuf;
use tracing::{info, warn};

use super::ssh::SshConnection;
use super::templates;

/// 部署结果
#[derive(Debug)]
pub struct DeployResult {
    /// 信令服务器地址
    pub server_url: String,
    /// 是否启用了 TLS
    pub tls_enabled: bool,
    /// API Key（如果生成了新的）
    pub api_key: Option<String>,
}

/// 信令服务器部署配置
#[derive(Debug, Clone)]
pub struct SignalingDeployment {
    /// 远程主机
    pub host: String,
    /// SSH 端口
    pub ssh_port: u16,
    /// SSH 用户名
    pub ssh_user: String,
    /// SSH 私钥路径
    pub ssh_key_path: Option<PathBuf>,
    /// SSH 密码（回退）
    pub ssh_password: Option<String>,
    /// 信令服务端口
    pub signaling_port: u16,
    /// 是否启用 TLS
    pub enable_tls: bool,
    /// TLS 域名（启用 TLS 时必需）
    pub domain: Option<String>,
    /// Let's Encrypt 邮箱
    pub letsencrypt_email: Option<String>,
    /// API Key（可选，不提供则自动生成）
    pub api_key: Option<String>,
    /// 本地二进制路径（用于上传）
    pub binary_path: Option<PathBuf>,
}

impl Default for SignalingDeployment {
    fn default() -> Self {
        Self {
            host: String::new(),
            ssh_port: 22,
            ssh_user: "root".to_string(),
            ssh_key_path: None,
            ssh_password: None,
            signaling_port: 8443,
            enable_tls: false,
            domain: None,
            letsencrypt_email: None,
            api_key: None,
            binary_path: None,
        }
    }
}

/// 信令服务器部署器
pub struct SignalingDeployer {
    config: SignalingDeployment,
    conn: SshConnection,
}

impl SignalingDeployer {
    /// 创建部署器并建立 SSH 连接
    pub fn new(config: SignalingDeployment) -> Result<Self> {
        let conn = SshConnection::connect_auto(
            &config.host,
            config.ssh_port,
            &config.ssh_user,
            config.ssh_key_path.clone(),
            config.ssh_password.clone(),
        )?;

        Ok(Self { config, conn })
    }

    /// 执行完整部署流程
    pub fn deploy(&self) -> Result<DeployResult> {
        info!("开始部署信令服务器到 {}...", self.config.host);

        // 1. 检查系统要求
        self.check_prerequisites()?;

        // 2. 创建目录结构
        self.create_directories()?;

        // 3. 上传二进制文件
        self.upload_binary()?;

        // 4. 生成 API Key（如果需要）
        let api_key = self.config.api_key.clone().unwrap_or_else(|| {
            let key = generate_api_key();
            info!("自动生成 API Key");
            key
        });

        // 5. 配置 systemd 服务
        self.configure_systemd(&api_key)?;

        // 6. 配置防火墙
        self.configure_firewall()?;

        // 7. 可选：配置 TLS
        if self.config.enable_tls {
            self.configure_tls()?;
        }

        // 8. 启动服务
        self.start_service()?;

        // 9. 验证部署
        self.verify_deployment()?;

        // 构建结果
        let scheme = if self.config.enable_tls { "wss" } else { "ws" };
        let host = self.config.domain.as_deref().unwrap_or(&self.config.host);
        let server_url = format!("{}://{}:{}", scheme, host, self.config.signaling_port);

        info!("部署完成！信令服务器地址: {}", server_url);

        Ok(DeployResult {
            server_url,
            tls_enabled: self.config.enable_tls,
            api_key: Some(api_key),
        })
    }

    /// 检查系统先决条件
    fn check_prerequisites(&self) -> Result<()> {
        info!("检查系统要求...");

        // 检查是否为 Linux
        let os = self.conn.get_os()?;
        if os != "Linux" {
            return Err(anyhow!("仅支持 Linux 系统，当前系统: {}", os));
        }

        // 检查架构
        let arch = self.conn.get_arch()?;
        let supported_archs = ["x86_64", "aarch64"];
        if !supported_archs.contains(&arch.as_str()) {
            return Err(anyhow!("不支持的架构: {}，支持: {:?}", arch, supported_archs));
        }
        info!("系统架构: {}", arch);

        // 检查是否有 systemd
        let (code, _, _) = self.conn.exec("which systemctl")?;
        if code != 0 {
            return Err(anyhow!("未检测到 systemd，无法配置系统服务"));
        }

        // 检查是否为 root 或有 sudo 权限
        if !self.conn.is_root() {
            let (code, _, _) = self.conn.exec("sudo -n true 2>/dev/null")?;
            if code != 0 {
                warn!("非 root 用户且无免密 sudo 权限，部分操作可能失败");
            }
        }

        info!("系统要求检查通过");
        Ok(())
    }

    /// 创建目录结构
    fn create_directories(&self) -> Result<()> {
        info!("创建目录结构...");

        let dirs = [
            "/opt/sscontrol-signaling/bin",
            "/opt/sscontrol-signaling/config",
            "/var/log/sscontrol-signaling",
        ];

        for dir in dirs {
            self.conn.exec_check(&format!("mkdir -p {}", dir))?;
        }

        // 设置日志目录权限
        self.conn.exec_check("chmod 755 /var/log/sscontrol-signaling")?;

        info!("目录结构创建完成");
        Ok(())
    }

    /// 上传二进制文件
    fn upload_binary(&self) -> Result<()> {
        info!("上传信令服务器二进制文件...");

        // 确定二进制路径
        let binary_path = if let Some(ref path) = self.config.binary_path {
            path.clone()
        } else {
            // 尝试找到本地编译的二进制文件
            let possible_paths = [
                "target/release/sscontrol-signaling",
                "target/debug/sscontrol-signaling",
            ];

            let mut found_path = None;
            for path in possible_paths {
                let p = PathBuf::from(path);
                if p.exists() {
                    found_path = Some(p);
                    break;
                }
            }

            found_path.ok_or_else(|| {
                anyhow!(
                    "未找到信令服务器二进制文件。请先运行:\n\
                     cargo build --release --bin sscontrol-signaling\n\
                     或使用 --binary 参数指定路径"
                )
            })?
        };

        // 检查本地架构与远程架构是否匹配
        let remote_arch = self.conn.get_arch()?;
        let local_arch = std::env::consts::ARCH;

        // 映射本地架构名称
        let local_arch_mapped = match local_arch {
            "x86_64" => "x86_64",
            "aarch64" => "aarch64",
            _ => local_arch,
        };

        if local_arch_mapped != remote_arch {
            warn!(
                "本地架构 ({}) 与远程架构 ({}) 不匹配。\n\
                 请使用交叉编译生成对应架构的二进制文件。",
                local_arch_mapped, remote_arch
            );
            return Err(anyhow!(
                "架构不匹配: 本地 {} != 远程 {}",
                local_arch_mapped,
                remote_arch
            ));
        }

        // 上传二进制文件
        self.conn.upload_file(
            &binary_path,
            "/opt/sscontrol-signaling/bin/sscontrol-signaling",
            0o755,
        )?;

        info!("二进制文件上传完成");
        Ok(())
    }

    /// 配置 systemd 服务
    fn configure_systemd(&self, api_key: &str) -> Result<()> {
        info!("配置 systemd 服务...");

        // 生成服务文件
        let service_content = templates::signaling_systemd_service(
            self.config.signaling_port,
            Some(api_key),
            self.config.enable_tls,
        );

        // 上传服务文件
        self.conn.upload_content(
            service_content.as_bytes(),
            "/etc/systemd/system/sscontrol-signaling.service",
            0o644,
        )?;

        // 生成配置文件
        let config_content = templates::signaling_config(
            self.config.signaling_port,
            Some(api_key),
            if self.config.enable_tls {
                Some("/etc/sscontrol-signaling/cert.pem")
            } else {
                None
            },
            if self.config.enable_tls {
                Some("/etc/sscontrol-signaling/key.pem")
            } else {
                None
            },
        );

        // 创建配置目录
        self.conn.exec_check("mkdir -p /etc/sscontrol-signaling")?;

        // 上传配置文件
        self.conn.upload_content(
            config_content.as_bytes(),
            "/etc/sscontrol-signaling/config.toml",
            0o600,
        )?;

        // 重新加载 systemd
        self.conn.exec_check("systemctl daemon-reload")?;

        // 启用服务开机自启
        self.conn.exec_check("systemctl enable sscontrol-signaling")?;

        info!("systemd 服务配置完成");
        Ok(())
    }

    /// 配置防火墙
    fn configure_firewall(&self) -> Result<()> {
        info!("配置防火墙...");

        let port = self.config.signaling_port;

        // 尝试 ufw
        let (code, _, _) = self.conn.exec("which ufw")?;
        if code == 0 {
            info!("检测到 UFW，配置防火墙规则...");
            let _ = self.conn.exec(&format!("ufw allow {}/tcp", port));
            return Ok(());
        }

        // 尝试 firewalld
        let (code, _, _) = self.conn.exec("which firewall-cmd")?;
        if code == 0 {
            info!("检测到 firewalld，配置防火墙规则...");
            let _ = self.conn.exec(&format!(
                "firewall-cmd --permanent --add-port={}/tcp && firewall-cmd --reload",
                port
            ));
            return Ok(());
        }

        // 尝试 iptables
        let (code, _, _) = self.conn.exec("which iptables")?;
        if code == 0 {
            info!("检测到 iptables，配置防火墙规则...");
            let _ = self.conn.exec(&format!(
                "iptables -I INPUT -p tcp --dport {} -j ACCEPT",
                port
            ));
            return Ok(());
        }

        warn!("未检测到已知防火墙，跳过防火墙配置");
        Ok(())
    }

    /// 配置 TLS (Let's Encrypt)
    fn configure_tls(&self) -> Result<()> {
        let domain = self.config.domain.as_ref().ok_or_else(|| {
            anyhow!("启用 TLS 需要提供域名 (--domain)")
        })?;

        let email = self.config.letsencrypt_email.as_ref().ok_or_else(|| {
            anyhow!("启用 TLS 需要提供 Let's Encrypt 邮箱 (--email)")
        })?;

        info!("配置 TLS (Let's Encrypt) 域名: {}...", domain);

        // 检查 certbot 是否安装
        let (code, _, _) = self.conn.exec("which certbot")?;
        if code != 0 {
            info!("安装 certbot...");
            // 尝试不同的包管理器
            let install_commands = [
                "apt-get update && apt-get install -y certbot",
                "yum install -y certbot",
                "dnf install -y certbot",
            ];

            let mut installed = false;
            for cmd in install_commands {
                let (code, _, _) = self.conn.exec(cmd)?;
                if code == 0 {
                    installed = true;
                    break;
                }
            }

            if !installed {
                return Err(anyhow!("无法安装 certbot，请手动安装后重试"));
            }
        }

        // 申请证书
        info!("申请 Let's Encrypt 证书...");
        let (code, stdout, stderr) = self.conn.exec(&format!(
            "certbot certonly --standalone --non-interactive \
             --agree-tos --email {} -d {} \
             --cert-path /etc/sscontrol-signaling/cert.pem \
             --key-path /etc/sscontrol-signaling/key.pem",
            email, domain
        ))?;

        if code != 0 {
            // 证书可能已存在，尝试使用现有证书
            let cert_path = format!("/etc/letsencrypt/live/{}/fullchain.pem", domain);
            let key_path = format!("/etc/letsencrypt/live/{}/privkey.pem", domain);

            if self.conn.file_exists(&cert_path) {
                info!("使用现有 Let's Encrypt 证书");
                // 创建符号链接
                self.conn.exec_check(&format!(
                    "ln -sf {} /etc/sscontrol-signaling/cert.pem",
                    cert_path
                ))?;
                self.conn.exec_check(&format!(
                    "ln -sf {} /etc/sscontrol-signaling/key.pem",
                    key_path
                ))?;
            } else {
                return Err(anyhow!(
                    "申请证书失败: {}\n{}",
                    stdout.trim(),
                    stderr.trim()
                ));
            }
        }

        // 配置证书续期钩子
        let hook_content = templates::certbot_renewal_hook();
        self.conn.upload_content(
            hook_content.as_bytes(),
            "/etc/letsencrypt/renewal-hooks/post/sscontrol-signaling",
            0o755,
        )?;

        info!("TLS 配置完成");
        Ok(())
    }

    /// 启动服务
    fn start_service(&self) -> Result<()> {
        info!("启动信令服务器...");

        // 停止现有服务（如果运行中）
        let _ = self.conn.exec("systemctl stop sscontrol-signaling");

        // 启动服务
        self.conn.exec_check("systemctl start sscontrol-signaling")?;

        // 等待服务启动
        std::thread::sleep(std::time::Duration::from_secs(2));

        info!("服务已启动");
        Ok(())
    }

    /// 验证部署
    fn verify_deployment(&self) -> Result<()> {
        info!("验证部署...");

        // 检查服务状态
        let (code, _, _) = self.conn.exec("systemctl is-active sscontrol-signaling")?;
        if code != 0 {
            // 获取服务日志
            let (_, logs, _) = self
                .conn
                .exec("journalctl -u sscontrol-signaling -n 20 --no-pager")?;
            return Err(anyhow!(
                "服务启动失败。最近日志:\n{}",
                logs
            ));
        }

        // 检查端口监听
        let (code, _, _) = self
            .conn
            .exec(&format!("ss -tlnp | grep -q \":{} \"", self.config.signaling_port))?;
        if code != 0 {
            warn!("端口 {} 可能未正常监听", self.config.signaling_port);
        }

        info!("部署验证通过");
        Ok(())
    }

    /// 检查服务状态
    pub fn status(&self) -> Result<String> {
        let (_, stdout, _) = self.conn.exec(
            "systemctl status sscontrol-signaling --no-pager -l 2>/dev/null || echo '服务未安装'"
        )?;
        Ok(stdout)
    }

    /// 卸载服务
    pub fn uninstall(&self) -> Result<()> {
        info!("卸载信令服务器...");

        // 停止并禁用服务
        let _ = self.conn.exec("systemctl stop sscontrol-signaling");
        let _ = self.conn.exec("systemctl disable sscontrol-signaling");

        // 删除服务文件
        let _ = self.conn.exec("rm -f /etc/systemd/system/sscontrol-signaling.service");
        let _ = self.conn.exec("systemctl daemon-reload");

        // 删除程序文件
        let _ = self.conn.exec("rm -rf /opt/sscontrol-signaling");
        let _ = self.conn.exec("rm -rf /etc/sscontrol-signaling");
        let _ = self.conn.exec("rm -rf /var/log/sscontrol-signaling");

        // 删除 certbot 钩子
        let _ = self
            .conn
            .exec("rm -f /etc/letsencrypt/renewal-hooks/post/sscontrol-signaling");

        info!("卸载完成");
        Ok(())
    }
}

/// 生成随机 API Key
fn generate_api_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_api_key() {
        let key = generate_api_key();
        assert_eq!(key.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_deployment_config_default() {
        let config = SignalingDeployment::default();
        assert_eq!(config.ssh_port, 22);
        assert_eq!(config.ssh_user, "root");
        assert_eq!(config.signaling_port, 8443);
        assert!(!config.enable_tls);
    }
}
