//! Linux systemd 服务实现
//!
//! 使用 systemd 管理系统级服务
//!
//! systemd 是现代 Linux 发行版的标准初始化系统，支持:
//! - 开机自启动
//! - 依赖管理
//! - 日志集成 (journald)
//! - 自动重启

use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SERVICE_NAME: &str = "sscontrol";
const SERVICE_FILE_PATH: &str = "/etc/systemd/system/sscontrol.service";

/// Linux systemd 服务控制器
pub struct SystemdService {
    service_file: PathBuf,
}

impl SystemdService {
    pub fn new() -> Self {
        Self {
            service_file: PathBuf::from(SERVICE_FILE_PATH),
        }
    }

    /// 生成 systemd 服务文件内容
    fn service_content(&self) -> String {
        let exe_path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/sscontrol"));

        format!(
            r#"[Unit]
Description=SSControl Remote Desktop Service
Documentation=https://github.com/sscontrol/sscontrol
After=network.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={exe} run
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=sscontrol

# 安全设置
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log

[Install]
WantedBy=multi-user.target
"#,
            exe = exe_path.display()
        )
    }

    /// 执行 systemctl 命令
    fn systemctl(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("systemctl")
            .args(args)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!(
                "systemctl 失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// 检查是否为 root 用户
    fn is_root(&self) -> bool {
        match Command::new("id").arg("-u").output() {
            Ok(output) => {
                let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
                uid == "0"
            }
            Err(_) => false,
        }
    }
}

impl super::ServiceController for SystemdService {
    fn install(&self) -> Result<()> {
        if !self.is_root() {
            return Err(anyhow!("需要 root 权限来安装服务，请使用 sudo"));
        }

        // 写入服务文件
        fs::write(&self.service_file, self.service_content())
            .map_err(|e| anyhow!("写入服务文件失败: {}", e))?;

        println!("服务文件已创建: {}", self.service_file.display());

        // 重新加载 systemd
        self.systemctl(&["daemon-reload"])?;
        println!("systemd 配置已重新加载");

        // 启用服务（开机自启动）
        self.systemctl(&["enable", SERVICE_NAME])?;
        println!("服务已设置为开机自启动");

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        if !self.is_root() {
            return Err(anyhow!("需要 root 权限来卸载服务，请使用 sudo"));
        }

        // 停止并禁用服务
        let _ = self.systemctl(&["stop", SERVICE_NAME]);
        let _ = self.systemctl(&["disable", SERVICE_NAME]);

        // 删除服务文件
        if self.service_file.exists() {
            fs::remove_file(&self.service_file)
                .map_err(|e| anyhow!("删除服务文件失败: {}", e))?;
            println!("服务文件已删除");
        }

        // 重新加载 systemd
        self.systemctl(&["daemon-reload"])?;
        println!("systemd 配置已重新加载");

        Ok(())
    }

    fn start(&self) -> Result<()> {
        if !self.is_root() {
            return Err(anyhow!("需要 root 权限来启动服务，请使用 sudo"));
        }

        self.systemctl(&["start", SERVICE_NAME])?;
        println!("服务已启动");
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        if !self.is_root() {
            return Err(anyhow!("需要 root 权限来停止服务，请使用 sudo"));
        }

        self.systemctl(&["stop", SERVICE_NAME])?;
        println!("服务已停止");
        Ok(())
    }

    fn status(&self) -> Result<super::ServiceStatus> {
        let output = self.systemctl(&["status", SERVICE_NAME]);

        match output {
            Ok(output) => {
                if output.contains("active (running)") {
                    Ok(super::ServiceStatus::Running)
                } else if output.contains("inactive") {
                    Ok(super::ServiceStatus::Stopped)
                } else if output.contains("failed") {
                    Ok(super::ServiceStatus::Failed("服务启动失败".to_string()))
                } else {
                    Ok(super::ServiceStatus::Unknown)
                }
            }
            Err(e) => {
                // systemctl status 返回非零退出码可能只是表示服务未运行
                if e.to_string().contains("inactive") || e.to_string().contains("dead") {
                    Ok(super::ServiceStatus::Stopped)
                } else {
                    Ok(super::ServiceStatus::Unknown)
                }
            }
        }
    }

    fn is_installed(&self) -> bool {
        self.service_file.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_content() {
        let service = SystemdService::new();
        let content = service.service_content();

        assert!(content.contains("[Unit]"));
        assert!(content.contains("[Service]"));
        assert!(content.contains("[Install]"));
        assert!(content.contains("sscontrol"));
    }

    #[test]
    fn test_service_file_path() {
        let service = SystemdService::new();
        assert_eq!(service.service_file, PathBuf::from(SERVICE_FILE_PATH));
    }
}
