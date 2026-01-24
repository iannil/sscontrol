//! 硬件编码器抽象层
//!
//! 支持多平台硬件编码器自动选择和回退

// 硬件编码器模块尚未完全集成，标记为允许死代码
#![allow(dead_code)]

use crate::encoder::{EncodedPacket, Frame};
use anyhow::{anyhow, Result};

/// 硬件编码器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareEncoderType {
    /// NVIDIA NVENC (H.264)
    NVENC,
    /// AMD AMF (H.264)
    AMF,
    /// Intel Quick Sync (H.264)
    QuickSync,
    /// Apple VideoToolbox (H.264)
    VideoToolbox,
    /// 软件编码器 (x264)
    Software,
    /// 未知/自动检测
    Auto,
}

impl std::fmt::Display for HardwareEncoderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NVENC => write!(f, "NVIDIA NVENC"),
            Self::AMF => write!(f, "AMD AMF"),
            Self::QuickSync => write!(f, "Intel Quick Sync"),
            Self::VideoToolbox => write!(f, "Apple VideoToolbox"),
            Self::Software => write!(f, "Software (x264)"),
            Self::Auto => write!(f, "Auto"),
        }
    }
}

/// 硬件编码器配置
#[derive(Debug, Clone)]
pub struct HardwareEncoderConfig {
    /// 编码器类型
    pub encoder_type: HardwareEncoderType,
    /// 目标码率 (kbps)
    pub bitrate: u32,
    /// 目标帧率
    pub fps: u32,
    /// 编码预设 (质量/速度平衡)
    pub preset: EncoderPreset,
}

/// 编码预设
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderPreset {
    /// 超低延迟 (牺牲质量)
    UltraLowLatency,
    /// 低延迟
    LowLatency,
    /// 平衡
    Balanced,
    /// 高质量
    Quality,
}

impl Default for HardwareEncoderConfig {
    fn default() -> Self {
        Self {
            encoder_type: HardwareEncoderType::Auto,
            bitrate: 2000,
            fps: 30,
            preset: EncoderPreset::LowLatency,
        }
    }
}

/// 硬件编码器 trait
pub trait HardwareEncoder: Send {
    /// 编码一帧
    fn encode(&mut self, frame: &Frame) -> Result<Option<EncodedPacket>>;

    /// 请求关键帧
    fn request_key_frame(&mut self) -> Result<()>;

    /// 获取编码器宽度
    fn width(&self) -> u32;

    /// 获取编码器高度
    fn height(&self) -> u32;

    /// 刷新编码器缓冲区
    fn flush(&mut self) -> Result<Option<EncodedPacket>>;

    /// 获取编码器类型
    fn encoder_type(&self) -> HardwareEncoderType;

    /// 检查编码器是否可用
    fn is_available(&self) -> bool;

    /// 设置码率 (kbps)
    ///
    /// 默认实现：不支持动态码率调整
    fn set_bitrate(&mut self, _bitrate_kbps: u32) -> Result<()> {
        Ok(())
    }
}

/// 硬件编码器包装器
pub enum HardwareEncoderWrapper {
    #[cfg(target_os = "windows")]
    NVENC(super::nvenc::NvencEncoder),
    #[cfg(target_os = "windows")]
    AMF(super::amf::AmfEncoder),
    #[cfg(target_os = "windows")]
    QuickSync(super::qsv::QuickSyncEncoder),
    #[cfg(target_os = "macos")]
    VideoToolbox(super::videotoolbox::VideoToolboxEncoder),
    Software(SoftwareEncoder),
}

impl HardwareEncoderWrapper {
    /// 自动选择并创建最佳硬件编码器
    pub fn auto_select(width: u32, height: u32, config: HardwareEncoderConfig) -> Result<Self> {
        tracing::info!("自动选择硬件编码器...");

        // 优先级顺序：
        // 1. NVIDIA NVENC - 最低延迟 <5ms
        // 2. Apple VideoToolbox - 超低功耗
        // 3. AMD AMF
        // 4. Intel Quick Sync
        // 5. 软件编码

        #[cfg(target_os = "windows")]
        {
            // 尝试 NVENC
            if Self::is_nvenc_available() {
                tracing::info!("选择编码器: NVIDIA NVENC");
                return Ok(Self::NVENC(super::nvenc::NvencEncoder::new(width, height, config)?));
            }

            // 尝试 AMF
            if Self::is_amf_available() {
                tracing::info!("选择编码器: AMD AMF");
                return Ok(Self::AMF(super::amf::AmfEncoder::new(width, height, config)?));
            }

            // 尝试 Quick Sync
            if Self::is_quicksync_available() {
                tracing::info!("选择编码器: Intel Quick Sync");
                return Ok(Self::QuickSync(super::qsv::QuickSyncEncoder::new(width, height, config)?));
            }
        }

        #[cfg(target_os = "macos")]
        {
            // 尝试 VideoToolbox
            if Self::is_videotoolbox_available() {
                tracing::info!("选择编码器: Apple VideoToolbox");
                return Ok(Self::VideoToolbox(super::videotoolbox::VideoToolboxEncoder::new(width, height, config)?));
            }
        }

        // 回退到软件编码
        tracing::info!("回退到软件编码器");
        Ok(Self::Software(SoftwareEncoder::new(width, height, config)?))
    }

