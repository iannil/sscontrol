//! SSH 连接管理
//!
//! 提供 SSH 连接、命令执行和文件上传功能

use anyhow::{anyhow, Result};
use ssh2::Session;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use zeroize::Zeroize;

/// SSH 认证方式
pub enum SshAuth {
    /// 公钥认证
    PublicKey {
        key_path: PathBuf,
        passphrase: Option<SecureString>,
    },
    /// 密码认证
    Password(SecureString),
    /// SSH Agent
    Agent,
}

/// 安全字符串包装，在 Drop 时自动清除内存
pub struct SecureString(String);

impl SecureString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl From<String> for SecureString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// SSH 连接配置
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
}

/// SSH 连接管理器
pub struct SshConnection {
    session: Session,
    #[allow(dead_code)]
    config: SshConfig,
}

impl SshConnection {
    /// 建立 SSH 连接
    pub fn connect(config: SshConfig) -> Result<Self> {
        info!("连接到 {}@{}:{}...", config.username, config.host, config.port);

        // TCP 连接
        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))?;
        tcp.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
        tcp.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;

        // 创建 SSH 会话
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;

        // 认证
        match &config.auth {
            SshAuth::PublicKey { key_path, passphrase } => {
                info!("使用公钥认证: {}", key_path.display());
                session.userauth_pubkey_file(
                    &config.username,
                    None, // 公钥路径 (可选，会自动推断)
                    key_path,
                    passphrase.as_ref().map(|p| p.as_str()),
                )?;
            }
            SshAuth::Password(password) => {
                info!("使用密码认证");
                session.userauth_password(&config.username, password.as_str())?;
            }
            SshAuth::Agent => {
                info!("使用 SSH Agent 认证");
                session.userauth_agent(&config.username)?;
            }
        }

        if !session.authenticated() {
            return Err(anyhow!("SSH 认证失败"));
        }

        info!("SSH 连接成功");
        Ok(Self { session, config })
    }

    /// 自动检测并连接
    ///
    /// 按以下顺序尝试认证：
    /// 1. SSH Agent
    /// 2. 指定的私钥路径
    /// 3. 默认密钥位置
    /// 4. 密码（如果提供）
    pub fn connect_auto(
        host: &str,
        port: u16,
        username: &str,
        key_path: Option<PathBuf>,
        password: Option<String>,
    ) -> Result<Self> {
        // 尝试 SSH Agent
        let agent_config = SshConfig {
            host: host.to_string(),
            port,
            username: username.to_string(),
            auth: SshAuth::Agent,
        };
        if let Ok(conn) = Self::connect(agent_config) {
            return Ok(conn);
        }
        debug!("SSH Agent 认证失败，尝试其他方式");

        // 尝试指定的密钥
        if let Some(key) = key_path {
            if key.exists() {
                let config = SshConfig {
                    host: host.to_string(),
                    port,
                    username: username.to_string(),
                    auth: SshAuth::PublicKey {
                        key_path: key,
                        passphrase: None,
                    },
                };
                if let Ok(conn) = Self::connect(config) {
                    return Ok(conn);
                }
            }
        }

        // 尝试默认密钥位置
        if let Some(key) = Self::detect_ssh_key() {
            let config = SshConfig {
                host: host.to_string(),
                port,
                username: username.to_string(),
                auth: SshAuth::PublicKey {
                    key_path: key.clone(),
                    passphrase: None,
                },
            };
            if let Ok(conn) = Self::connect(config) {
                return Ok(conn);
            }
            debug!("默认密钥 {} 认证失败", key.display());
        }

        // 尝试密码
        if let Some(pwd) = password {
            let config = SshConfig {
                host: host.to_string(),
                port,
                username: username.to_string(),
                auth: SshAuth::Password(SecureString::new(pwd)),
            };
            return Self::connect(config);
        }

        Err(anyhow!(
            "所有认证方式均失败。请确保:\n\
             1. SSH Agent 正在运行并已添加密钥，或\n\
             2. 提供有效的 SSH 私钥路径，或\n\
             3. 提供正确的密码"
        ))
    }

    /// 执行远程命令
    pub fn exec(&self, command: &str) -> Result<(i32, String, String)> {
        debug!("执行命令: {}", command);

        let mut channel = self.session.channel_session()?;
        channel.exec(command)?;

        let mut stdout = String::new();
        let mut stderr = String::new();
        channel.read_to_string(&mut stdout)?;
        channel.stderr().read_to_string(&mut stderr)?;
        channel.wait_close()?;

        let exit_status = channel.exit_status()?;

        if exit_status != 0 {
            debug!("命令返回非零状态: {}", exit_status);
            if !stderr.is_empty() {
                debug!("stderr: {}", stderr.trim());
            }
        }

        Ok((exit_status, stdout, stderr))
    }

    /// 执行命令并检查成功
    pub fn exec_check(&self, command: &str) -> Result<String> {
        let (code, stdout, stderr) = self.exec(command)?;
        if code != 0 {
            return Err(anyhow!("命令执行失败 (退出码: {}): {}", code, stderr.trim()));
        }
        Ok(stdout)
    }

    /// 上传文件
    pub fn upload_file(&self, local_path: &Path, remote_path: &str, mode: i32) -> Result<()> {
        info!("上传文件: {} -> {}", local_path.display(), remote_path);

        let content = std::fs::read(local_path)?;
        self.upload_content(&content, remote_path, mode)
    }

    /// 上传内容
    pub fn upload_content(&self, content: &[u8], remote_path: &str, mode: i32) -> Result<()> {
        debug!("上传内容到 {} ({} 字节)", remote_path, content.len());

        let mut remote_file = self.session.scp_send(
            Path::new(remote_path),
            mode,
            content.len() as u64,
            None,
        )?;

        remote_file.write_all(content)?;
        remote_file.send_eof()?;
        remote_file.wait_eof()?;
        remote_file.close()?;
        remote_file.wait_close()?;

        Ok(())
    }

    /// 检查远程文件是否存在
    pub fn file_exists(&self, path: &str) -> bool {
        self.exec(&format!("test -f {}", path))
            .map(|(code, _, _)| code == 0)
            .unwrap_or(false)
    }

    /// 检查远程目录是否存在
    pub fn dir_exists(&self, path: &str) -> bool {
        self.exec(&format!("test -d {}", path))
            .map(|(code, _, _)| code == 0)
            .unwrap_or(false)
    }

    /// 自动检测本地 SSH 密钥
    pub fn detect_ssh_key() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let ssh_dir = home.join(".ssh");

        // 按优先级尝试常见密钥类型
        let key_names = ["id_ed25519", "id_rsa", "id_ecdsa"];

        for name in key_names {
            let key_path = ssh_dir.join(name);
            if key_path.exists() {
                debug!("检测到 SSH 密钥: {}", key_path.display());
                return Some(key_path);
            }
        }

        warn!("未检测到任何 SSH 密钥");
        None
    }

    /// 获取远程系统架构
    pub fn get_arch(&self) -> Result<String> {
        let (code, stdout, _) = self.exec("uname -m")?;
        if code != 0 {
            return Err(anyhow!("无法获取系统架构"));
        }
        Ok(stdout.trim().to_string())
    }

    /// 获取远程操作系统
    pub fn get_os(&self) -> Result<String> {
        let (code, stdout, _) = self.exec("uname -s")?;
        if code != 0 {
            return Err(anyhow!("无法获取操作系统类型"));
        }
        Ok(stdout.trim().to_string())
    }

    /// 检查是否为 root 用户
    pub fn is_root(&self) -> bool {
        self.exec("id -u")
            .map(|(code, stdout, _)| code == 0 && stdout.trim() == "0")
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_string_zeroize() {
        let mut s = SecureString::new("secret".to_string());
        assert_eq!(s.as_str(), "secret");
        drop(s);
        // 内存已清除，无法验证，但确保代码正常运行
    }

    #[test]
    fn test_detect_ssh_key() {
        // 仅测试函数不会 panic
        let _ = SshConnection::detect_ssh_key();
    }
}
