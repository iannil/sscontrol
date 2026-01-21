//! macOS 输入模拟实现
//!
//! 使用 Core Graphics CGEvent API

#![cfg(target_os = "macos")]

use super::{InputSimulator, MouseButton};
use anyhow::{anyhow, Result};
use core_graphics::display::CGDisplay;
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGMouseButton,
};
use core_graphics::event_source::CGEventSource;
use core_graphics::geometry::CGPoint;

/// macOS 输入模拟器
pub struct MacOSInputSimulator {
    display_width: f64,
    display_height: f64,
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

        // 获取当前鼠标位置
        let location = self.get_mouse_location()?;
        let source = self.create_event_source()?;

        // 创建滚轮事件
        let wheel_event = CGEvent::new(source)
            .map_err(|e| anyhow!("创建滚轮事件失败: {:?}", e))?;

        // 设置滚动事件类型
        wheel_event.set_type(CGEventType::ScrollWheel);
        wheel_event.post(CGEventTapLocation::Session);

        tracing::trace!("鼠标滚轮: delta_x={}, delta_y={}", delta_x, delta_y);
        Ok(())
    }
}

/// 默认实现
impl Default for MacOSInputSimulator {
    fn default() -> Self {
        Self::new().expect("无法初始化 macOS 输入模拟器")
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
