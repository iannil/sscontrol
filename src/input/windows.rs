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

/// Windows 键盘输入标志
const KEYEVENTF_KEYUP: u32 = 0x0002;
#[allow(dead_code)]
const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;

/// Windows 虚拟键码 (Virtual Key Code)
/// 参考: https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
mod vk {
    // 字母键
    pub const VK_A: u16 = 0x41;
    pub const VK_B: u16 = 0x42;
    pub const VK_C: u16 = 0x43;
    pub const VK_D: u16 = 0x44;
    pub const VK_E: u16 = 0x45;
    pub const VK_F: u16 = 0x46;
    pub const VK_G: u16 = 0x47;
    pub const VK_H: u16 = 0x48;
    pub const VK_I: u16 = 0x49;
    pub const VK_J: u16 = 0x4A;
    pub const VK_K: u16 = 0x4B;
    pub const VK_L: u16 = 0x4C;
    pub const VK_M: u16 = 0x4D;
    pub const VK_N: u16 = 0x4E;
    pub const VK_O: u16 = 0x4F;
    pub const VK_P: u16 = 0x50;
    pub const VK_Q: u16 = 0x51;
    pub const VK_R: u16 = 0x52;
    pub const VK_S: u16 = 0x53;
    pub const VK_T: u16 = 0x54;
    pub const VK_U: u16 = 0x55;
    pub const VK_V: u16 = 0x56;
    pub const VK_W: u16 = 0x57;
    pub const VK_X: u16 = 0x58;
    pub const VK_Y: u16 = 0x59;
    pub const VK_Z: u16 = 0x5A;
    // 数字键
    pub const VK_0: u16 = 0x30;
    pub const VK_1: u16 = 0x31;
    pub const VK_2: u16 = 0x32;
    pub const VK_3: u16 = 0x33;
    pub const VK_4: u16 = 0x34;
    pub const VK_5: u16 = 0x35;
    pub const VK_6: u16 = 0x36;
    pub const VK_7: u16 = 0x37;
    pub const VK_8: u16 = 0x38;
    pub const VK_9: u16 = 0x39;
    // 功能键
    pub const VK_F1: u16 = 0x70;
    pub const VK_F2: u16 = 0x71;
    pub const VK_F3: u16 = 0x72;
    pub const VK_F4: u16 = 0x73;
    pub const VK_F5: u16 = 0x74;
    pub const VK_F6: u16 = 0x75;
    pub const VK_F7: u16 = 0x76;
    pub const VK_F8: u16 = 0x77;
    pub const VK_F9: u16 = 0x78;
    pub const VK_F10: u16 = 0x79;
    pub const VK_F11: u16 = 0x7A;
    pub const VK_F12: u16 = 0x7B;
    // 修饰键
    pub const VK_SHIFT: u16 = 0x10;
    pub const VK_CONTROL: u16 = 0x11;
    pub const VK_MENU: u16 = 0x12; // Alt
    pub const VK_LWIN: u16 = 0x5B;
    pub const VK_RWIN: u16 = 0x5C;
    pub const VK_LSHIFT: u16 = 0xA0;
    pub const VK_RSHIFT: u16 = 0xA1;
    pub const VK_LCONTROL: u16 = 0xA2;
    pub const VK_RCONTROL: u16 = 0xA3;
    pub const VK_LMENU: u16 = 0xA4;
    pub const VK_RMENU: u16 = 0xA5;
    pub const VK_CAPITAL: u16 = 0x14; // Caps Lock
    // 特殊键
    pub const VK_RETURN: u16 = 0x0D;
    pub const VK_TAB: u16 = 0x09;
    pub const VK_SPACE: u16 = 0x20;
    pub const VK_BACK: u16 = 0x08; // Backspace
    pub const VK_DELETE: u16 = 0x2E;
    pub const VK_ESCAPE: u16 = 0x1B;
    // 方向键
    pub const VK_UP: u16 = 0x26;
    pub const VK_DOWN: u16 = 0x28;
    pub const VK_LEFT: u16 = 0x25;
    pub const VK_RIGHT: u16 = 0x27;
    // 导航键
    pub const VK_HOME: u16 = 0x24;
    pub const VK_END: u16 = 0x23;
    pub const VK_PRIOR: u16 = 0x21; // Page Up
    pub const VK_NEXT: u16 = 0x22;  // Page Down
    // 符号键 (使用 OEM 键码)
    pub const VK_OEM_MINUS: u16 = 0xBD;
    pub const VK_OEM_PLUS: u16 = 0xBB;
    pub const VK_OEM_4: u16 = 0xDB;     // [
    pub const VK_OEM_6: u16 = 0xDD;     // ]
    pub const VK_OEM_5: u16 = 0xDC;     // \
    pub const VK_OEM_1: u16 = 0xBA;     // ;
    pub const VK_OEM_7: u16 = 0xDE;     // '
    pub const VK_OEM_3: u16 = 0xC0;     // `
    pub const VK_OEM_COMMA: u16 = 0xBC;
    pub const VK_OEM_PERIOD: u16 = 0xBE;
    pub const VK_OEM_2: u16 = 0xBF;     // /
    // 媒体键
    pub const VK_VOLUME_UP: u16 = 0xAF;
    pub const VK_VOLUME_DOWN: u16 = 0xAE;
    pub const VK_VOLUME_MUTE: u16 = 0xAD;
}

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
        use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, MOUSEINPUT, INPUT_MOUSE, MOUSE_EVENT_FLAGS};

        unsafe {
            let mouse_input = MOUSEINPUT {
                dx: x,
                dy: y,
                mouseData: data,
                dwFlags: MOUSE_EVENT_FLAGS(flags),
                time: 0,
                dwExtraInfo: 0,
            };

            let input = INPUT {
                r#type: INPUT_MOUSE,
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

    /// 发送键盘输入
    fn send_keyboard_input(vk: u16, pressed: bool) -> Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, KEYBDINPUT, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, VIRTUAL_KEY,
        };

        unsafe {
            let flags = if pressed { 0 } else { KEYEVENTF_KEYUP };

            let keyboard_input = KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(flags),
                time: 0,
                dwExtraInfo: 0,
            };

            let input = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: keyboard_input,
                },
            };

            let size = mem::size_of::<INPUT>() as i32;
            let result = SendInput(&[input], size);

            if result == 0 {
                return Err(anyhow!("SendInput 键盘失败: {:?}", windows::core::Error::from_win32()));
            }
        }

        Ok(())
    }

    /// 将键名称转换为 Windows 虚拟键码
    fn key_name_to_vk(key: &str) -> Option<u16> {
        match key.to_lowercase().as_str() {
            // 字母键
            "a" => Some(vk::VK_A),
            "b" => Some(vk::VK_B),
            "c" => Some(vk::VK_C),
            "d" => Some(vk::VK_D),
            "e" => Some(vk::VK_E),
            "f" => Some(vk::VK_F),
            "g" => Some(vk::VK_G),
            "h" => Some(vk::VK_H),
            "i" => Some(vk::VK_I),
            "j" => Some(vk::VK_J),
            "k" => Some(vk::VK_K),
            "l" => Some(vk::VK_L),
            "m" => Some(vk::VK_M),
            "n" => Some(vk::VK_N),
            "o" => Some(vk::VK_O),
            "p" => Some(vk::VK_P),
            "q" => Some(vk::VK_Q),
            "r" => Some(vk::VK_R),
            "s" => Some(vk::VK_S),
            "t" => Some(vk::VK_T),
            "u" => Some(vk::VK_U),
            "v" => Some(vk::VK_V),
            "w" => Some(vk::VK_W),
            "x" => Some(vk::VK_X),
            "y" => Some(vk::VK_Y),
            "z" => Some(vk::VK_Z),
            // 数字键
            "0" | "digit0" => Some(vk::VK_0),
            "1" | "digit1" => Some(vk::VK_1),
            "2" | "digit2" => Some(vk::VK_2),
            "3" | "digit3" => Some(vk::VK_3),
            "4" | "digit4" => Some(vk::VK_4),
            "5" | "digit5" => Some(vk::VK_5),
            "6" | "digit6" => Some(vk::VK_6),
            "7" | "digit7" => Some(vk::VK_7),
            "8" | "digit8" => Some(vk::VK_8),
            "9" | "digit9" => Some(vk::VK_9),
            // 功能键
            "f1" => Some(vk::VK_F1),
            "f2" => Some(vk::VK_F2),
            "f3" => Some(vk::VK_F3),
            "f4" => Some(vk::VK_F4),
            "f5" => Some(vk::VK_F5),
            "f6" => Some(vk::VK_F6),
            "f7" => Some(vk::VK_F7),
            "f8" => Some(vk::VK_F8),
            "f9" => Some(vk::VK_F9),
            "f10" => Some(vk::VK_F10),
            "f11" => Some(vk::VK_F11),
            "f12" => Some(vk::VK_F12),
            // 修饰键
            "shift" | "shiftleft" => Some(vk::VK_LSHIFT),
            "shiftright" => Some(vk::VK_RSHIFT),
            "control" | "controlleft" | "ctrl" => Some(vk::VK_LCONTROL),
            "controlright" | "ctrlright" => Some(vk::VK_RCONTROL),
            "alt" | "altleft" => Some(vk::VK_LMENU),
            "altright" => Some(vk::VK_RMENU),
            "meta" | "metaleft" | "win" | "windows" => Some(vk::VK_LWIN),
            "metaright" | "winright" => Some(vk::VK_RWIN),
            "capslock" => Some(vk::VK_CAPITAL),
            // 特殊键
            "enter" | "return" => Some(vk::VK_RETURN),
            "tab" => Some(vk::VK_TAB),
            "space" | " " => Some(vk::VK_SPACE),
            "backspace" => Some(vk::VK_BACK),
            "delete" => Some(vk::VK_DELETE),
            "escape" | "esc" => Some(vk::VK_ESCAPE),
            // 方向键
            "arrowup" | "up" => Some(vk::VK_UP),
            "arrowdown" | "down" => Some(vk::VK_DOWN),
            "arrowleft" | "left" => Some(vk::VK_LEFT),
            "arrowright" | "right" => Some(vk::VK_RIGHT),
            // 导航键
            "home" => Some(vk::VK_HOME),
            "end" => Some(vk::VK_END),
            "pageup" => Some(vk::VK_PRIOR),
            "pagedown" => Some(vk::VK_NEXT),
            // 符号键
            "minus" | "-" => Some(vk::VK_OEM_MINUS),
            "equal" | "=" => Some(vk::VK_OEM_PLUS),
            "bracketleft" | "[" => Some(vk::VK_OEM_4),
            "bracketright" | "]" => Some(vk::VK_OEM_6),
            "backslash" | "\\" => Some(vk::VK_OEM_5),
            "semicolon" | ";" => Some(vk::VK_OEM_1),
            "quote" | "'" => Some(vk::VK_OEM_7),
            "backquote" | "`" => Some(vk::VK_OEM_3),
            "comma" | "," => Some(vk::VK_OEM_COMMA),
            "period" | "." => Some(vk::VK_OEM_PERIOD),
            "slash" | "/" => Some(vk::VK_OEM_2),
            // 媒体键
            "volumeup" => Some(vk::VK_VOLUME_UP),
            "volumedown" => Some(vk::VK_VOLUME_DOWN),
            "mute" | "volumemute" => Some(vk::VK_VOLUME_MUTE),
            _ => None,
        }
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

    fn key_event(&mut self, key: &str, pressed: bool) -> Result<()> {
        let vk = Self::key_name_to_vk(key)
            .ok_or_else(|| anyhow!("未知的键名: {}", key))?;

        Self::send_keyboard_input(vk, pressed)?;

        tracing::trace!(
            "键盘事件: key={}, vk={:#x}, pressed={}",
            key,
            vk,
            pressed
        );
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