    /// 创建指定类型的编码器
    pub fn create(
        encoder_type: HardwareEncoderType,
        width: u32,
        height: u32,
        config: HardwareEncoderConfig,
    ) -> Result<Self> {
        match encoder_type {
            #[cfg(target_os = "windows")]
            HardwareEncoderType::NVENC => {
                Ok(Self::NVENC(super::nvenc::NvencEncoder::new(width, height, config)?))
            }
            #[cfg(target_os = "windows")]
            HardwareEncoderType::AMF => Ok(Self::AMF(super::amf::AmfEncoder::new(width, height, config)?)),
            #[cfg(target_os = "windows")]
            HardwareEncoderType::QuickSync => {
                Ok(Self::QuickSync(super::qsv::QuickSyncEncoder::new(width, height, config)?))
            }
            #[cfg(target_os = "macos")]
            HardwareEncoderType::VideoToolbox => Ok(Self::VideoToolbox(
                super::videotoolbox::VideoToolboxEncoder::new(width, height, config)?,
            )),
            HardwareEncoderType::Software => {
                Ok(Self::Software(SoftwareEncoder::new(width, height, config)?))
            }
            HardwareEncoderType::Auto => Self::auto_select(width, height, config),
            #[cfg(not(target_os = "windows"))]
            HardwareEncoderType::NVENC | HardwareEncoderType::AMF | HardwareEncoderType::QuickSync => {
                Err(anyhow!("编码器 {:?} 在当前平台不支持", encoder_type))
            }
            #[cfg(not(target_os = "macos"))]
            HardwareEncoderType::VideoToolbox => {
                Err(anyhow!("VideoToolbox 在当前平台不支持"))
            }
        }
    }

    /// 检查 NVENC 是否可用
    #[cfg(target_os = "windows")]
    fn is_nvenc_available() -> bool {
        super::nvenc::NvencEncoder::is_available()
    }

    /// 检查 AMF 是否可用
    #[cfg(target_os = "windows")]
    fn is_amf_available() -> bool {
        super::amf::AmfEncoder::is_available()
    }

    /// 检查 Quick Sync 是否可用
    #[cfg(target_os = "windows")]
    fn is_quicksync_available() -> bool {
        super::qsv::QuickSyncEncoder::is_available()
    }

    /// 检查 VideoToolbox 是否可用
    #[cfg(target_os = "macos")]
    fn is_videotoolbox_available() -> bool {
        // VideoToolbox 在所有 macOS 上都可用
        true
    }
}

