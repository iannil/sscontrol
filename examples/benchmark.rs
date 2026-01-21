//! 性能基准测试
//!
//! 测试捕获和编码的性能

use sscontrol::capture::{create_capturer, Frame};
use sscontrol::encoder::{create_encoder, Encoder};
use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    println!("=== 性能基准测试 ===\n");

    // 创建捕获器和编码器
    let mut capturer = create_capturer(Some(0))?;
    let mut encoder = create_encoder(capturer.width(), capturer.height(), 30)?;

    let width = capturer.width();
    let height = capturer.height();
    let resolution = width * height;
    let bytes_per_frame = (resolution * 4) as f64 / 1024.0 / 1024.0; // MB

    println!("测试配置:");
    println!("  分辨率: {}x{} ({} 像素)", width, height, resolution);
    println!("  每帧大小: {:.2} MB", bytes_per_frame);
    println!("  目标帧率: 30 FPS\n");

    capturer.start()?;

    // 测试 10 帧
    let frame_count = 10;
    let mut total_capture_time = Duration::ZERO;
    let mut total_encode_time = Duration::ZERO;

    println!("捕获和编码 {} 帧...", frame_count);

    for i in 0..frame_count {
        // 捕获
        let start = Instant::now();
        let frame = capturer.capture()?;
        let capture_time = start.elapsed();
        total_capture_time += capture_time;

        // 编码
        let start = Instant::now();
        let _encoded = encoder.encode(&frame)?;
        let encode_time = start.elapsed();
        total_encode_time += encode_time;

        println!("  帧 {}: 捕获 {:>8.2}ms | 编码 {:>8.2}ms",
                 i + 1,
                 capture_time.as_secs_f64() * 1000.0,
                 encode_time.as_secs_f64() * 1000.0);
    }

    capturer.stop()?;

    // 计算统计数据
    let avg_capture = total_capture_time / frame_count;
    let avg_encode = total_encode_time / frame_count;
    let avg_total = avg_capture + avg_encode;

    let max_fps = 1.0 / avg_total.as_secs_f64();
    let bandwidth_mb_s = bytes_per_frame * max_fps;

    println!("\n=== 结果统计 ===");
    println!("平均捕获时间: {:.2} ms", avg_capture.as_secs_f64() * 1000.0);
    println!("平均编码时间: {:.2} ms", avg_encode.as_secs_f64() * 1000.0);
    println!("平均总时间:   {:.2} ms", avg_total.as_secs_f64() * 1000.0);
    println!("最大帧率:     {:.1} FPS", max_fps);
    println!("带宽需求:     {:.1} MB/s (原始数据)", bandwidth_mb_s);

    // 评估结果
    println!("\n=== 评估 ===");
    if max_fps >= 30.0 {
        println!("✓ 可以达到 30 FPS 目标");
    } else if max_fps >= 15.0 {
        println!("⚠ 帧率低于目标，但可接受");
    } else {
        println!("✗ 帧率过低，需要优化");
    }

    Ok(())
}
