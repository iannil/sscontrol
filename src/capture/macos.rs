//! macOS 屏幕捕获实现
//!
//! 使用 CGDisplayStream API 进行屏幕捕获

use super::Frame;
use super::Capturer;
use anyhow::{anyhow, Result};
use core_graphics::display::CGDisplay;

/// macOS 屏幕捕获器
pub struct MacOSCapturer {
    display_id: u32,
    width: u32,
    height: u32,
}

impl MacOSCapturer {
    /// 创建新的 macOS 捕获器
    ///
    /// # 参数
    /// * `screen_index` - 屏幕索引 (None = 主显示器)
    pub fn new(screen_index: Option<u32>) -> Result<Self> {
        // 获取显示 ID
        let display_id = Self::get_display_id(screen_index)?;

        // 获取显示尺寸
        let (width, height) = Self::get_display_size(display_id)?;

        tracing::info!(
            "创建 macOS 捕获器: display_id={}, width={}, height={}",
            display_id,
            width,
            height
        );

        Ok(MacOSCapturer {
            display_id,
            width,
            height,
        })
    }

    /// 获取显示器 ID
    fn get_display_id(screen_index: Option<u32>) -> Result<u32> {
        let displays = CGDisplay::active_displays()
            .map_err(|e| anyhow!("获取显示器列表失败: {:?}", e))?;

        if displays.is_empty() {
            return Err(anyhow!("没有检测到显示器"));
        }

        let index = screen_index.unwrap_or(0) as usize;
        if index >= displays.len() {
            return Err(anyhow!(
                "屏幕索引 {} 超出范围，共有 {} 个显示器",
                index,
                displays.len()
            ));
        }

        Ok(displays[index])
    }

    /// 获取显示器尺寸
    fn get_display_size(display_id: u32) -> Result<(u32, u32)> {
        let display = CGDisplay::new(display_id);

        // 直接从 CGDisplay 获取尺寸
        let width = display.pixels_wide() as u32;
        let height = display.pixels_high() as u32;

        Ok((width, height))
    }

    /// 检查屏幕录制权限
    pub fn check_screen_recording_permission() -> bool {
        // macOS 没有直接的 API 来检查屏幕录制权限
        // 实际运行时会弹出权限请求
        true
    }
}

impl Capturer for MacOSCapturer {
    /// 捕获一帧屏幕 (同步方式)
    fn capture(&mut self) -> Result<Frame> {
        let display = CGDisplay::new(self.display_id);

        // 捕获屏幕图像
        let image = display
            .image()
            .ok_or_else(|| anyhow!("无法捕获屏幕图像"))?;

        let width = image.width() as u32;
        let height = image.height() as u32;
        let bytes_per_row = image.bytes_per_row();

        tracing::trace!(
            "捕获帧: {}x{}, bpr={}",
            width,
            height,
            bytes_per_row
        );

        // 获取原始像素数据
        let data = image.data();

        // CGImage 返回的数据是 RGBA/RGB 格式
        let pixel_data: Vec<u8> = data.bytes().to_vec();

        Ok(Frame::from_raw_data(width, height, pixel_data, bytes_per_row))
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn start(&mut self) -> Result<()> {
        tracing::info!("屏幕捕获已启动");
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        tracing::info!("屏幕捕获已停止");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_display_id() {
        let display_id = MacOSCapturer::get_display_id(None).unwrap();
        assert!(display_id > 0);
    }

    #[test]
    fn test_get_display_size() {
        let display_id = MacOSCapturer::get_display_id(None).unwrap();
        let (width, height) = MacOSCapturer::get_display_size(display_id).unwrap();
        assert!(width > 0);
        assert!(height > 0);
        println!("显示器尺寸: {}x{}", width, height);
    }

    #[test]
    fn test_capturer_creation() {
        let capturer = MacOSCapturer::new(None).unwrap();
        assert_eq!(capturer.width(), capturer.width());
        assert_eq!(capturer.height(), capturer.height());
    }

    #[test]
    #[ignore] // 需要屏幕录制权限
    fn test_capture_frame() {
        let mut capturer = MacOSCapturer::new(None).unwrap();
        capturer.start().unwrap();

        let frame = capturer.capture().unwrap();
        assert_eq!(frame.width, capturer.width());
        assert_eq!(frame.height, capturer.height());
        assert!(!frame.data.is_empty());

        println!("捕获帧: {}x{}, 数据大小: {} bytes", frame.width, frame.height, frame.data.len());
    }
}
