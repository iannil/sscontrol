//! macOS LaunchAgent 实现
//!
//! 使用 launchd 管理用户级服务
//!
//! LaunchAgent 是 macOS 的用户级服务机制，支持:
//! - 开机自启动 (用户登录时)
//! - 自动重启 (KeepAlive)
//! - 日志重定向

use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const LAUNCH_AGENT_NAME: &str = "com.sscontrol.agent";
const PLIST_RELATIVE_PATH: &str = "Library/LaunchAgents/com.sscontrol.agent.plist";

/// macOS LaunchAgent 服务控制器
pub struct MacOSLaunchAgent {
    plist_path: PathBuf,
}

impl Default for MacOSLaunchAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOSLaunchAgent {
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .unwrap_or_else(|_| ".".to_string());

        let plist_path = PathBuf::from(home).join(PLIST_RELATIVE_PATH);

        Self { plist_path }
    }

    /// 生成 plist 文件内容
    fn plist_content(&self) -> String {
        let exe_path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/sscontrol"));

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
        <key>Crashed</key>
        <true/>
    </dict>
    <key>StandardOutPath</key>
    <string>/var/log/sscontrol.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/sscontrol.error.log</string>
    <key>WorkingDirectory</key>
    <string>/tmp</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin</string>
    </dict>
</dict>
</plist>"#,
            label = LAUNCH_AGENT_NAME,
            exe = exe_path.display()
        )
    }

    /// 执行 launchctl 命令
    fn launchctl(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("launchctl")
            .args(args)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(anyhow!(
                "launchctl 失败: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// 获取当前用户 UID
    #[allow(dead_code)]
    fn get_user_uid(&self) -> Result<String> {
        let output = Command::new("id")
            .arg("-u")
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("无法获取用户 UID"))
        }
    }
}

impl super::ServiceController for MacOSLaunchAgent {
    fn install(&self) -> Result<()> {
        // 创建目录
        if let Some(parent) = self.plist_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| anyhow!("创建目录失败 {}: {}", parent.display(), e))?;
        }

        // 写入 plist 文件
        fs::write(&self.plist_path, self.plist_content())
            .map_err(|e| anyhow!("写入 plist 文件失败 {}: {}", self.plist_path.display(), e))?;

        println!("plist 文件已创建: {}", self.plist_path.display());

        // 加载服务
        let path_str = self.plist_path.to_str()
            .ok_or_else(|| anyhow!("plist 路径包含非 UTF-8 字符"))?;
        self.launchctl(&["load", path_str])?;
        println!("服务已加载");

        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        // 先尝试卸载服务
        if self.is_installed() {
            if let Some(path_str) = self.plist_path.to_str() {
                let _ = self.launchctl(&["unload", path_str]);
            }
        }

        // 删除 plist 文件
        if self.plist_path.exists() {
            fs::remove_file(&self.plist_path)
                .map_err(|e| anyhow!("删除 plist 文件失败: {}", e))?;
            println!("plist 文件已删除");
        }

        Ok(())
    }

    fn start(&self) -> Result<()> {
        if !self.is_installed() {
            return Err(anyhow!("服务未安装，请先运行 install 命令"));
        }

        self.launchctl(&["start", LAUNCH_AGENT_NAME])?;
        println!("服务已启动");
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        if !self.is_installed() {
            return Err(anyhow!("服务未安装"));
        }

        self.launchctl(&["stop", LAUNCH_AGENT_NAME])?;
        println!("服务已停止");
        Ok(())
    }

    fn status(&self) -> Result<super::ServiceStatus> {
        if !self.is_installed() {
            return Ok(super::ServiceStatus::Stopped);
        }

        // 使用 launchctl list 查看服务状态
        let output = self.launchctl(&["list"])?;

        // 查找我们的服务
        for line in output.lines() {
            if line.contains(LAUNCH_AGENT_NAME) {
                // 格式: PID  状态  名称
                // 如果 PID 不是 "-" 则说明服务正在运行
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let pid = parts[0];
                    if pid != "-" && pid.parse::<u32>().is_ok() {
                        return Ok(super::ServiceStatus::Running);
                    }
                }
            }
        }

        Ok(super::ServiceStatus::Stopped)
    }

    fn is_installed(&self) -> bool {
        self.plist_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plist_content() {
        let agent = MacOSLaunchAgent::new();
        let content = agent.plist_content();

        assert!(content.contains(LAUNCH_AGENT_NAME));
        assert!(content.contains("RunAtLoad"));
        assert!(content.contains("KeepAlive"));
    }

    #[test]
    fn test_plist_path() {
        let agent = MacOSLaunchAgent::new();
        let path_str = agent.plist_path.to_string_lossy();

        assert!(path_str.contains("LaunchAgents"));
        assert!(path_str.contains("com.sscontrol.agent.plist"));
    }
}
