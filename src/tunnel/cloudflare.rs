//! Cloudflare Tunnel 实现
//!
//! 使用 cloudflared crate 创建 Quick Tunnel

#![allow(dead_code)]

use anyhow::{anyhow, Result};
use cloudflared::Tunnel;
use tracing::{info, warn, debug};

/// Cloudflare Tunnel 包装器
pub struct CloudflareTunnel {
    tunnel: Option<Tunnel>,
    public_url: Option<String>,
}

impl CloudflareTunnel {
    /// 创建新的 Cloudflare Tunnel 实例
    pub fn new() -> Self {
        Self {
            tunnel: None,
            public_url: None,
        }
    }

    /// 启动隧道，指向本地端口
    ///
    /// 返回公网 WebSocket URL (wss://xxx.trycloudflare.com)
    pub fn start(&mut self, local_port: u16) -> Result<String> {
        let local_url = format!("http://localhost:{}", local_port);

        info!("正在创建 Cloudflare Tunnel: {}", local_url);

        let tunnel = Tunnel::builder()
            .url(&local_url)
            .build()
            .map_err(|e| anyhow!("创建 Cloudflare Tunnel 失败: {}", e))?;

        let public_url = tunnel.url().to_string();
        info!("Cloudflare Tunnel 公网地址: {}", public_url);

        // 等待隧道稳定建立
        debug!("等待隧道连接稳定...");
        std::thread::sleep(std::time::Duration::from_secs(2));
        debug!("隧道应已稳定");

        // 将 https:// 转换为 wss:// (用于 WebSocket)
        let ws_url = public_url.replace("https://", "wss://");

        self.tunnel = Some(tunnel);
        self.public_url = Some(ws_url.clone());

        Ok(ws_url)
    }

    /// 获取公网 WebSocket URL
    pub fn url(&self) -> Option<&str> {
        self.public_url.as_deref()
    }

    /// 检查隧道是否正在运行
    pub fn is_running(&self) -> bool {
        self.tunnel.is_some()
    }
}

impl Default for CloudflareTunnel {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CloudflareTunnel {
    fn drop(&mut self) {
        if self.tunnel.is_some() {
            warn!("Cloudflare Tunnel 正在关闭...");
            // Tunnel 会在 drop 时自动关闭
            self.tunnel.take();
        }
    }
}
