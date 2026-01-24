//! 视频编码模块
//!
//! 提供视频帧编码功能

// H.264/VP8 编码器需要 FFmpeg，尚未完全集成，标记为允许死代码
#![allow(dead_code)]

// 硬件编码器抽象层
pub mod hardware;

// 平台特定的硬件编码器
#[cfg(target_os = "macos")]
pub mod videotoolbox;

#[cfg(target_os = "windows")]
pub mod nvenc;

#[cfg(target_os = "windows")]
pub mod amf;

#[cfg(target_os = "windows")]
pub mod qsv;

use crate::capture::Frame;
use anyhow::Result;

/// 编码后的数据包
#[derive(Debug, Clone)]
pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub is_key_frame: bool,
    pub timestamp: u64,
    pub pts: i64,
}

/// 视频编码器 trait
pub trait Encoder: Send {
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

    /// 设置码率 (kbps)
    ///
    /// 默认实现：不支持动态码率调整
    /// 编码器可以重写此方法以实现动态码率调整
    fn set_bitrate(&mut self, _bitrate_kbps: u32) -> Result<()> {
        Ok(())
    }
}

/// 简单编码器 - 直接传输原始帧数据
///
/// 注意: 这是一个 MVP 实现，直接传输原始 RGBA 数据
/// 生产环境应该使用 H.264 编码器
pub struct SimpleEncoder {
    width: u32,
    height: u32,
    #[allow(dead_code)]
    fps: u32,
    frame_count: u64,
}

impl SimpleEncoder {
    /// 创建新的简单编码器
    ///
    /// # 参数
    /// * `width` - 视频宽度
    /// * `height` - 视频高度
    /// * `fps` - 目标帧率
    pub fn new(width: u32, height: u32, fps: u32, _bitrate: u32) -> Result<Self> {
        tracing::info!(
            "创建简单编码器: {}x{} @ {}fps",
            width,
            height,
            fps
        );

        Ok(SimpleEncoder {
            width,
            height,
            fps,
            frame_count: 0,
        })
    }
}

impl Encoder for SimpleEncoder {
    fn encode(&mut self, frame: &Frame) -> Result<Option<EncodedPacket>> {
        // 简单编码: 添加帧头 + 原始数据
        // 帧头格式: [magic(4)][width(4)][height(4)][timestamp(8)][data_size(4)]
        let mut packet_data = Vec::with_capacity(24 + frame.data.len());

        // Magic number
        packet_data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        // Width
        packet_data.extend_from_slice(&frame.width.to_be_bytes());
        // Height
        packet_data.extend_from_slice(&frame.height.to_be_bytes());
        // Timestamp
        packet_data.extend_from_slice(&frame.timestamp.to_be_bytes());
        // Data size
        packet_data.extend_from_slice(&(frame.data.len() as u32).to_be_bytes());
        // Pixel data
        packet_data.extend_from_slice(&frame.data);

        self.frame_count += 1;
        let is_key_frame = self.frame_count.is_multiple_of(30); // 每30帧一个关键帧

        Ok(Some(EncodedPacket {
            data: packet_data,
            is_key_frame,
            timestamp: frame.timestamp,
            pts: self.frame_count as i64,
        }))
    }

