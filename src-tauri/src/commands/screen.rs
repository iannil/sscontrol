use serde::{Deserialize, Serialize};

/// 屏幕信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenInfo {
    /// 屏幕索引
    pub index: u32,
    /// 屏幕宽度
    pub width: u32,
    /// 屏幕高度
    pub height: u32,
    /// 是否是主显示器
    pub is_primary: bool,
    /// 屏幕名称
    pub name: String,
    /// DPI 缩放因子
    pub scale_factor: f64,
}

/// 获取可用屏幕列表
#[tauri::command]
pub async fn get_screens() -> Result<Vec<ScreenInfo>, String> {
    let mut screens = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // 简化实现：返回默认屏幕信息
        // 实际应用中可以使用 core-graphics crate 获取真实屏幕信息
        screens.push(ScreenInfo {
            index: 0,
            width: 1920,
            height: 1080,
            is_primary: true,
            name: "主屏幕".to_string(),
            scale_factor: 2.0, // macOS Retina 通常是 2x
        });
    }

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Gdi::{
            GetDC, ReleaseDC, GetDeviceCaps,
            DESKTOPHORZRES, DESKTOPVERTRES,
            HORZRES, VERTRES,
        };

        // 简化实现：获取主屏幕
        screens.push(ScreenInfo {
            index: 0,
            width: 1920,
            height: 1080,
            is_primary: true,
            name: "主屏幕".to_string(),
            scale_factor: 1.0,
        });
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        screens.push(ScreenInfo {
            index: 0,
            width: 1920,
            height: 1080,
            is_primary: true,
            name: "主屏幕".to_string(),
            scale_factor: 1.0,
        });
    }

    Ok(screens)
}

/// 捕获屏幕预览图（缩略图）
#[tauri::command]
pub async fn capture_screen_preview(_screen_index: Option<u32>) -> Result<String, String> {
    // 创建捕获器
    let mut capturer = sscontrol::capture::create_capturer(_screen_index)
        .map_err(|e| format!("创建捕获器失败: {}", e))?;

    // 捕获一帧
    let _frame = capturer.capture()
        .map_err(|e| format!("捕获失败: {}", e))?;

    // 简化实现：返回一个占位符图片
    // 实际应用中应该使用 image crate 将 RGBA 转换为 PNG/JPEG 并编码为 base64
    let data_uri = format!("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==");

    Ok(data_uri)
}
