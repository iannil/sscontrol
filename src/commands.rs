//! Command handlers for sscontrol utility commands
//!
//! This module contains handlers for service management and utility commands.

use anyhow::Result;
use std::path::PathBuf;

use crate::config;
use crate::encoder;
use crate::encoder::hardware::{HardwareEncoder, HardwareEncoderConfig, HardwareEncoderType, EncoderPreset};
use crate::encoder::Encoder;
use crate::capture::{self, Frame};
use crate::service::{self, ServiceController};
use crate::tools;

/// ServiceCommands enum (re-exported from cli for convenience)
pub use crate::cli::ServiceCommands;

/// Initialize logging with the specified verbosity level
pub fn init_logging(verbose: u8) {
    use tracing::Level;
    use tracing_subscriber;
    use std::str::FromStr;

    let log_level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let level = Level::from_str(log_level).unwrap_or(Level::INFO);

    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_max_level(level)
        .init();
}

/// Handle service management commands
pub fn handle_service_command(action: ServiceCommands) -> Result<()> {
    let controller = service::create_controller();

    match action {
        ServiceCommands::Install => {
            println!("正在安装服务...");
            controller.install()?;
            println!("服务安装成功!");
        }
        ServiceCommands::Uninstall => {
            println!("正在卸载服务...");
            controller.uninstall()?;
            println!("服务卸载成功!");
        }
        ServiceCommands::Start => {
            println!("正在启动服务...");
            controller.start()?;
        }
        ServiceCommands::Stop => {
            println!("正在停止服务...");
            controller.stop()?;
        }
        ServiceCommands::Status => {
            let status = controller.status()?;
            println!("服务状态: {}", status);
        }
    }

    Ok(())
}

/// Handle list encoders command
pub fn handle_list_encoders() -> Result<()> {
    use encoder::hardware::HardwareEncoderType;

    println!("可用编码器列表:");
    println!();

    // 检测各个编码器
    let encoders = vec![
        (HardwareEncoderType::NVENC, "NVIDIA NVENC"),
        (HardwareEncoderType::AMF, "AMD AMF"),
        (HardwareEncoderType::QuickSync, "Intel Quick Sync"),
        (HardwareEncoderType::VideoToolbox, "Apple VideoToolbox"),
        (HardwareEncoderType::Software, "Software (x264)"),
    ];

    for (encoder_type, name) in encoders {
        let available = match encoder_type {
            #[cfg(target_os = "windows")]
            HardwareEncoderType::NVENC => encoder::nvenc::NvencEncoder::is_available(),
            #[cfg(target_os = "windows")]
            HardwareEncoderType::AMF => encoder::amf::AmfEncoder::is_available(),
            #[cfg(target_os = "windows")]
            HardwareEncoderType::QuickSync => encoder::qsv::QuickSyncEncoder::is_available(),
            #[cfg(target_os = "macos")]
            HardwareEncoderType::VideoToolbox => {
                encoder::videotoolbox::VideoToolboxEncoder::is_available()
            }
            HardwareEncoderType::Software => true,
            _ => false,
        };

        let status = if available { "✓ 可用" } else { "✗ 不可用" };
        println!("  {:<25} {}", name, status);
    }

    println!();
    println!("使用 --encoder <类型> 指定编码器");
    Ok(())
}

