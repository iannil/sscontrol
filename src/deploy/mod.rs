//! 远程部署模块
//!
//! 用于通过 SSH 将信令服务器部署到远程 Linux 服务器
//!
//! # 功能
//!
//! - SSH 连接管理（支持公钥和密码认证）
//! - 信令服务器自动部署
//! - systemd 服务配置
//! - 可选的 TLS 配置（Let's Encrypt）
//!
//! # 使用示例
//!
//! ```bash
//! # 使用 SSH 密钥部署
//! sscontrol deploy signaling --host 1.2.3.4 --user root --port 8443
//!
//! # 启用 TLS
//! sscontrol deploy signaling --host 1.2.3.4 --tls --domain example.com --email admin@example.com
//! ```

mod ssh;
mod signaling_deploy;
mod templates;

pub use ssh::{SshAuth, SshConfig, SshConnection};
pub use signaling_deploy::{SignalingDeployer, SignalingDeployment, DeployResult};
pub use templates::*;
