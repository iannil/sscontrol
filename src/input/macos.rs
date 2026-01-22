//! macOS 输入模拟实现
//!
//! 使用 Core Graphics CGEvent API

use super::{InputSimulator, MouseButton};
use anyhow::{anyhow, Result};
use core_graphics::display::CGDisplay;
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGMouseButton,
};
use core_graphics::event_source::CGEventSource;
use core_graphics::geometry::CGPoint;

/// macOS 虚拟键码 (Virtual Key Code)
/// 参考: https://eastmanreference.com/complete-list-of-applescript-key-codes
mod keycode {
    pub const KEY_A: u16 = 0x00;
    pub const KEY_S: u16 = 0x01;
    pub const KEY_D: u16 = 0x02;
    pub const KEY_F: u16 = 0x03;
    pub const KEY_H: u16 = 0x04;
    pub const KEY_G: u16 = 0x05;
    pub const KEY_Z: u16 = 0x06;
    pub const KEY_X: u16 = 0x07;
    pub const KEY_C: u16 = 0x08;
    pub const KEY_V: u16 = 0x09;
    pub const KEY_B: u16 = 0x0B;
    pub const KEY_Q: u16 = 0x0C;
    pub const KEY_W: u16 = 0x0D;
    pub const KEY_E: u16 = 0x0E;
    pub const KEY_R: u16 = 0x0F;
    pub const KEY_Y: u16 = 0x10;
    pub const KEY_T: u16 = 0x11;
    pub const KEY_1: u16 = 0x12;
    pub const KEY_2: u16 = 0x13;
    pub const KEY_3: u16 = 0x14;
    pub const KEY_4: u16 = 0x15;
    pub const KEY_6: u16 = 0x16;
    pub const KEY_5: u16 = 0x17;
    pub const KEY_EQUAL: u16 = 0x18;
    pub const KEY_9: u16 = 0x19;
    pub const KEY_7: u16 = 0x1A;
    pub const KEY_MINUS: u16 = 0x1B;
    pub const KEY_8: u16 = 0x1C;
    pub const KEY_0: u16 = 0x1D;
    pub const KEY_BRACKET_RIGHT: u16 = 0x1E;
    pub const KEY_O: u16 = 0x1F;
    pub const KEY_U: u16 = 0x20;
    pub const KEY_BRACKET_LEFT: u16 = 0x21;
    pub const KEY_I: u16 = 0x22;
    pub const KEY_P: u16 = 0x23;
    pub const KEY_ENTER: u16 = 0x24;
    pub const KEY_L: u16 = 0x25;
    pub const KEY_J: u16 = 0x26;
    pub const KEY_QUOTE: u16 = 0x27;
    pub const KEY_K: u16 = 0x28;
    pub const KEY_SEMICOLON: u16 = 0x29;
    pub const KEY_BACKSLASH: u16 = 0x2A;
    pub const KEY_COMMA: u16 = 0x2B;
    pub const KEY_SLASH: u16 = 0x2C;
    pub const KEY_N: u16 = 0x2D;
    pub const KEY_M: u16 = 0x2E;
    pub const KEY_PERIOD: u16 = 0x2F;
    pub const KEY_TAB: u16 = 0x30;
    pub const KEY_SPACE: u16 = 0x31;
    pub const KEY_BACKQUOTE: u16 = 0x32;
    pub const KEY_BACKSPACE: u16 = 0x33;
    pub const KEY_ESCAPE: u16 = 0x35;
    pub const KEY_COMMAND: u16 = 0x37;
    pub const KEY_SHIFT: u16 = 0x38;
    pub const KEY_CAPS_LOCK: u16 = 0x39;
    pub const KEY_OPTION: u16 = 0x3A;
    pub const KEY_CONTROL: u16 = 0x3B;
    pub const KEY_RIGHT_SHIFT: u16 = 0x3C;
    pub const KEY_RIGHT_OPTION: u16 = 0x3D;
    pub const KEY_RIGHT_CONTROL: u16 = 0x3E;
    pub const KEY_FUNCTION: u16 = 0x3F;
    pub const KEY_F17: u16 = 0x40;
    pub const KEY_VOLUME_UP: u16 = 0x48;
    pub const KEY_VOLUME_DOWN: u16 = 0x49;
    pub const KEY_MUTE: u16 = 0x4A;
    pub const KEY_F5: u16 = 0x60;
    pub const KEY_F6: u16 = 0x61;
    pub const KEY_F7: u16 = 0x62;
    pub const KEY_F3: u16 = 0x63;
    pub const KEY_F8: u16 = 0x64;
    pub const KEY_F9: u16 = 0x65;
    pub const KEY_F11: u16 = 0x67;
    pub const KEY_F13: u16 = 0x69;
    pub const KEY_F14: u16 = 0x6B;
    pub const KEY_F10: u16 = 0x6D;
    pub const KEY_F12: u16 = 0x6F;
    pub const KEY_F15: u16 = 0x71;
    pub const KEY_HOME: u16 = 0x73;
    pub const KEY_PAGE_UP: u16 = 0x74;
    pub const KEY_DELETE: u16 = 0x75;
    pub const KEY_F4: u16 = 0x76;
    pub const KEY_END: u16 = 0x77;
    pub const KEY_F2: u16 = 0x78;
    pub const KEY_PAGE_DOWN: u16 = 0x79;
    pub const KEY_F1: u16 = 0x7A;
    pub const KEY_ARROW_LEFT: u16 = 0x7B;
    pub const KEY_ARROW_RIGHT: u16 = 0x7C;
    pub const KEY_ARROW_DOWN: u16 = 0x7D;
    pub const KEY_ARROW_UP: u16 = 0x7E;
}