/// Handle encoder benchmark command
pub async fn handle_benchmark(duration: u64, width: u32, height: u32) -> Result<()> {
    use std::time::{Instant, Duration};

    println!("编码器性能测试");
    println!("==================");
    println!("分辨率: {}x{}", width, height);
    println!("测试时长: {} 秒", duration);
    println!();

    // 创建测试帧
    let frame_data = vec![0u8; (width * height * 4) as usize];
    let stride = (width * 4) as usize;
    let test_frame = Frame {
        data: frame_data,
        width,
        height,
        timestamp: 0,
        stride,
    };

    // 测试配置
    let config = HardwareEncoderConfig {
        encoder_type: HardwareEncoderType::Auto,
        bitrate: 2000,
        fps: 30,
        preset: EncoderPreset::LowLatency,
    };

    // 尝试创建编码器
    let mut encoder = match encoder::hardware::HardwareEncoderWrapper::auto_select(width, height, config) {
        Ok(enc) => enc,
        Err(e) => {
            println!("❌ 无法创建编码器: {}", e);
            return Ok(());
        }
    };

    println!("编码器类型: {}", encoder.encoder_type());
    println!();
    println!("开始测试...");

    let start_time = Instant::now();
    let test_duration = Duration::from_secs(duration);
    let mut frame_count = 0u64;
    let mut total_encode_time = Duration::from_secs(0);
    let mut total_encoded_size = 0u64;

    while start_time.elapsed() < test_duration {
        let encode_start = Instant::now();
        match Encoder::encode(&mut encoder, &test_frame) {
            Ok(Some(packet)) => {
                total_encoded_size += packet.data.len() as u64;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("编码错误: {}", e);
            }
        }
        total_encode_time += encode_start.elapsed();
        frame_count += 1;

        // 控制帧率
        tokio::time::sleep(Duration::from_secs_f64(1.0 / 30.0)).await;
    }

    let elapsed = start_time.elapsed();
    let avg_latency = total_encode_time / frame_count.max(1) as u32;
    let fps = frame_count as f64 / elapsed.as_secs_f64();
    let avg_size = total_encoded_size / frame_count.max(1);
    let bandwidth = (total_encoded_size as f64 / elapsed.as_secs_f64()) / 1_000_000.0; // MB/s

    println!();
    println!("测试结果:");
    println!("=========");
    println!("总帧数: {}", frame_count);
    println!("实际 FPS: {:.2}", fps);
    println!("平均编码延迟: {:?}", avg_latency);
    println!("平均帧大小: {} bytes", avg_size);
    println!("平均带宽: {:.2} MB/s", bandwidth);

    Ok(())
}

/// Handle network diagnostics command
pub async fn handle_doctor(nat: bool, quality: bool) -> Result<()> {
    println!("sscontrol 网络诊断");
    println!("==================");
    println!();

    // 运行基础诊断
    tools::diagnostic::print_diagnostics();

    // 如果需要 NAT 检测
    if nat {
        println!();
        println!("NAT 类型检测:");
        println!("===============");
        // TODO: 实现详细的 NAT 检测
        println!("NAT 详细检测功能即将推出...");
    }

    // 如果需要网络质量测试
    if quality {
        println!();
        println!("网络质量测试:");
        println!("===============");
        // TODO: 实现网络质量测试
        println!("网络质量测试功能即将推出...");
    }

    Ok(())
}

/// Handle system info command
pub fn handle_sysinfo() -> Result<()> {
    println!("sscontrol 系统信息");
    println!("================");
    println!();

    // 操作系统信息
    println!("操作系统:");
    println!("  {}", std::env::consts::OS);
    println!("  架构: {}", std::env::consts::ARCH);
    println!();

    // 屏幕信息
    println!("屏幕信息:");
    match capture::create_capturer(Some(0)) {
        Ok(capturer) => {
            println!("  分辨率: {}x{}", capturer.width(), capturer.height());
        }
        Err(e) => {
            println!("  无法获取屏幕信息: {}", e);
        }
    }
    println!();

    // 编码器信息
    println!("编码器支持:");
    println!("  H.264 (软件): {}", if cfg!(feature = "h264") { "✓" } else { "✗" });
    println!("  WebRTC: {}", if cfg!(feature = "webrtc") { "✓" } else { "✗" });
    println!("  安全特性: {}", if cfg!(feature = "security") { "✓" } else { "✗" });
    println!("  服务模式: {}", if cfg!(feature = "service") { "✓" } else { "✗" });
    println!();

    // 硬件编码器
    println!("硬件编码器:");

    #[cfg(target_os = "windows")]
    {
        println!("  NVIDIA NVENC: {}", if encoder::nvenc::NvencEncoder::is_available() { "✓ 可用" } else { "✗ 不可用" });
        println!("  AMD AMF: {}", if encoder::amf::AmfEncoder::is_available() { "✓ 可用" } else { "✗ 不可用" });
        println!("  Intel Quick Sync: {}", if encoder::qsv::QuickSyncEncoder::is_available() { "✓ 可用" } else { "✗ 不可用" });
    }

    #[cfg(target_os = "macos")]
    {
        println!("  Apple VideoToolbox: ✓ 可用 (所有 macOS)");
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        println!("  (硬件编码器检测仅支持 Windows/macOS)");
    }
    println!();

    // 网络信息
    println!("网络信息:");
    match local_ip_address::local_ip() {
        Ok(ip) => println!("  本地 IP: {}", ip),
        Err(_) => println!("  本地 IP: 无法检测"),
    }
    println!();

    // 版本信息
    println!("版本信息:");
    println!("  sscontrol 版本: {}", env!("CARGO_PKG_VERSION"));

    Ok(())
}

