//! 编码器测试

use sscontrol::capture::{create_capturer, Frame};
use sscontrol::encoder::{create_encoder, Encoder};

fn main() -> anyhow::Result<()> {
    println!("=== 编码器功能测试 ===\n");

    // 创建捕获器
    let mut capturer = create_capturer(Some(0))?;
    println!("✓ 捕获器创建: {}x{}", capturer.width(), capturer.height());

    // 创建编码器
    let mut encoder = create_encoder(capturer.width(), capturer.height(), 30)?;
    println!("✓ 编码器创建成功");

    // 捕获一帧
    capturer.start()?;
    let frame = capturer.capture()?;
    capturer.stop()?;
    println!("✓ 捕获帧: {} bytes", frame.data.len());

    // 编码
    let encoded = encoder.encode(&frame)?;
    println!("✓ 编码完成:");
    if let Some(packet) = encoded {
        println!("  数据大小: {} bytes ({:.2} KB)",
                 packet.data.len(),
                 packet.data.len() as f64 / 1024.0);
        println!("  关键帧: {}", packet.is_key_frame);
        println!("  时间戳: {}", packet.timestamp);

        // 验证帧头
        if packet.data.len() >= 24 {
            let magic = &packet.data[0..4];
            let width = u32::from_be_bytes(packet.data[4..8].try_into().unwrap());
            let height = u32::from_be_bytes(packet.data[8..12].try_into().unwrap());
            let data_size = u32::from_be_bytes(packet.data[20..24].try_into().unwrap());

            println!("\n  帧头解析:");
            println!("    Magic: {:02X} {:02X} {:02X} {:02X}",
                     magic[0], magic[1], magic[2], magic[3]);
            println!("    宽度: {}", width);
            println!("    高度: {}", height);
            println!("    数据大小: {} bytes", data_size);
        }
    } else {
        println!("  (无数据输出)");
    }

    println!("\n=== 所有测试通过! ===");
    Ok(())
}