/// macOS 输入模拟器
pub struct MacOSInputSimulator {
    display_width: f64,
    display_height: f64,
    #[allow(dead_code)]
    display_id: u32,
}

impl MacOSInputSimulator {
    /// 创建新的 macOS 输入模拟器
    pub fn new() -> Result<Self> {
        // 获取主显示器尺寸
        let displays = CGDisplay::active_displays()
            .map_err(|e| anyhow!("无法获取显示器列表: {:?}", e))?;

        if displays.is_empty() {
            return Err(anyhow!("没有活动的显示器"));
        }

        let main_display = displays[0];
        let display = CGDisplay::new(main_display);

        let width = display.pixels_wide() as f64;
        let height = display.pixels_high() as f64;

        tracing::info!("macOS 输入模拟器初始化: {}x{}", width, height);

        Ok(Self {
            display_width: width,
            display_height: height,
            display_id: main_display,
        })
    }

    /// 将归一化坐标转换为像素坐标
    fn normalize_to_pixel(&self, x: f64, y: f64) -> CGPoint {
        CGPoint {
            x: (x.clamp(0.0, 1.0) * self.display_width).round(),
            y: (y.clamp(0.0, 1.0) * self.display_height).round(),
        }
    }

    /// 转换鼠标按钮类型
    fn convert_button(button: MouseButton) -> CGMouseButton {
        match button {
            MouseButton::Left => CGMouseButton::Left,
            MouseButton::Right => CGMouseButton::Right,
            MouseButton::Middle => CGMouseButton::Center,
        }
    }

    /// 获取对应的鼠标按下事件类型
    fn mouse_down_event_type(button: CGMouseButton) -> CGEventType {
        match button {
            CGMouseButton::Left => CGEventType::LeftMouseDown,
            CGMouseButton::Right => CGEventType::RightMouseDown,
            CGMouseButton::Center => CGEventType::OtherMouseDown,
        }
    }

    /// 获取对应的鼠标释放事件类型
    fn mouse_up_event_type(button: CGMouseButton) -> CGEventType {
        match button {
            CGMouseButton::Left => CGEventType::LeftMouseUp,
            CGMouseButton::Right => CGEventType::RightMouseUp,
            CGMouseButton::Center => CGEventType::OtherMouseUp,
        }
    }

