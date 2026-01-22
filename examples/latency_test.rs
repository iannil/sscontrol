//! 端到端延迟测试工具
//!
//! 详细测量捕获、编码各阶段延迟，生成统计报告
//!
//! 用法:
//!   cargo run --example latency_test
//!   cargo run --example latency_test -- --frames 100 --warmup 10

use sscontrol::capture::create_capturer;
use sscontrol::encoder::{Encoder, H264Encoder};
use std::time::Instant;

/// 延迟测量结果
#[derive(Debug, Clone)]
struct LatencyMeasurement {
    capture_us: u64,
    encode_us: u64,
    total_us: u64,
}

/// 统计数据
#[derive(Debug)]
struct LatencyStats {
    min: f64,
    max: f64,
    mean: f64,
    median: f64,
    p95: f64,
    p99: f64,
    std_dev: f64,
}

impl LatencyStats {
    fn from_samples(mut samples: Vec<f64>) -> Self {
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = samples.len();

        let min = samples[0];
        let max = samples[n - 1];
        let mean = samples.iter().sum::<f64>() / n as f64;
        let median = samples[n / 2];
        let p95 = samples[(n as f64 * 0.95) as usize];
        let p99 = samples[(n as f64 * 0.99) as usize];

        let variance = samples.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / n as f64;
        let std_dev = variance.sqrt();

        LatencyStats {
            min,
            max,
            mean,
            median,
            p95,
            p99,
            std_dev,
        }
    }

    fn print(&self, name: &str) {
        println!("{}:", name);
        println!("  Min:    {:>8.2} ms", self.min);
        println!("  Max:    {:>8.2} ms", self.max);
        println!("  Mean:   {:>8.2} ms", self.mean);
        println!("  Median: {:>8.2} ms", self.median);
        println!("  P95:    {:>8.2} ms", self.p95);
        println!("  P99:    {:>8.2} ms", self.p99);
        println!("  StdDev: {:>8.2} ms", self.std_dev);
    }
}

/// 打印延迟直方图
fn print_histogram(samples: &[f64], name: &str, bucket_size: f64) {
    let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let bucket_count = ((max - min) / bucket_size).ceil() as usize + 1;
    let mut buckets = vec![0usize; bucket_count.max(1)];

    for &sample in samples {
        let len = buckets.len();
        let idx = ((sample - min) / bucket_size) as usize;
        buckets[idx.min(len - 1)] += 1;
    }

    let max_count = *buckets.iter().max().unwrap_or(&1);
    let bar_width = 40;

    println!("\n{} 延迟分布:", name);
    println!("{:>10} {:>8} {}", "范围(ms)", "计数", "分布");

    for (i, count) in buckets.iter().enumerate() {
        let start = min + (i as f64 * bucket_size);
        let end = start + bucket_size;
        let bar_len = (*count as f64 / max_count as f64 * bar_width as f64) as usize;
        let bar: String = "█".repeat(bar_len);

        if *count > 0 {
            println!("{:>4.1}-{:<4.1} {:>8} {}", start, end, count, bar);
        }
    }
}

