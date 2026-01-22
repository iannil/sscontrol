//! 输入模拟模块
//!
//! 在本地模拟鼠标和键盘操作

use anyhow::Result;

/// 鼠标按钮
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// 输入事件
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum InputEvent {
    /// 鼠标移动 (归一化坐标 0.0-1.0)
    MouseMove { x: f64, y: f64 },
    /// 鼠标点击
    MouseClick { button: String, pressed: bool },
    /// 鼠标滚轮
    MouseWheel { delta_x: i32, delta_y: i32 },
    /// 键盘事件
    KeyEvent { key: String, pressed: bool },
}

impl InputEvent {
    /// 创建鼠标移动事件
    pub fn mouse_move(x: f64, y: f64) -> Self {
        Self::MouseMove { x, y }
    }

    /// 创建鼠标点击事件
    pub fn mouse_click(button: MouseButton, pressed: bool) -> Self {
        let button_str = match button {
            MouseButton::Left => "left",
            MouseButton::Right => "right",
            MouseButton::Middle => "middle",
        };
        Self::MouseClick {
            button: button_str.to_string(),
            pressed,
        }
    }

    /// 创建鼠标滚轮事件
    pub fn mouse_wheel(delta_x: i32, delta_y: i32) -> Self {
        Self::MouseWheel { delta_x, delta_y }
    }
}

/// 输入模拟器 trait
pub trait InputSimulator: Send {
    /// 移动鼠标到指定位置 (归一化坐标 0.0-1.0)
    fn mouse_move(&mut self, x: f64, y: f64) -> Result<()>;

    /// 鼠标点击
    fn mouse_click(&mut self, button: MouseButton, pressed: bool) -> Result<()>;

    /// 鼠标滚轮
    fn mouse_wheel(&mut self, delta_x: i32, delta_y: i32) -> Result<()>;

    /// 键盘事件
    ///
    /// # 参数
    /// * `key` - 键名称 (如 "a", "Enter", "Shift", "Control")
    /// * `pressed` - true 表示按下，false 表示释放
    fn key_event(&mut self, key: &str, pressed: bool) -> Result<()>;

    /// 处理输入事件
    fn handle_event(&mut self, event: &InputEvent) -> Result<()> {
        match event {
            InputEvent::MouseMove { x, y } => self.mouse_move(*x, *y),
            InputEvent::MouseClick { button, pressed } => {
                let btn = match button.as_str() {
                    "left" => MouseButton::Left,
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                self.mouse_click(btn, *pressed)
            }
            InputEvent::MouseWheel { delta_x, delta_y } => {
                self.mouse_wheel(*delta_x, *delta_y)
            }
            InputEvent::KeyEvent { key, pressed } => {
                self.key_event(key, *pressed)
            }
        }
    }
}

// 平台特定实现
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacOSInputSimulator;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsInputSimulator;

// 创建平台特定的输入模拟器
pub fn create_input_simulator() -> Result<Box<dyn InputSimulator>> {
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(MacOSInputSimulator::new()?))
    }

    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(WindowsInputSimulator::new()?))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow::anyhow!("当前平台不支持输入模拟"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_creation() {
        let event = InputEvent::mouse_move(0.5, 0.5);
        assert!(matches!(event, InputEvent::MouseMove { x: 0.5, y: 0.5 }));

        let event = InputEvent::mouse_click(MouseButton::Left, true);
        assert!(matches!(
            event,
            InputEvent::MouseClick {
                button: _,
                pressed: true
            }
        ));

        let event = InputEvent::mouse_wheel(1, 2);
        assert!(matches!(event, InputEvent::MouseWheel { .. }));
    }

    #[test]
    fn test_mouse_button_conversion() {
        let event = InputEvent::mouse_click(MouseButton::Right, false);
        if let InputEvent::MouseClick { button, .. } = event {
            assert_eq!(button, "right");
        }
    }
}