    /// 获取当前鼠标位置
    fn get_mouse_location(&self) -> Result<CGPoint> {
        // 使用 CGEvent::new 获取当前事件来查询鼠标位置
        let source = CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::Private)
            .map_err(|e| anyhow!("创建 CGEventSource 失败: {:?}", e))?;
        let event = CGEvent::new(source)
            .map_err(|e| anyhow!("创建 CGEvent 失败: {:?}", e))?;
        Ok(event.location())
    }

    /// 创建事件源
    fn create_event_source(&self) -> Result<CGEventSource> {
        CGEventSource::new(core_graphics::event_source::CGEventSourceStateID::Private)
            .map_err(|e| anyhow!("创建 CGEventSource 失败: {:?}", e))
    }

    /// 将键名称转换为 macOS 虚拟键码
    fn key_name_to_keycode(key: &str) -> Option<u16> {
        match key.to_lowercase().as_str() {
            // 字母键
            "a" => Some(keycode::KEY_A),
            "b" => Some(keycode::KEY_B),
            "c" => Some(keycode::KEY_C),
            "d" => Some(keycode::KEY_D),
            "e" => Some(keycode::KEY_E),
            "f" => Some(keycode::KEY_F),
            "g" => Some(keycode::KEY_G),
            "h" => Some(keycode::KEY_H),
            "i" => Some(keycode::KEY_I),
            "j" => Some(keycode::KEY_J),
            "k" => Some(keycode::KEY_K),
            "l" => Some(keycode::KEY_L),
            "m" => Some(keycode::KEY_M),
            "n" => Some(keycode::KEY_N),
            "o" => Some(keycode::KEY_O),
            "p" => Some(keycode::KEY_P),
            "q" => Some(keycode::KEY_Q),
            "r" => Some(keycode::KEY_R),
            "s" => Some(keycode::KEY_S),
            "t" => Some(keycode::KEY_T),
            "u" => Some(keycode::KEY_U),
            "v" => Some(keycode::KEY_V),
            "w" => Some(keycode::KEY_W),
            "x" => Some(keycode::KEY_X),
            "y" => Some(keycode::KEY_Y),
            "z" => Some(keycode::KEY_Z),
            // 数字键
            "0" | "digit0" => Some(keycode::KEY_0),
            "1" | "digit1" => Some(keycode::KEY_1),
            "2" | "digit2" => Some(keycode::KEY_2),
            "3" | "digit3" => Some(keycode::KEY_3),
            "4" | "digit4" => Some(keycode::KEY_4),
            "5" | "digit5" => Some(keycode::KEY_5),
            "6" | "digit6" => Some(keycode::KEY_6),
            "7" | "digit7" => Some(keycode::KEY_7),
            "8" | "digit8" => Some(keycode::KEY_8),
            "9" | "digit9" => Some(keycode::KEY_9),
            // 功能键
            "f1" => Some(keycode::KEY_F1),
            "f2" => Some(keycode::KEY_F2),
            "f3" => Some(keycode::KEY_F3),
            "f4" => Some(keycode::KEY_F4),
            "f5" => Some(keycode::KEY_F5),
            "f6" => Some(keycode::KEY_F6),
            "f7" => Some(keycode::KEY_F7),
            "f8" => Some(keycode::KEY_F8),
            "f9" => Some(keycode::KEY_F9),
            "f10" => Some(keycode::KEY_F10),
            "f11" => Some(keycode::KEY_F11),
            "f12" => Some(keycode::KEY_F12),
            "f13" => Some(keycode::KEY_F13),
            "f14" => Some(keycode::KEY_F14),
            "f15" => Some(keycode::KEY_F15),
            "f17" => Some(keycode::KEY_F17),
            // 修饰键
            "shift" | "shiftleft" => Some(keycode::KEY_SHIFT),
            "shiftright" => Some(keycode::KEY_RIGHT_SHIFT),
            "control" | "controlleft" | "ctrl" => Some(keycode::KEY_CONTROL),
            "controlright" | "ctrlright" => Some(keycode::KEY_RIGHT_CONTROL),
            "alt" | "option" | "altleft" | "optionleft" => Some(keycode::KEY_OPTION),
            "altright" | "optionright" => Some(keycode::KEY_RIGHT_OPTION),
            "meta" | "command" | "cmd" | "metaleft" => Some(keycode::KEY_COMMAND),
            "capslock" => Some(keycode::KEY_CAPS_LOCK),
            "fn" | "function" => Some(keycode::KEY_FUNCTION),
            // 特殊键
            "enter" | "return" => Some(keycode::KEY_ENTER),
            "tab" => Some(keycode::KEY_TAB),
            "space" | " " => Some(keycode::KEY_SPACE),
            "backspace" => Some(keycode::KEY_BACKSPACE),
            "delete" => Some(keycode::KEY_DELETE),
            "escape" | "esc" => Some(keycode::KEY_ESCAPE),
            // 方向键
            "arrowup" | "up" => Some(keycode::KEY_ARROW_UP),
            "arrowdown" | "down" => Some(keycode::KEY_ARROW_DOWN),
            "arrowleft" | "left" => Some(keycode::KEY_ARROW_LEFT),
            "arrowright" | "right" => Some(keycode::KEY_ARROW_RIGHT),
            // 导航键
            "home" => Some(keycode::KEY_HOME),
            "end" => Some(keycode::KEY_END),
            "pageup" => Some(keycode::KEY_PAGE_UP),
            "pagedown" => Some(keycode::KEY_PAGE_DOWN),
            // 符号键
            "minus" | "-" => Some(keycode::KEY_MINUS),
            "equal" | "=" => Some(keycode::KEY_EQUAL),
            "bracketleft" | "[" => Some(keycode::KEY_BRACKET_LEFT),
            "bracketright" | "]" => Some(keycode::KEY_BRACKET_RIGHT),
            "backslash" | "\\" => Some(keycode::KEY_BACKSLASH),
            "semicolon" | ";" => Some(keycode::KEY_SEMICOLON),
            "quote" | "'" => Some(keycode::KEY_QUOTE),
            "backquote" | "`" => Some(keycode::KEY_BACKQUOTE),
            "comma" | "," => Some(keycode::KEY_COMMA),
            "period" | "." => Some(keycode::KEY_PERIOD),
            "slash" | "/" => Some(keycode::KEY_SLASH),
            // 媒体键
            "volumeup" => Some(keycode::KEY_VOLUME_UP),
            "volumedown" => Some(keycode::KEY_VOLUME_DOWN),
            "mute" | "volumemute" => Some(keycode::KEY_MUTE),
            _ => None,
        }
    }
}