// 为 HardwareEncoderWrapper 实现 HardwareEncoder trait
impl HardwareEncoder for HardwareEncoderWrapper {
    fn encode(&mut self, frame: &Frame) -> Result<Option<EncodedPacket>> {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.encode(frame),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.encode(frame),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.encode(frame),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.encode(frame),
            Self::Software(enc) => enc.encode(frame),
        }
    }

    fn request_key_frame(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.request_key_frame(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.request_key_frame(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.request_key_frame(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.request_key_frame(),
            Self::Software(enc) => enc.request_key_frame(),
        }
    }

    fn width(&self) -> u32 {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.width(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.width(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.width(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.width(),
            Self::Software(enc) => enc.width(),
        }
    }

    fn height(&self) -> u32 {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.height(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.height(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.height(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.height(),
            Self::Software(enc) => enc.height(),
        }
    }

    fn flush(&mut self) -> Result<Option<EncodedPacket>> {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.flush(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.flush(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.flush(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.flush(),
            Self::Software(enc) => enc.flush(),
        }
    }

    fn encoder_type(&self) -> HardwareEncoderType {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(_) => HardwareEncoderType::NVENC,
            #[cfg(target_os = "windows")]
            Self::AMF(_) => HardwareEncoderType::AMF,
            #[cfg(target_os = "windows")]
            Self::QuickSync(_) => HardwareEncoderType::QuickSync,
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(_) => HardwareEncoderType::VideoToolbox,
            Self::Software(_) => HardwareEncoderType::Software,
        }
    }

    fn is_available(&self) -> bool {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.is_available(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.is_available(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.is_available(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.is_available(),
            Self::Software(enc) => enc.is_available(),
        }
    }
}

// Also implement the generic Encoder trait for HardwareEncoderWrapper
impl crate::encoder::Encoder for HardwareEncoderWrapper {
    fn encode(&mut self, frame: &Frame) -> Result<Option<EncodedPacket>> {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.encode(frame),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.encode(frame),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.encode(frame),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.encode(frame),
            Self::Software(enc) => enc.encode(frame),
        }
    }

    fn request_key_frame(&mut self) -> Result<()> {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.request_key_frame(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.request_key_frame(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.request_key_frame(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.request_key_frame(),
            Self::Software(enc) => enc.request_key_frame(),
        }
    }

    fn width(&self) -> u32 {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.width(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.width(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.width(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.width(),
            Self::Software(enc) => enc.width(),
        }
    }

    fn height(&self) -> u32 {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.height(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.height(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.height(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.height(),
            Self::Software(enc) => enc.height(),
        }
    }

    fn flush(&mut self) -> Result<Option<EncodedPacket>> {
        match self {
            #[cfg(target_os = "windows")]
            Self::NVENC(enc) => enc.flush(),
            #[cfg(target_os = "windows")]
            Self::AMF(enc) => enc.flush(),
            #[cfg(target_os = "windows")]
            Self::QuickSync(enc) => enc.flush(),
            #[cfg(target_os = "macos")]
            Self::VideoToolbox(enc) => enc.flush(),
            Self::Software(enc) => enc.flush(),
        }
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        // Dynamic bitrate adjustment for hardware encoders
        // Note: Most hardware encoders don't support runtime bitrate adjustment
        // This implementation logs the request but doesn't change the bitrate
        tracing::debug!("请求调整硬件编码器码率: {} kbps (类型: {:?})", bitrate_kbps, self.encoder_type());

        // For Software encoder (which wraps H264Encoder), try to actually change bitrate
        if let Self::Software(enc) = self {
            return enc.set_bitrate(bitrate_kbps);
        }

        // For hardware encoders, we just log the request
        // Future implementation could use codec-specific APIs
        Ok(())
    }
}

/// 软件编码器 (x264)
pub struct SoftwareEncoder {
    width: u32,
    height: u32,
    config: HardwareEncoderConfig,
    #[cfg(feature = "h264")]
    inner: Option<crate::encoder::H264Encoder>,
}

impl SoftwareEncoder {
    pub fn new(width: u32, height: u32, config: HardwareEncoderConfig) -> Result<Self> {
        tracing::info!("初始化软件编码器 (x264): {}x{}", width, height);

        #[cfg(feature = "h264")]
        {
            let inner = Some(crate::encoder::H264Encoder::new(
                width,
                height,
                config.fps,
                config.bitrate,
            )?);

            Ok(Self {
                width,
                height,
                config,
                inner,
            })
        }

        #[cfg(not(feature = "h264"))]
        {
            Ok(Self {
                width,
                height,
                config,
            })
        }
    }
}

#[cfg(feature = "h264")]
impl HardwareEncoder for SoftwareEncoder {
    fn encode(&mut self, frame: &Frame) -> Result<Option<EncodedPacket>> {
        if let Some(ref mut encoder) = self.inner {
            return encoder.encode(frame);
        }
        Ok(None)
    }

    fn request_key_frame(&mut self) -> Result<()> {
        if let Some(ref mut encoder) = self.inner {
            return encoder.request_key_frame();
        }
        Ok(())
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn flush(&mut self) -> Result<Option<EncodedPacket>> {
        if let Some(ref mut encoder) = self.inner {
            return encoder.flush();
        }
        Ok(None)
    }

    fn encoder_type(&self) -> HardwareEncoderType {
        HardwareEncoderType::Software
    }

    fn is_available(&self) -> bool {
        self.inner.is_some()
    }

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        if let Some(ref mut encoder) = self.inner {
            return encoder.set_bitrate(bitrate_kbps);
        }
        Ok(())
    }
}

#[cfg(not(feature = "h264"))]
impl HardwareEncoder for SoftwareEncoder {
    fn encode(&mut self, _frame: &Frame) -> Result<Option<EncodedPacket>> {
        Ok(None)
    }

    fn request_key_frame(&mut self) -> Result<()> {
        Ok(())
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn flush(&mut self) -> Result<Option<EncodedPacket>> {
        Ok(None)
    }

    fn encoder_type(&self) -> HardwareEncoderType {
        HardwareEncoderType::Software
    }

    fn is_available(&self) -> bool {
        false
    }

    fn set_bitrate(&mut self, _bitrate_kbps: u32) -> Result<()> {
        // No-op for non-h264 build
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_encoder_config_default() {
        let config = HardwareEncoderConfig::default();
        assert_eq!(config.encoder_type, HardwareEncoderType::Auto);
        assert_eq!(config.bitrate, 2000);
        assert_eq!(config.fps, 30);
    }

    #[test]
    fn test_encoder_type_display() {
        assert_eq!(format!("{}", HardwareEncoderType::NVENC), "NVIDIA NVENC");
        assert_eq!(format!("{}", HardwareEncoderType::Software), "Software (x264)");
    }
}
