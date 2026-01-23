//! Windows DXGI Desktop Duplication 实现
//!
//! 使用 DXGI Desktop Duplication API 进行屏幕捕获
//! 比 GDI BitBlt 更高效，支持 Windows 8+
//!
//! 参考: https://docs.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api

#![cfg(target_os = "windows")]

use super::{Capturer, Frame};
use anyhow::{anyhow, Result};
use windows::core::ComInterface;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIAdapter, IDXGIDevice, IDXGIOutput, IDXGIOutput1, IDXGIOutputDuplication,
    DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

/// DXGI Desktop Duplication 捕获器
pub struct DXGICapturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging_texture: Option<ID3D11Texture2D>,
    width: u32,
    height: u32,
    is_started: bool,
}

impl DXGICapturer {
    /// 创建新的 DXGI 捕获器
    ///
    /// # 参数
    /// * `screen_index` - 屏幕索引 (0 = 主显示器)
    pub fn new(screen_index: Option<u32>) -> Result<Self> {
        let index = screen_index.unwrap_or(0);

        unsafe {
            // 创建 D3D11 设备
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;

            D3D11CreateDevice(
                None, // 默认适配器
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None, // 默认特性级别
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;

            let device = device.ok_or_else(|| anyhow!("无法创建 D3D11 设备"))?;
            let context = context.ok_or_else(|| anyhow!("无法创建 D3D11 设备上下文"))?;

            // 获取 DXGI 设备
            let dxgi_device: IDXGIDevice = device.cast()?;

            // 获取适配器
            let adapter: IDXGIAdapter = dxgi_device.GetAdapter()?;

            // 获取输出 (显示器)
            let output: IDXGIOutput = adapter.EnumOutputs(index)?;

            // 获取输出1接口以支持 Desktop Duplication
            let output1: IDXGIOutput1 = output.cast()?;

            // 获取输出描述以获取分辨率
            let mut desc = windows::Win32::Graphics::Dxgi::DXGI_OUTPUT_DESC::default();
            output.GetDesc(&mut desc)?;
            let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left) as u32;
            let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top) as u32;

            tracing::info!(
                "DXGI 捕获器初始化: {}x{} (显示器 {})",
                width,
                height,
                index
            );

            // 创建桌面复制
            let duplication = output1.DuplicateOutput(&device)?;

            // 设置资源优先级为最高
            dxgi_device.SetGPUThreadPriority(7)?;

            Ok(DXGICapturer {
                device,
                context,
                duplication,
                staging_texture: None,
                width,
                height,
                is_started: false,
            })
        }
    }

    /// 创建 staging 纹理用于 CPU 读取
    fn create_staging_texture(&mut self) -> Result<()> {
        if self.staging_texture.is_some() {
            return Ok(());
        }

        unsafe {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: self.width,
                Height: self.height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            let mut texture: Option<ID3D11Texture2D> = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut texture))?;

            self.staging_texture = texture;
            Ok(())
        }
    }

    /// 尝试重新获取桌面复制
    fn try_reacquire_duplication(&mut self) -> Result<()> {
        unsafe {
            // 获取 DXGI 设备
            let dxgi_device: IDXGIDevice = self.device.cast()?;
            let adapter: IDXGIAdapter = dxgi_device.GetAdapter()?;
            let output: IDXGIOutput = adapter.EnumOutputs(0)?;
            let output1: IDXGIOutput1 = output.cast()?;

            // 重新创建桌面复制
            self.duplication = output1.DuplicateOutput(&self.device)?;
            tracing::info!("DXGI 桌面复制已重新获取");
            Ok(())
        }
    }
}

impl Capturer for DXGICapturer {
    fn capture(&mut self) -> Result<Frame> {
        if !self.is_started {
            return Err(anyhow!("捕获器未启动"));
        }

        // 确保 staging 纹理已创建
        self.create_staging_texture()?;

        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut desktop_resource = None;

            // 尝试获取帧 (超时 100ms)
            let result = self.duplication.AcquireNextFrame(
                100,
                &mut frame_info,
                &mut desktop_resource,
            );

            match result {
                Ok(()) => {
                    // 获取成功
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    // 超时是正常行为 - 屏幕没有更新时会发生
                    // 使用 debug 级别而不是 error，因为这不是错误
                    tracing::debug!("DXGI 无新帧 (屏幕未更新)");
                    return Err(anyhow!("等待帧超时"));
                }
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                    // 访问丢失，需要重新获取
                    tracing::warn!("DXGI 访问丢失，尝试重新获取");
                    self.try_reacquire_duplication()?;
                    return Err(anyhow!("DXGI 访问丢失，已重新获取"));
                }
                Err(e) => {
                    tracing::error!("DXGI 获取帧失败: {:?}", e);
                    return Err(anyhow!("获取帧失败: {:?}", e));
                }
            }

            let desktop_resource = desktop_resource.ok_or_else(|| anyhow!("桌面资源为空"))?;

            // 获取纹理
            let desktop_texture: ID3D11Texture2D = desktop_resource.cast()?;

            // 复制到 staging 纹理
            let staging = self.staging_texture.as_ref()
                .ok_or_else(|| anyhow!("staging 纹理未创建"))?;
            self.context.CopyResource(staging, &desktop_texture);

            // 映射纹理以读取数据
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;

            // 计算数据大小
            let row_pitch = mapped.RowPitch as usize;
            let data_size = (self.width * self.height * 4) as usize;
            let mut rgba_data = Vec::with_capacity(data_size);

            // 复制并转换数据 (BGRA -> RGBA)
            let src = std::slice::from_raw_parts(
                mapped.pData as *const u8,
                row_pitch * self.height as usize,
            );

            for y in 0..self.height as usize {
                let row_start = y * row_pitch;
                for x in 0..self.width as usize {
                    let pixel_start = row_start + x * 4;
                    // BGRA -> RGBA
                    rgba_data.push(src[pixel_start + 2]); // R
                    rgba_data.push(src[pixel_start + 1]); // G
                    rgba_data.push(src[pixel_start]);     // B
                    rgba_data.push(src[pixel_start + 3]); // A
                }
            }

            // 解除映射
            self.context.Unmap(staging, 0);

            // 释放帧
            self.duplication.ReleaseFrame()?;

            Ok(Frame {
                width: self.width,
                height: self.height,
                data: rgba_data,
                timestamp: Frame::current_timestamp(),
                stride: self.width as usize * 4,
            })
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
            self.create_staging_texture()?;
            self.is_started = true;
            tracing::info!("DXGI 捕获器已启动");
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.is_started = false;
        self.staging_texture = None;
        tracing::info!("DXGI 捕获器已停止");
        Ok(())
    }
}

// DXGICapturer 不能自动 Send，需要手动实现
// DXGI 对象在单线程中使用是安全的
unsafe impl Send for DXGICapturer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dxgi_capturer_creation() {
        // 这个测试只在 Windows 平台运行
        #[cfg(target_os = "windows")]
        {
            let result = DXGICapturer::new(Some(0));
            // DXGI 可能在某些环境下不可用
            if result.is_ok() {
                let capturer = result.unwrap();
                assert!(capturer.width > 0);
                assert!(capturer.height > 0);
            } else {
                tracing::warn!("DXGI 捕获器创建失败 (可能是环境不支持): {:?}", result.err());
            }
        }
    }
}
