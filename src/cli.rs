//! CLI argument definitions for sscontrol
//!
//! This module contains all command-line argument parsing logic.

use clap::{Parser, Subcommand};

/// sscontrol - 命令行参数
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// 配置文件路径
    #[arg(short, long)]
    pub config: Option<String>,

    /// 目标帧率
    #[arg(short, long)]
    pub fps: Option<u32>,

    /// 屏幕索引
    #[arg(short = 'i', long)]
    pub screen: Option<u32>,

    /// 日志级别 (0=warn, 1=info, 2=debug, 3=trace)
    #[arg(short, long)]
    pub verbose: Option<u8>,

    /// 编码器类型 (auto/software/nvenc/amf/qsv/videotoolbox)
    #[arg(long)]
    pub encoder: Option<String>,

    /// 目标码率
    #[arg(long)]
    pub bitrate: Option<u32>,

    /// 启用自适应码率
    #[arg(long)]
    pub adaptive: bool,
}

/// 子命令
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 以服务模式运行
    Run,

    /// 服务管理
    Service {
        #[command(subcommand)]
        action: ServiceCommands,
    },

    /// 被控端模式 - 启动内嵌信令服务器等待连接
    Host {
        /// 信令服务器端口 (默认 9527)
        #[arg(short, long, default_value = "9527")]
        port: u16,

        /// 启用公网隧道 (Cloudflare Tunnel)
        #[cfg(feature = "tunnel")]
        #[arg(long)]
        tunnel: bool,
    },

    /// 控制端模式 - 通过 IP 或公网 URL 连接被控端
    Connect {
        /// 被控端 IP 地址 (局域网模式)
        #[arg(long, conflicts_with = "url")]
        ip: Option<String>,

        /// 被控端公网 URL (隧道模式，如 wss://xxx.trycloudflare.com)
        #[arg(long, conflicts_with = "ip")]
        url: Option<String>,

        /// 被控端端口 (仅 --ip 时使用，默认 9527)
        #[arg(short, long, default_value = "9527")]
        port: u16,
    },

    /// 列出可用编码器
    ListEncoders,

    /// 编码器性能测试
    Benchmark {
        /// 测试时长 (秒)
        #[arg(long, default_value = "10")]
        duration: u64,

        /// 测试分辨率宽度
        #[arg(long, default_value = "1920")]
        width: u32,

        /// 测试分辨率高度
        #[arg(long, default_value = "1080")]
        height: u32,
    },

    /// 网络诊断
    Doctor {
        /// 详细 NAT 检测
        #[arg(long)]
        nat: bool,

        /// 网络质量测试
        #[arg(long)]
        quality: bool,
    },

    /// 显示系统信息
    SysInfo,

    /// 生成配置文件
    Config {
        /// 配置文件路径
        #[arg(short, long)]
        path: Option<String>,
    },

    /// 实时性能监控
    Stats,
}

/// 服务命令
#[derive(Subcommand, Debug)]
pub enum ServiceCommands {
    /// 安装服务
    Install,
    /// 卸载服务
    Uninstall,
    /// 启动服务
    Start,
    /// 停止服务
    Stop,
    /// 查看服务状态
    Status,
}