    fn request_key_frame(&mut self) -> Result<()> {
        self.frame_count = 0; // 下一帧将是关键帧
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
}

/// H.264 编码器 (使用 FFmpeg)
///
/// 注意: 需要安装 FFmpeg 开发库
/// 使用 ffmpeg-next crate
///
/// **重要**: FFmpeg 的 SwsContext 不是 Send，但我们在单线程编码器中使用是安全的
#[cfg(feature = "h264")]
pub struct H264Encoder {
    width: u32,
    height: u32,
    #[allow(dead_code)]
    fps: u32,
    #[allow(dead_code)]
    bitrate: u32,
    encoder: Option<ffmpeg::encoder::Video>,
    sws_context: Option<ffmpeg::software::scaling::Context>,
    pts: i64,
    key_frame_interval: u64,
    frame_count: u64,
}

// SAFETY: FFmpeg SwsContext 在单线程使用时是安全的
#[cfg(feature = "h264")]
unsafe impl Send for H264Encoder {}

#[cfg(feature = "h264")]
use ffmpeg_next as ffmpeg;

#[cfg(feature = "h264")]
use anyhow::anyhow;

#[cfg(feature = "h264")]
impl H264Encoder {
    /// 创建新的 H.264 编码器
    ///
    /// # 参数
    /// * `width` - 视频宽度
    /// * `height` - 视频高度
    /// * `fps` - 目标帧率
    /// * `bitrate` - 目标码率 (kbps)
    pub fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<Self> {
        tracing::info!(
            "创建 H.264 编码器: {}x{} @ {}fps, {}kbps",
            width,
            height,
            fps,
            bitrate
        );

        // 初始化 FFmpeg (仅第一次)
        ffmpeg::init()?;

        // 查找 H.264 编码器
        let encoder = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
            .ok_or_else(|| anyhow!("找不到 H.264 编码器"))?;

        // 配置编码器
        let context = ffmpeg::codec::context::Context::new_with_codec(encoder);
        let mut encoder_context = context.encoder().video()?;

        encoder_context.set_bit_rate((bitrate * 1000) as usize);
        encoder_context.set_width(width);
        encoder_context.set_height(height);
        encoder_context.set_frame_rate(Some(ffmpeg::Rational(fps as i32, 1)));
        encoder_context.set_time_base(ffmpeg::Rational(1, fps as i32));
        encoder_context.set_gop(30);
        encoder_context.set_format(ffmpeg::format::Pixel::YUV420P);

        // 设置低延迟编码参数
        let mut opts = ffmpeg::Dictionary::new();
        opts.set("preset", "ultrafast");
        opts.set("tune", "zerolatency");
        opts.set("rc-lookahead", "0");

        // 打开编码器
        let video_encoder = encoder_context.open_with(opts)?;

        // 创建 SwsContext 用于 RGBA -> YUV420P 转换
        let sws_context = ffmpeg::software::scaling::Context::get(
            ffmpeg::format::Pixel::RGBA,
            width,
            height,
            ffmpeg::format::Pixel::YUV420P,
            width,
            height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?;

        tracing::info!("H.264 编码器创建成功 (ultrafast/zerolatency)");
        Ok(H264Encoder {
            width,
            height,
            fps,
            bitrate,
            encoder: Some(video_encoder),
            sws_context: Some(sws_context),
            pts: 0,
            key_frame_interval: 30,
            frame_count: 0,
        })
    }

    /// 将 RGBA 帧转换为 YUV420P (使用 SwsContext 硬件加速)
    fn rgba_to_yuv420p_frame(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<ffmpeg::frame::Video> {
        // 创建源帧 (RGBA)
        let mut src_frame = ffmpeg::frame::Video::empty();
        src_frame.set_format(ffmpeg::format::Pixel::RGBA);
        src_frame.set_width(width);
        src_frame.set_height(height);

        unsafe {
            src_frame.alloc(ffmpeg::format::Pixel::RGBA, width, height);
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

        // 创建目标帧 (YUV420P)
        let mut dst_frame = ffmpeg::frame::Video::empty();
        dst_frame.set_format(ffmpeg::format::Pixel::YUV420P);
        dst_frame.set_width(width);
        dst_frame.set_height(height);

        unsafe {
            dst_frame.alloc(ffmpeg::format::Pixel::YUV420P, width, height);
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

#[cfg(feature = "h264")]
impl Encoder for H264Encoder {
    fn encode(&mut self, frame: &Frame) -> Result<Option<EncodedPacket>> {
        // 阶段 1: 转换为 YUV420P (使用 sws_context)
        // 注意: 必须先完成此操作再获取 encoder 引用，避免借用冲突
        let mut yuv_frame = self.rgba_to_yuv420p_frame(&frame.data, frame.width, frame.height)?;

        // 设置 PTS
        yuv_frame.set_pts(Some(self.pts));
        self.pts += 1;
        self.frame_count += 1;

        // 判断是否为关键帧
        let is_key_frame = self.frame_count % self.key_frame_interval == 0;

        // 阶段 2: 编码 (使用 encoder)
        let encoder = self.encoder.as_mut().ok_or_else(|| anyhow!("编码器未初始化"))?;
        encoder.send_frame(&yuv_frame)?;

        let mut packet = ffmpeg::packet::Packet::empty();
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
                // 可能需要更多帧
                let err_msg = e.to_string();
                if err_msg.contains("more frames") || err_msg.contains("flushing") {
                    Ok(None)
                } else {
                    Err(anyhow!("编码失败: {}", e))
                }
            }
        }
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

    fn flush(&mut self) -> Result<Option<EncodedPacket>> {
        if let Some(encoder) = self.encoder.as_mut() {
            let _ = encoder.send_eof();
            let mut packet = ffmpeg::packet::Packet::empty();
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

    fn set_bitrate(&mut self, bitrate_kbps: u32) -> Result<()> {
        // 尝试动态调整 FFmpeg 编码器的码率
        // 注意: 不是所有编码器都支持运行时码率调整
        if let Some(encoder) = self.encoder.as_mut() {
            // 更新内部记录
            self.bitrate = bitrate_kbps;

            // 尝试设置编码器码率 (以 bps 为单位)
            let bitrate_bps = bitrate_kbps as usize * 1000;

            // 通过 FFmpeg 的全局质量/码率设置来调整
            // 注意: 这种方式对 x264 等编码器可能不会立即生效
            tracing::debug!("尝试调整 H.264 编码器码率: {} kbps", bitrate_kbps);

            // FFmpeg 的编码器通常不支持运行时码率调整
            // 这里我们仅记录码率变化，实际效果取决于编码器实现
        }
        Ok(())
    }
}

/// H264Encoder 类型别名 (当 h264 feature 未启用时使用 SimpleEncoder)
#[cfg(not(feature = "h264"))]
pub type H264Encoder = SimpleEncoder;

/// VP8 编码器 (用于 WebRTC)
///
/// 使用 FFmpeg 的 VP8 编码器 (libvpx)
#[cfg(feature = "h264")]
pub struct VP8Encoder {
    width: u32,
    height: u32,
    #[allow(dead_code)]
    fps: u32,
    encoder: Option<ffmpeg::encoder::Video>,
    sws_context: Option<ffmpeg::software::scaling::Context>,
    pts: i64,
    key_frame_interval: u64,
    frame_count: u64,
}

#[cfg(feature = "h264")]
unsafe impl Send for VP8Encoder {}

#[cfg(feature = "h264")]
impl VP8Encoder {
    /// 创建新的 VP8 编码器
    pub fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<Self> {
        tracing::info!(
            "创建 VP8 编码器: {}x{} @ {}fps, {}kbps",
            width,
            height,
            fps,
            bitrate
        );

        // 初始化 FFmpeg (仅第一次)
        ffmpeg::init()?;

        // 查找 VP8 编码器 (libvpx)
        let encoder = ffmpeg::encoder::find(ffmpeg::codec::Id::VP8)
            .ok_or_else(|| anyhow!("找不到 VP8 编码器 (需要 libvpx)"))?;

        // 配置编码器
        let context = ffmpeg::codec::context::Context::new_with_codec(encoder);
        let mut encoder_context = context.encoder().video()?;

        encoder_context.set_bit_rate((bitrate * 1000) as usize);
        encoder_context.set_width(width);
        encoder_context.set_height(height);
        encoder_context.set_frame_rate(Some(ffmpeg::Rational(fps as i32, 1)));
        encoder_context.set_time_base(ffmpeg::Rational(1, fps as i32));
        encoder_context.set_gop(30);
        encoder_context.set_format(ffmpeg::format::Pixel::YUV420P);

        // 设置低延迟编码参数
        let mut opts = ffmpeg::Dictionary::new();
        opts.set("deadline", "realtime");
        opts.set("cpu-used", "8");  // 最快速度
        opts.set("lag-in-frames", "0");
        opts.set("error-resilient", "1");

        // 打开编码器
        let video_encoder = encoder_context.open_with(opts)?;

        // 创建 SwsContext 用于 RGBA -> YUV420P 转换
        let sws_context = ffmpeg::software::scaling::Context::get(
            ffmpeg::format::Pixel::RGBA,
            width,
            height,
            ffmpeg::format::Pixel::YUV420P,
            width,
            height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?;

        tracing::info!("VP8 编码器创建成功 (realtime mode)");
        Ok(VP8Encoder {
            width,
            height,
            fps,
            encoder: Some(video_encoder),
            sws_context: Some(sws_context),
            pts: 0,
            key_frame_interval: 30,
            frame_count: 0,
        })
    }

    /// 编码帧并返回 VP8 数据
    pub fn encode_frame(&mut self, frame: &Frame) -> Result<Option<Vec<u8>>> {
        // 转换为 YUV420P
        let mut yuv_frame = self.rgba_to_yuv420p_frame(&frame.data, frame.width, frame.height)?;

        // 设置 PTS
        yuv_frame.set_pts(Some(self.pts));
        self.pts += 1;
        self.frame_count += 1;

        // 编码
        let encoder = self.encoder.as_mut().ok_or_else(|| anyhow!("编码器未初始化"))?;
        encoder.send_frame(&yuv_frame)?;

        let mut packet = ffmpeg::packet::Packet::empty();
        match encoder.receive_packet(&mut packet) {
            Ok(_) => {
                if packet.size() > 0 {
                    Ok(Some(packet.data().unwrap_or(&[]).to_vec()))
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("more frames") || err_msg.contains("flushing") {
                    Ok(None)
                } else {
                    Err(anyhow!("VP8 编码失败: {}", e))
                }
            }
        }
    }

    /// 将 RGBA 帧转换为 YUV420P
    fn rgba_to_yuv420p_frame(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<ffmpeg::frame::Video> {
        let mut src_frame = ffmpeg::frame::Video::empty();
        src_frame.set_format(ffmpeg::format::Pixel::RGBA);
        src_frame.set_width(width);
        src_frame.set_height(height);

        unsafe {
            src_frame.alloc(ffmpeg::format::Pixel::RGBA, width, height);
        }

        let src_stride = src_frame.stride(0);
        let src_data = src_frame.data_mut(0);
        for y in 0..height as usize {
            let src_row_start = y * (width as usize * 4);
            let dst_row_start = y * src_stride;
            let row_len = width as usize * 4;
            src_data[dst_row_start..dst_row_start + row_len]
                .copy_from_slice(&rgba[src_row_start..src_row_start + row_len]);
        }

        let mut dst_frame = ffmpeg::frame::Video::empty();
        dst_frame.set_format(ffmpeg::format::Pixel::YUV420P);
        dst_frame.set_width(width);
        dst_frame.set_height(height);

        unsafe {
            dst_frame.alloc(ffmpeg::format::Pixel::YUV420P, width, height);
        }

        if let Some(ref mut sws) = self.sws_context {
            sws.run(&src_frame, &mut dst_frame)?;
        } else {
            return Err(anyhow!("SwsContext 未初始化"));
        }

        Ok(dst_frame)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

/// VP8Encoder 占位符 (当 h264 feature 未启用时)
#[cfg(not(feature = "h264"))]
pub struct VP8Encoder;

#[cfg(not(feature = "h264"))]
impl VP8Encoder {
    pub fn new(_width: u32, _height: u32, _fps: u32, _bitrate: u32) -> Result<Self> {
        Err(anyhow::anyhow!("VP8 编码器需要启用 h264 feature (FFmpeg)"))
    }
}

/// 创建编码器
///
/// # 参数
/// * `width` - 视频宽度
/// * `height` - 视频高度
/// * `fps` - 目标帧率
///
/// # 返回
/// 编码器实例
///
/// # 说明
/// 当启用 `h264` feature 时，返回 H.264 编码器
/// 否则返回 SimpleEncoder (原始数据)
pub fn create_encoder(width: u32, height: u32, fps: u32) -> Result<Box<dyn Encoder>> {
    #[cfg(feature = "h264")]
    {
        // 使用 H.264 编码器
        tracing::info!("使用 H.264 编码器");
        Ok(Box::new(H264Encoder::new(width, height, fps, 2000)?))
    }

    #[cfg(not(feature = "h264"))]
    {
        // 使用简单编码器 (原始数据)
        tracing::info!("使用 SimpleEncoder (原始数据)");
        Ok(Box::new(SimpleEncoder::new(width, height, fps, 2000)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_encoder_creation() {
        let encoder = SimpleEncoder::new(1920, 1080, 30, 2000);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_encoder_dimensions() {
        let encoder = SimpleEncoder::new(1280, 720, 30, 1000).unwrap();
        assert_eq!(encoder.width(), 1280);
        assert_eq!(encoder.height(), 720);
    }

    #[test]
    fn test_encode_frame() {
        let mut encoder = SimpleEncoder::new(1920, 1080, 30, 2000).unwrap();
        let frame = Frame::new(1920, 1080);

        let result = encoder.encode(&frame);
        assert!(result.is_ok());

        if let Ok(Some(packet)) = result {
            assert!(!packet.data.is_empty());
            assert!(packet.data.len() > 24); // 至少包含头部
        }
    }
}