fn main() -> anyhow::Result<()> {
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    let mut frame_count = 100;
    let mut warmup_count = 10;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--frames" | "-n" => {
                i += 1;
                if i < args.len() {
                    frame_count = args[i].parse().unwrap_or(100);
                }
            }
            "--warmup" | "-w" => {
                i += 1;
                if i < args.len() {
                    warmup_count = args[i].parse().unwrap_or(10);
                }
            }
            "--help" | "-h" => {
                println!("端到端延迟测试工具");
                println!();
                println!("用法: cargo run --example latency_test -- [选项]");
                println!();
                println!("选项:");
                println!("  --frames, -n <N>   测试帧数 (默认: 100)");
                println!("  --warmup, -w <N>   预热帧数 (默认: 10)");
                println!("  --help, -h         显示帮助信息");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║          端到端延迟测试工具 (Latency Test)             ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();

    // 创建捕获器和编码器
    println!("初始化...");
    let mut capturer = create_capturer(Some(0))?;
    let width = capturer.width();
    let height = capturer.height();

    let mut encoder = H264Encoder::new(width, height, 30, 2000)?;

    println!("  分辨率: {}x{}", width, height);
    println!("  测试帧数: {}", frame_count);
    println!("  预热帧数: {}", warmup_count);
    println!();

    capturer.start()?;

    // 预热阶段
    println!("预热阶段 ({} 帧)...", warmup_count);
    for _ in 0..warmup_count {
        let frame = capturer.capture()?;
        let _ = encoder.encode(&frame)?;
    }
    println!("预热完成\n");

    // 测量阶段
    println!("测量阶段 ({} 帧)...", frame_count);
    let mut measurements = Vec::with_capacity(frame_count);
    let test_start = Instant::now();

    for i in 0..frame_count {
        let frame_start = Instant::now();

        // 捕获
        let capture_start = Instant::now();
        let frame = capturer.capture()?;
        let capture_time = capture_start.elapsed();

        // 编码
        let encode_start = Instant::now();
        let _encoded = encoder.encode(&frame)?;
        let encode_time = encode_start.elapsed();

        let total_time = frame_start.elapsed();

        measurements.push(LatencyMeasurement {
            capture_us: capture_time.as_micros() as u64,
            encode_us: encode_time.as_micros() as u64,
            total_us: total_time.as_micros() as u64,
        });

        // 每 20 帧显示进度
        if (i + 1) % 20 == 0 || i + 1 == frame_count {
            print!("\r  进度: {}/{} ({:.0}%)",
                   i + 1, frame_count,
                   (i + 1) as f64 / frame_count as f64 * 100.0);
            std::io::Write::flush(&mut std::io::stdout())?;
        }
    }

    let test_duration = test_start.elapsed();
    println!("\n测量完成 (耗时 {:.2} 秒)\n", test_duration.as_secs_f64());

    capturer.stop()?;

    // 计算统计数据
    let capture_samples: Vec<f64> = measurements.iter()
        .map(|m| m.capture_us as f64 / 1000.0)
        .collect();
    let encode_samples: Vec<f64> = measurements.iter()
        .map(|m| m.encode_us as f64 / 1000.0)
        .collect();
    let total_samples: Vec<f64> = measurements.iter()
        .map(|m| m.total_us as f64 / 1000.0)
        .collect();

    let capture_stats = LatencyStats::from_samples(capture_samples.clone());
    let encode_stats = LatencyStats::from_samples(encode_samples.clone());
    let total_stats = LatencyStats::from_samples(total_samples.clone());

    // 打印统计报告
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║                    统计报告                            ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();

    capture_stats.print("捕获延迟");
    println!();
    encode_stats.print("编码延迟");
    println!();
    total_stats.print("总延迟");

    // 计算理论最大帧率
    let theoretical_fps = 1000.0 / total_stats.mean;
    let sustainable_fps = 1000.0 / total_stats.p99;

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║                    性能评估                            ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!();
    println!("理论最大帧率 (基于平均延迟):  {:.1} FPS", theoretical_fps);
    println!("可持续帧率 (基于 P99 延迟):   {:.1} FPS", sustainable_fps);
    println!("实际测试帧率:                 {:.1} FPS",
             frame_count as f64 / test_duration.as_secs_f64());

    // 延迟占比分析
    println!("\n延迟占比:");
    let capture_pct = capture_stats.mean / total_stats.mean * 100.0;
    let encode_pct = encode_stats.mean / total_stats.mean * 100.0;
    println!("  捕获: {:.1}%", capture_pct);
    println!("  编码: {:.1}%", encode_pct);

    // 评估结果
    println!();
    if sustainable_fps >= 30.0 {
        println!("✓ 性能良好: 可以稳定达到 30 FPS");
    } else if sustainable_fps >= 20.0 {
        println!("⚠ 性能一般: 帧率可能不稳定");
    } else {
        println!("✗ 性能不足: 需要优化");
    }

    if total_stats.p99 > 50.0 {
        println!("⚠ 尾部延迟较高 (P99 > 50ms)，可能导致卡顿");
    }

    // 打印直方图
    print_histogram(&capture_samples, "捕获", 2.0);
    print_histogram(&encode_samples, "编码", 2.0);
    print_histogram(&total_samples, "总", 5.0);

    println!();
    println!("测试完成!");

    Ok(())
}
