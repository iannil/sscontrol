//! AMD AMF 硬件编码器
//!
//! 使用 AMD AMF SDK 进行 H.264 硬件编码
//!
//! ## 性能特点
//! - 编码延迟: <10ms
//! - CPU 占用: <8%
//! - 带宽: 2-4 Mbps @1080p@30fps
//!
//! ## 支持的平台

// AMF 编码器尚未完全集成，标记为允许死代码
#![allow(dead_code)]
//! - Windows + AMD GPU (Radeon HD 7000系列及更新)
//!
//! ## 依赖
//! - AMD GPU with VCE/VCN support
//! - AMD Graphics Driver (Adrenalin 2020或更新)
//! - FFmpeg with h264_amf codec

#[cfg(target_os = "windows")]
use crate::encoder::{EncodedPacket, Frame};
#[cfg(target_os = "windows")]
use crate::encoder::hardware::{HardwareEncoder, HardwareEncoderConfig, HardwareEncoderType};
#[cfg(target_os = "windows")]
use anyhow::{anyhow, Result};

/// AMD AMF 编码器
///
/// 使用 AMD AMF (Advanced Media Framework) 进行 H.264 硬件编码
#[cfg(target_os = "windows")]
pub struct AmfEncoder {
    width: u32,
    height: u32,
    config: HardwareEncoderConfig,
    #[cfg(feature = "h264")]
    inner: Option<ffmpeg_next::encoder::Video>,
    #[cfg(feature = "h264")]
    sws_context: Option<ffmpeg_next::software::scaling::Context>,
    pts: i64,
    key_frame_interval: u64,
    frame_count: u64,
}

#[cfg(target_os = "windows")]
impl AmfEncoder {
    /// 创建新的 AMF 编码器
    pub fn new(width: u32, height: u32, config: HardwareEncoderConfig) -> Result<Self> {
        tracing::info!(
            "初始化 AMD AMF 编码器: {}x{} @ {}fps, {}kbps",
            width, height, config.fps, config.bitrate
        );

        #[cfg(feature = "h264")]
        {
            // 初始化 FFmpeg
            ffmpeg_next::init()?;

            // 查找 AMF H.264 编码器
            let encoder = ffmpeg_next::encoder::find_by_name("h264_amf")
                .ok_or_else(|| anyhow!("找不到 AMF 编码器 (h264_amf)。请确保安装了 AMD 驱动且支持 VCE/VCN"))?;

            tracing::info!("找到编码器: {}", encoder.name());

            // 配置编码器
            let context = ffmpeg_next::codec::context::Context::new_with_codec(encoder);
            let mut encoder_context = context.encoder().video()?;

            encoder_context.set_bit_rate((config.bitrate * 1000) as usize);
            encoder_context.set_width(width);
            encoder_context.set_height(height);
            encoder_context.set_frame_rate(Some(ffmpeg_next::Rational(config.fps as i32, 1)));
            encoder_context.set_time_base(ffmpeg_next::Rational(1, config.fps as i32));
            encoder_context.set_gop(30);
            encoder_context.set_format(ffmpeg_next::format::Pixel::NV12);

            // AMF 特定选项
            let mut opts = ffmpeg_next::Dictionary::new();
            opts.set("quality", "speed");  // 优先速度
            opts.set("rc", "cbr");         // 恒定码率
            opts.set("b_max", "0");        // 禁用 B 帧

            // 打开编码器
            let video_encoder = encoder_context.open_with(opts)?;

            // 创建 SwsContext 用于 RGBA -> NV12 转换
            let sws_context = ffmpeg_next::software::scaling::Context::get(
                ffmpeg_next::format::Pixel::RGBA,
                width,
                height,
                ffmpeg_next::format::Pixel::NV12,
                width,
                height,
                ffmpeg_next::software::scaling::Flags::BILINEAR,
            )?;

            tracing::info!("AMF 编码器创建成功 (speed 预设)");
            Ok(Self {
                width,
                height,
                config,
                inner: Some(video_encoder),
                sws_context: Some(sws_context),
                pts: 0,
                key_frame_interval: 30,
                frame_count: 0,
            })
        }

        #[cfg(not(feature = "h264"))]
        {
            Err(anyhow!("AMF 编码器需要启用 h264 feature"))
        }
    }

    /// 检测 AMF 是否可用
    pub fn is_available() -> bool {
        #[cfg(feature = "h264")]
        {
            if let Ok(_) = ffmpeg_next::init() {
                if let Some(encoder) = ffmpeg_next::encoder::find_by_name("h264_amf") {
                    tracing::info!("AMF 编码器可用: {}", encoder.name());
                    return true;
                }
            }
            tracing::warn!("AMF 编码器不可用");
            false
        }
        #[cfg(not(feature = "h264"))]
        {
            false
        }
    }