impl InputSimulator for MacOSInputSimulator {
    fn mouse_move(&mut self, x: f64, y: f64) -> Result<()> {
        let point = self.normalize_to_pixel(x, y);
        let source = self.create_event_source()?;

        // 创建鼠标移动事件
        let mouse_event = CGEvent::new_mouse_event(
            source,
            CGEventType::MouseMoved,
            point,
            CGMouseButton::Left,
        )
        .map_err(|e| anyhow!("创建鼠标移动事件失败: {:?}", e))?;

        mouse_event.post(CGEventTapLocation::Session);

        tracing::trace!("鼠标移动: ({}, {}) -> ({}, {})", x, y, point.x, point.y);
        Ok(())
    }

    fn mouse_click(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        let cg_button = Self::convert_button(button);
        let event_type = if pressed {
            Self::mouse_down_event_type(cg_button)
        } else {
            Self::mouse_up_event_type(cg_button)
        };

        // 获取当前鼠标位置
        let location = self.get_mouse_location()?;
        let source = self.create_event_source()?;

        // 创建鼠标点击事件
        let click_event = CGEvent::new_mouse_event(
            source,
            event_type,
            location,
            cg_button,
        )
        .map_err(|e| anyhow!("创建鼠标点击事件失败: {:?}", e))?;

        click_event.post(CGEventTapLocation::Session);

        tracing::trace!(
            "鼠标点击: button={:?}, pressed={}",
            button,
            pressed
        );
        Ok(())
    }

