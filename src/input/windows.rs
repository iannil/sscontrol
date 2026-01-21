//! Windows 输入模拟实现
//!
//! 使用 Windows SendInput API

#![cfg(target_os = "windows")]

use super::{InputSimulator, MouseButton};
use anyhow::{anyhow, Result};
use std::mem;

/// Windows 鼠标输入标志
#[allow(dead_code)]
const MOUSEEVENTF_MOVE: u32 = 0x0001;
const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;
const MOUSEEVENTF_WHEEL: u32 = 0x0800;
const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;

/// Windows 输入模拟器
pub struct WindowsInputSimulator {
    screen_width: i32,
    screen_height: i32,
}

impl WindowsInputSimulator {
    /// 创建新的 Windows 输入模拟器
    pub fn new() -> Result<Self> {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
        };

        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);

            if width <= 0 || height <= 0 {
                return Err(anyhow!("无效的屏幕尺寸: {}x{}", width, height));
            }

            tracing::info!("Windows 输入模拟器初始化: {}x{}", width, height);

            Ok(Self {
                screen_width: width,
                screen_height: height,
            })
        }
    }

    /// 将归一化坐标转换为像素坐标
    fn normalize_to_pixel(&self, x: f64, y: f64) -> (i32, i32) {
        // Windows 使用 0-65535 范围的绝对坐标
        let x_pixel = (x.clamp(0.0, 1.0) * 65535.0) as i32;
        let y_pixel = (y.clamp(0.0, 1.0) * 65535.0) as i32;
        (x_pixel, y_pixel)
    }

    /// 获取鼠标按钮按下标志
    fn button_down_flag(button: MouseButton) -> u32 {
        match button {
            MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
            MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
            MouseButton::Middle => MOUSEEVENTF_MIDDLEDOWN,
        }
    }

    /// 获取鼠标按钮释放标志
    fn button_up_flag(button: MouseButton) -> u32 {
        match button {
            MouseButton::Left => MOUSEEVENTF_LEFTUP,
            MouseButton::Right => MOUSEEVENTF_RIGHTUP,
            MouseButton::Middle => MOUSEEVENTF_MIDDLEUP,
        }
    }

    /// 发送鼠标输入
    fn send_mouse_input(flags: u32, x: i32, y: i32, data: u32) -> Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::{SendInput, INPUT, INPUT_0, MOUSEINPUT};

        unsafe {
            let mouse_input = MOUSEINPUT {
                dx: x,
                dy: y,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            };

            let input = INPUT {
                r#type: windows::Win32::UI::WindowsAndMessaging::INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: mouse_input,
                },
            };

            let size = mem::size_of::<INPUT>() as i32;
            let result = SendInput(&[input], size);

            if result == 0 {
                return Err(anyhow!("SendInput 失败: {:?}", windows::core::Error::from_win32()));
            }
        }

        Ok(())
    }
}

impl InputSimulator for WindowsInputSimulator {
    fn mouse_move(&mut self, x: f64, y: f64) -> Result<()> {
        let (x_pixel, y_pixel) = self.normalize_to_pixel(x, y);

        Self::send_mouse_input(
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
            x_pixel,
            y_pixel,
            0,
        )?;

        tracing::trace!("鼠标移动: ({}, {}) -> ({}, {})", x, y, x_pixel, y_pixel);
        Ok(())
    }

    fn mouse_click(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        let flag = if pressed {
            Self::button_down_flag(button)
        } else {
            Self::button_up_flag(button)
        };

        Self::send_mouse_input(flag, 0, 0, 0)?;

        tracing::trace!(
            "鼠标点击: button={:?}, pressed={}",
            button,
            pressed
        );
        Ok(())
    }

    fn mouse_wheel(&mut self, delta_x: i32, delta_y: i32) -> Result<()> {
        if delta_y != 0 {
            // Windows 滚轮数据使用 WHEEL_DELTA (120) 为单位
            let wheel_data = (delta_y * 120) as u32;
            Self::send_mouse_input(MOUSEEVENTF_WHEEL, 0, 0, wheel_data)?;
        }

        // 水平滚轮 (需要 Windows Vista+)
        if delta_x != 0 {
            const MOUSEEVENTF_HWHEEL: u32 = 0x01000;
            let wheel_data = (delta_x * 120) as u32;
            Self::send_mouse_input(MOUSEEVENTF_HWHEEL, 0, 0, wheel_data)?;
        }

        tracing::trace!("鼠标滚轮: delta_x={}, delta_y={}", delta_x, delta_y);
        Ok(())
    }
}

/// 默认实现
impl Default for WindowsInputSimulator {
    fn default() -> Self {
        Self::new().expect("无法初始化 Windows 输入模拟器")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulator_creation() {
        let simulator = WindowsInputSimulator::new();
        // 在非 Windows 平台会失败，这是预期的
        #[cfg(target_os = "windows")]
        assert!(simulator.is_ok());
    }

    #[test]
    fn test_normalize_to_pixel() {
        let simulator = WindowsInputSimulator {
            screen_width: 1920,
            screen_height: 1080,
        };

        let (x, y) = simulator.normalize_to_pixel(0.5, 0.5);
        // 应该在 32767 (65535 / 2) 附近
        assert_eq!(x, 32767);
        assert_eq!(y, 32767);
    }

    #[test]
    fn test_clamp_coordinates() {
        let simulator = WindowsInputSimulator {
            screen_width: 1920,
            screen_height: 1080,
        };

        // 超出范围的坐标应该被限制
        let (x1, y1) = simulator.normalize_to_pixel(1.5, 1.5);
        assert_eq!(x1, 65535);
        assert_eq!(y1, 65535);

        let (x2, y2) = simulator.normalize_to_pixel(-0.5, -0.5);
        assert_eq!(x2, 0);
        assert_eq!(y2, 0);
    }

    #[test]
    fn test_button_flags() {
        assert_eq!(
            WindowsInputSimulator::button_down_flag(MouseButton::Left),
            MOUSEEVENTF_LEFTDOWN
        );
        assert_eq!(
            WindowsInputSimulator::button_down_flag(MouseButton::Right),
            MOUSEEVENTF_RIGHTDOWN
        );
        assert_eq!(
            WindowsInputSimulator::button_down_flag(MouseButton::Middle),
            MOUSEEVENTF_MIDDLEDOWN
        );

        assert_eq!(
            WindowsInputSimulator::button_up_flag(MouseButton::Left),
            MOUSEEVENTF_LEFTUP
        );
        assert_eq!(
            WindowsInputSimulator::button_up_flag(MouseButton::Right),
            MOUSEEVENTF_RIGHTUP
        );
        assert_eq!(
            WindowsInputSimulator::button_up_flag(MouseButton::Middle),
            MOUSEEVENTF_MIDDLEUP
        );
    }
}
