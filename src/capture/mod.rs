//! 屏幕捕获模块
//!
//! 提供跨平台的屏幕捕获抽象

use anyhow::Result;

/// 视频帧
#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,  // RGBA 格式
    pub timestamp: u64,  // 时间戳 (毫秒)
    pub stride: usize,   // 每行字节数
}

impl Frame {
    /// 创建一个新的空帧
    pub fn new(width: u32, height: u32) -> Self {
        let stride = (width as usize) * 4;  // RGBA = 4 bytes per pixel
        let data = vec![0u8; height as usize * stride];
        Frame {
            width,
            height,
            data,
            timestamp: Self::current_timestamp(),
            stride,
        }
    }

    /// 获取当前时间戳 (毫秒)
    pub fn current_timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// 从原始数据创建帧
    pub fn from_raw_data(width: u32, height: u32, data: Vec<u8>, stride: usize) -> Self {
        Frame {
            width,
            height,
            data,
            timestamp: Self::current_timestamp(),
            stride,
        }
    }
}

/// 屏幕捕获器 trait
pub trait Capturer: Send {
    /// 捕获一帧屏幕
    fn capture(&mut self) -> Result<Frame>;

    /// 获取屏幕宽度
    fn width(&self) -> u32;

    /// 获取屏幕高度
    fn height(&self) -> u32;

    /// 开始捕获 (对于流式捕获)
    fn start(&mut self) -> Result<()>;

    /// 停止捕获
    fn stop(&mut self) -> Result<()>;
}

/// 创建平台特定的捕获器
pub fn create_capturer(screen_index: Option<u32>) -> Result<Box<dyn Capturer>> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOSCapturer::new(screen_index)?))
    }

    #[cfg(target_os = "windows")]
    {
        compile_error!("Windows 捕获器尚未实现");
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow::anyhow!("不支持的平台: 只支持 macOS 和 Windows"))
    }
}

// macOS 实现
#[cfg(target_os = "macos")]
pub mod macos;

// Windows 实现 (占位)
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_creation() {
        let frame = Frame::new(1920, 1080);
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert_eq!(frame.data.len(), 1920 * 1080 * 4);
        assert_eq!(frame.stride, 1920 * 4);
    }
}