    fn mouse_wheel(&mut self, delta_x: i32, delta_y: i32) -> Result<()> {
        if delta_x == 0 && delta_y == 0 {
            return Ok(());
        }

        // 使用 CGEventRef 直接创建滚轮事件
        // core-graphics crate 不直接暴露 CGEventCreateScrollWheelEvent
        // 所以我们使用 Core Foundation 的 raw API
        use std::ptr;

        extern "C" {
            fn CGEventCreateScrollWheelEvent(
                source: *const std::ffi::c_void,
                units: u32,
                wheelCount: u32,
                wheel1: i32,
                wheel2: i32,
            ) -> *mut std::ffi::c_void;
            fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
            fn CFRelease(cf: *mut std::ffi::c_void);
        }

        // kCGScrollEventUnitLine = 1
        const KCG_SCROLL_EVENT_UNIT_LINE: u32 = 1;
        // kCGSessionEventTap = 1
        const KCG_SESSION_EVENT_TAP: u32 = 1;

        unsafe {
            let event = CGEventCreateScrollWheelEvent(
                ptr::null(),
                KCG_SCROLL_EVENT_UNIT_LINE,
                2, // wheelCount
                delta_y,
                delta_x,
            );

            if event.is_null() {
                return Err(anyhow!("创建滚轮事件失败"));
            }

            CGEventPost(KCG_SESSION_EVENT_TAP, event);
            CFRelease(event);
        }

        tracing::trace!("鼠标滚轮: delta_x={}, delta_y={}", delta_x, delta_y);
        Ok(())
    }

    fn key_event(&mut self, key: &str, pressed: bool) -> Result<()> {
        let keycode = Self::key_name_to_keycode(key)
            .ok_or_else(|| anyhow!("未知的键名: {}", key))?;

        let source = self.create_event_source()?;

        // 创建键盘事件
        let key_event = CGEvent::new_keyboard_event(
            source,
            keycode,
            pressed,
        )
        .map_err(|e| anyhow!("创建键盘事件失败: {:?}", e))?;

        key_event.post(CGEventTapLocation::Session);

        tracing::trace!(
            "键盘事件: key={}, keycode={}, pressed={}",
            key,
            keycode,
            pressed
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulator_creation() {
        let simulator = MacOSInputSimulator::new();
        assert!(simulator.is_ok());
    }

    #[test]
    fn test_normalize_to_pixel() {
        let simulator = MacOSInputSimulator::new().unwrap();
        let point = simulator.normalize_to_pixel(0.5, 0.5);

        // 应该在屏幕中心附近
        assert!(point.x > 0.0);
        assert!(point.y > 0.0);
        assert!(point.x < simulator.display_width);
        assert!(point.y < simulator.display_height);
    }

    #[test]
    fn test_button_conversion() {
        // CGMouseButton 不实现 PartialEq，所以使用 matches! 检查
        matches!(
            MacOSInputSimulator::convert_button(MouseButton::Left),
            CGMouseButton::Left
        );
        matches!(
            MacOSInputSimulator::convert_button(MouseButton::Right),
            CGMouseButton::Right
        );
        matches!(
            MacOSInputSimulator::convert_button(MouseButton::Middle),
            CGMouseButton::Center
        );
    }

    #[test]
    fn test_clamp_coordinates() {
        let simulator = MacOSInputSimulator::new().unwrap();

        // 超出范围的坐标应该被限制
        let point1 = simulator.normalize_to_pixel(1.5, 1.5);
        assert!(point1.x <= simulator.display_width);
        assert!(point1.y <= simulator.display_height);

        let point2 = simulator.normalize_to_pixel(-0.5, -0.5);
        assert!(point2.x >= 0.0);
        assert!(point2.y >= 0.0);
    }
}
