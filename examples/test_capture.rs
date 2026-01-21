//! 简单的屏幕捕获测试

use sscontrol::capture::create_capturer;

fn main() -> anyhow::Result<()> {
    println!("=== 屏幕捕获功能测试 ===\n");

    let mut capturer = create_capturer(Some(0))?;
    println!("✓ 捕获器创建成功");
    println!("  分辨率: {}x{}", capturer.width(), capturer.height());

    capturer.start()?;
    println!("✓ 捕获器已启动");

    // 捕获一帧
    let frame = capturer.capture()?;
    println!("✓ 捕获成功:");
    println!("  尺寸: {}x{}", frame.width, frame.height);
    println!("  数据大小: {} bytes ({:.2} MB)",
             frame.data.len(),
             frame.data.len() as f64 / 1024.0 / 1024.0);
    println!("  时间戳: {}", frame.timestamp);
    println!("  每像素字节数: {}", frame.data.len() / (frame.width as usize * frame.height as usize));

    capturer.stop()?;
    println!("✓ 捕获器已停止");

    println!("\n=== 所有测试通过! ===");
    Ok(())
}
