//! Windows 屏幕捕获实现
//!
//! 使用 Windows.Graphics.Capture API 和 Desktop Duplication API

#![cfg(target_os = "windows")]

use super::{Capturer, Frame};
use anyhow::{anyhow, Result};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetDIBits, HDC, HBITMAP,
    ReleaseDC, SelectObject, SRCCOPY,
};
use windows::Win32::Graphics::Gdi::{BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_USAGE};
use windows::Win32::UI::WindowsAndMessaging::{
    GetDesktopWindow, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
};

/// Windows 屏幕捕获器
pub struct WindowsCapturer {
    display_id: u32,
    width: u32,
    height: u32,
    hdc: HDC,
    mem_dc: HDC,
    hbitmap: HBITMAP,
    is_started: bool,
}

impl WindowsCapturer {
    /// 创建新的 Windows 捕获器
    ///
    /// # 参数
    /// * `screen_index` - 屏幕索引 (0 = 主显示器)
    pub fn new(screen_index: Option<u32>) -> Result<Self> {
        let _index = screen_index.unwrap_or(0);

        unsafe {
            // 获取屏幕尺寸
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);

            if width <= 0 || height <= 0 {
                return Err(anyhow!("无效的屏幕尺寸: {}x{}", width, height));
            }

            tracing::info!("Windows 捕获器初始化: {}x{}", width, height);

            Ok(WindowsCapturer {
                display_id: 0,
                width: width as u32,
                height: height as u32,
                hdc: HDC::default(),
                mem_dc: HDC::default(),
                hbitmap: HBITMAP::default(),
                is_started: false,
            })
        }
    }

    /// 初始化 GDI 资源
    fn init_gdi_resources(&mut self) -> Result<()> {
        unsafe {
            // 获取桌面 DC
            let hwnd = GetDesktopWindow();
            self.hdc = GetDC(hwnd);

            if self.hdc.is_invalid() {
                return Err(anyhow!("无法获取桌面 DC"));
            }

            // 创建兼容 DC
            self.mem_dc = CreateCompatibleDC(self.hdc);

            if self.mem_dc.is_invalid() {
                ReleaseDC(hwnd, self.hdc);
                return Err(anyhow!("无法创建兼容 DC"));
            }

            // 创建位图
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: self.width as i32,
                    biHeight: self.height as i32, // 正数表示自下而上
                    biPlanes: 1,
                    biBitCount: 32, // BGRA
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [Default::default()],
            };

            self.hbitmap = CreateDIBSection(
                self.hdc,
                &bmi,
                DIB_USAGE(0),
                std::ptr::null_mut(),
                None,
                0,
            )?;

            if self.hbitmap.is_invalid() {
                ReleaseDC(hwnd, self.hdc);
                DeleteDC(self.mem_dc);
                return Err(anyhow!("无法创建位图"));
            }

            // 选入位图
            let old_bitmap = SelectObject(self.mem_dc, self.hbitmap);

            if old_bitmap.is_invalid() {
                ReleaseDC(hwnd, self.hdc);
                DeleteDC(self.mem_dc);
                DeleteObject(self.hbitmap);
                return Err(anyhow!("无法选入位图"));
            }

            self.is_started = true;
            Ok(())
        }
    }

    /// 清理 GDI 资源
    fn cleanup_gdi_resources(&mut self) {
        if !self.mem_dc.is_invalid() {
            unsafe {
                DeleteDC(self.mem_dc);
            }
        }
        if !self.hdc.is_invalid() {
            unsafe {
                let hwnd = GetDesktopWindow();
                ReleaseDC(hwnd, self.hdc);
            }
        }
        if !self.hbitmap.is_invalid() {
            unsafe {
                DeleteObject(self.hbitmap);
            }
        }
        self.is_started = false;
    }
}

impl Capturer for WindowsCapturer {
    fn capture(&mut self) -> Result<Frame> {
        if !self.is_started {
            return Err(anyhow!("捕获器未启动"));
        }

        unsafe {
            // 复制屏幕到位图
            let result = windows::Win32::Graphics::Gdi::BitBlt(
                self.mem_dc,
                0,
                0,
                self.width as i32,
                self.height as i32,
                self.hdc,
                0,
                0,
                SRCCOPY,
            );

            if result.is_ok() {
                // 获取位图数据
                let mut bitmap_info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: self.width as i32,
                        biHeight: self.height as i32,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: [Default::default()],
                };

                let data_size = (self.width * self.height * 4) as usize;
                let mut data = vec![0u8; data_size];

                let scan_lines = GetDIBits(
                    self.mem_dc,
                    self.hbitmap,
                    0,
                    self.height as u32,
                    Some(data.as_mut_ptr() as *mut _),
                    &mut bitmap_info,
                    DIB_USAGE(0),
                );

                if scan_lines == 0 {
                    return Err(anyhow!("GetDIBits 失败"));
                }

                // Windows GDI 返回的是 BGRA 格式，需要转换为 RGBA
                let mut rgba_data = Vec::with_capacity(data_size);
                for chunk in data.chunks_exact(4) {
                    // BGRA -> RGBA
                    rgba_data.push(chunk[2]); // R
                    rgba_data.push(chunk[1]); // G
                    rgba_data.push(chunk[0]); // B
                    rgba_data.push(chunk[3]); // A
                }

                Ok(Frame {
                    width: self.width,
                    height: self.height,
                    data: rgba_data,
                    timestamp: Frame::current_timestamp(),
                    stride: self.width as usize * 4,
                })
            } else {
                Err(anyhow!("BitBlt 失败"))
            }
        }
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn start(&mut self) -> Result<()> {
        if !self.is_started {
            self.init_gdi_resources()?;
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.cleanup_gdi_resources();
        Ok(())
    }
}

impl Drop for WindowsCapturer {
    fn drop(&mut self) {
        self.cleanup_gdi_resources();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_capturer_creation() {
        // 这个测试只在 Windows 平台运行
        #[cfg(target_os = "windows")]
        {
            let result = WindowsCapturer::new(Some(0));
            // 应该能创建成功（需要图形环境）
            if result.is_ok() {
                let capturer = result.unwrap();
                assert!(capturer.width > 0);
                assert!(capturer.height > 0);
            }
        }
    }
}