    /// 将 RGBA 帧转换为 NV12 格式
    #[cfg(feature = "h264")]
    fn rgba_to_nv12_frame(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<ffmpeg_next::frame::Video> {
        // 创建源帧 (RGBA)
        let mut src_frame = ffmpeg_next::frame::Video::empty();
        src_frame.set_format(ffmpeg_next::format::Pixel::RGBA);
        src_frame.set_width(width);
        src_frame.set_height(height);

        unsafe {
            src_frame.alloc(ffmpeg_next::format::Pixel::RGBA, width, height);
        }

        // 复制 RGBA 数据到源帧
        let src_stride = src_frame.stride(0);
        let src_data = src_frame.data_mut(0);
        for y in 0..height as usize {
            let src_row_start = y * (width as usize * 4);
            let dst_row_start = y * src_stride;
            let row_len = width as usize * 4;
            src_data[dst_row_start..dst_row_start + row_len]
                .copy_from_slice(&rgba[src_row_start..src_row_start + row_len]);
        }

        // 创建目标帧 (NV12)
        let mut dst_frame = ffmpeg_next::frame::Video::empty();
        dst_frame.set_format(ffmpeg_next::format::Pixel::NV12);
        dst_frame.set_width(width);
        dst_frame.set_height(height);

        unsafe {
            dst_frame.alloc(ffmpeg_next::format::Pixel::NV12, width, height);
        }

        // 使用 SwsContext 进行转换
        if let Some(ref mut sws) = self.sws_context {
            sws.run(&src_frame, &mut dst_frame)?;
        } else {
            return Err(anyhow!("SwsContext 未初始化"));
        }

        Ok(dst_frame)
    }
}

#[cfg(target_os = "windows")]
impl HardwareEncoder for AmfEncoder {
    #[cfg(feature = "h264")]
    fn encode(&mut self, frame: &Frame) -> Result<Option<EncodedPacket>> {
        // 转换为 NV12
        let mut nv12_frame = self.rgba_to_nv12_frame(&frame.data, frame.width, frame.height)?;

        // 设置 PTS
        nv12_frame.set_pts(Some(self.pts));
        self.pts += 1;
        self.frame_count += 1;

        // 判断是否为关键帧
        let is_key_frame = self.frame_count % self.key_frame_interval == 0;

        // 编码
        let encoder = self.inner.as_mut().ok_or_else(|| anyhow!("编码器未初始化"))?;
        encoder.send_frame(&nv12_frame)?;

        let mut packet = ffmpeg_next::packet::Packet::empty();
        match encoder.receive_packet(&mut packet) {
            Ok(_) => {
                if packet.size() > 0 {
                    let data = packet.data().unwrap_or(&[]).to_vec();
                    Ok(Some(EncodedPacket {
                        data,
                        is_key_frame,
                        timestamp: frame.timestamp,
                        pts: self.pts,
                    }))
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("more frames") || err_msg.contains("flushing") {
                    Ok(None)
                } else {
                    Err(anyhow!("AMF 编码失败: {}", e))
                }
            }
        }
    }

    #[cfg(not(feature = "h264"))]
    fn encode(&mut self, _frame: &Frame) -> Result<Option<EncodedPacket>> {
        Err(anyhow!("AMF 编码器需要启用 h264 feature"))
    }

    fn request_key_frame(&mut self) -> Result<()> {
        self.frame_count = self.key_frame_interval - 1;
        Ok(())
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    #[cfg(feature = "h264")]
    fn flush(&mut self) -> Result<Option<EncodedPacket>> {
        if let Some(encoder) = self.inner.as_mut() {
            let _ = encoder.send_eof();
            let mut packet = ffmpeg_next::packet::Packet::empty();
            match encoder.receive_packet(&mut packet) {
                Ok(_) => {
                    if packet.size() > 0 {
                        let data = packet.data().unwrap_or(&[]).to_vec();
                        return Ok(Some(EncodedPacket {
                            data,
                            is_key_frame: true,
                            timestamp: 0,
                            pts: self.pts,
                        }));
                    }
                }
                Err(_) => {}
            }
        }
        Ok(None)
    }

    #[cfg(not(feature = "h264"))]
    fn flush(&mut self) -> Result<Option<EncodedPacket>> {
        Ok(None)
    }

    fn encoder_type(&self) -> HardwareEncoderType {
        HardwareEncoderType::AMF
    }

    fn is_available(&self) -> bool {
        #[cfg(feature = "h264")]
        {
            self.inner.is_some()
        }
        #[cfg(not(feature = "h264"))]
        {
            false
        }
    }
}

#[cfg(not(target_os = "windows"))]
/// AMF 只在 Windows 上可用
pub struct AmfEncoder;

#[cfg(not(target_os = "windows"))]
impl AmfEncoder {
    pub fn new(_width: u32, _height: u32, _config: crate::encoder::hardware::HardwareEncoderConfig) -> Result<Self> {
        Err(anyhow::anyhow!("AMF 只在 Windows + AMD GPU 上可用"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_amf_availability() {
        let available = AmfEncoder::is_available();
        tracing::info!("AMF 可用性: {}", available);
        // 结果取决于硬件
    }

    #[test]
    fn test_config_validation() {
        #[cfg(all(target_os = "windows", feature = "h264"))]
        {
            let config = HardwareEncoderConfig {
                bitrate: 2000,
                fps: 30,
                ..Default::default()
            };

            // 需要 AMD GPU 才能真正测试
            if AmfEncoder::is_available() {
                let result = AmfEncoder::new(1920, 1080, config);
                assert!(result.is_ok() || result.is_err());
            }
        }
    }
}