/// Handle generate config command
pub fn handle_generate_config(path: Option<String>) -> Result<()> {
    use anyhow::anyhow;

    let config_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        PathBuf::from(config::Config::get_config_path(None))
    };

    println!("生成配置文件: {}", config_path.display());

    // 检查文件是否已存在
    if config_path.exists() {
        println!("⚠ 配置文件已存在");
        print!("是否覆盖? (y/N): ");
        use std::io::Write;
        std::io::stdout().flush().ok();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().to_lowercase().starts_with('y') {
            println!("已取消");
            return Ok(());
        }
    }

    // 创建默认配置
    let default_config = config::Config::default();

    // 序列化为 TOML
    let toml_string = toml::to_string_pretty(&default_config)
        .map_err(|e| anyhow!("序列化配置失败: {}", e))?;

    // 写入文件
    std::fs::write(&config_path, toml_string)
        .map_err(|e| anyhow!("写入配置文件失败: {}", e))?;

    println!("✓ 配置文件已生成: {}", config_path.display());
    println!();
    println!("配置内容:");
    println!("  设备 ID: {}", default_config.server.device_id);
    println!("  服务器 URL: {}", default_config.server.url);
    println!("  屏幕索引: {:?}", default_config.capture.screen_index);
    println!("  目标帧率: {}", default_config.capture.fps);
    println!("  日志级别: {}", default_config.logging.level);

    Ok(())
}

/// Handle stats command
pub fn handle_stats() -> Result<()> {
    println!("sscontrol 实时性能统计");
    println!("====================");
    println!();

    // 系统资源使用情况
    println!("系统资源:");

    // CPU 使用率（平台相关）
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("wmic")
            .args(&["CPU", "Get", "LoadPercentage", "/Value"])
            .output()
        {
            if let Ok(cpu_str) = String::from_utf8(output.stdout) {
                if let Ok(cpu_val) = cpu_str.trim().parse::<f32>() {
                    println!("  CPU 使用率: {:.1}%", cpu_val);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("sh")
            .arg("-c")
            .arg("top -l 1 | grep -E \"^CPU\" | awk '{print $3}'")
            .output()
        {
            if let Ok(cpu_str) = String::from_utf8(output.stdout) {
                let cpu_val: f32 = cpu_str.trim().parse().unwrap_or(0.0);
                println!("  CPU 使用率: {:.1}%", cpu_val);
            }
        }
    }

    // 内存使用
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("wmic")
            .args(&["OS", "Get", "FreePhysicalMemory", "/Value"])
            .output()
        {
            if let Ok(mem_str) = String::from_utf8(output.stdout) {
                if let Ok(mem_free) = mem_str.trim().parse::<u64>() {
                    let total_mem = sys_info_total_memory()?;
                    let used_mem = total_mem.saturating_sub(mem_free);
                    let usage_percent = (used_mem as f64 / total_mem as f64) * 100.0;
                    println!("  内存使用: {} / {} MB ({:.1}%)",
                        used_mem / (1024 * 1024),
                        total_mem / (1024 * 1024),
                        usage_percent
                    );
                }
            }
        }
    }

    println!();
    println!("编码器状态:");

    // 检测硬件编码器
    #[cfg(target_os = "windows")]
    {
        println!("  NVIDIA NVENC: {}", if encoder::nvenc::NvencEncoder::is_available() { "✓ 可用" } else { "✗ 不可用" });
        println!("  AMD AMF: {}", if encoder::amf::AmfEncoder::is_available() { "✓ 可用" } else { "✗ 不可用" });
        println!("  Intel Quick Sync: {}", if encoder::qsv::QuickSyncEncoder::is_available() { "✓ 可用" } else { "✗ 不可用" });
    }

    #[cfg(target_os = "macos")]
    {
        println!("  Apple VideoToolbox: ✓ 可用 (所有 macOS)");
    }

    println!();
    println!("提示:");
    println!("  - 运行 'sscontrol host' 启动被控端后，性能统计将实时显示");
    println!("  - 使用 'sscontrol --verbose' 查看详细日志");
    println!("  - 使用 'sscontrol benchmark' 进行编码器性能测试");

    Ok(())
}

#[cfg(target_os = "windows")]
fn sys_info_total_memory() -> anyhow::Result<u64> {
    use std::process::Command;
    let output = Command::new("wmic")
        .args(&["ComputerSystem", "Get", "TotalPhysicalMemory", "/Value"])
        .output()?;
    let mem_str = String::from_utf8(output.stdout)?;
    Ok(mem_str.trim().parse::<u64>()?)
}

#[cfg(not(target_os = "windows"))]
fn sys_info_total_memory() -> anyhow::Result<u64> {
    Ok(8 * 1024 * 1024 * 1024) // 默认 8GB
}
