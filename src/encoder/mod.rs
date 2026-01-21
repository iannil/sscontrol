//! 视频编码模块
//!
//! 提供视频帧编码功能

use crate::capture::Frame;
use anyhow::{anyhow, Result};

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
}

/// 简单编码器 - 直接传输原始帧数据
///
/// 注意: 这是一个 MVP 实现，直接传输原始 RGBA 数据
/// 生产环境应该使用 H.264 编码器
pub struct SimpleEncoder {
    width: u32,
    height: u32,
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
        let is_key_frame = self.frame_count % 30 == 0; // 每30帧一个关键帧

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
    fps: u32,
    bitrate: u32,
    encoder: Option<ffmpeg::encoder::Video>,
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
        let mut context = ffmpeg::codec::context::Context::new();
        let mut encoder_context = context.encoder().video()?;

        encoder_context.set_bit_rate((bitrate * 1000) as usize);
        encoder_context.set_width(width);
        encoder_context.set_height(height);
        encoder_context.set_frame_rate(Some(ffmpeg::Rational(fps as i32, 1)));
        encoder_context.set_time_base(ffmpeg::Rational(1, fps as i32));
        encoder_context.set_gop(30);
        encoder_context.set_format(ffmpeg::format::Pixel::YUV420P);

        // 打开编码器
        let video_encoder = encoder_context.open()?;

        tracing::info!("H.264 编码器创建成功");
        Ok(H264Encoder {
            width,
            height,
            fps,
            bitrate,
            encoder: Some(video_encoder),
            pts: 0,
            key_frame_interval: 30,
            frame_count: 0,
        })
    }

    /// 将 RGBA 帧转换为 YUV420P
    fn rgba_to_yuv420p_frame(rgba: &[u8], width: u32, height: u32) -> ffmpeg::frame::Video {
        let mut yuv_frame = ffmpeg::frame::Video::empty();
        yuv_frame.set_format(ffmpeg::format::Pixel::YUV420P);
        yuv_frame.set_width(width);
        yuv_frame.set_height(height);

        unsafe {
            yuv_frame.alloc(ffmpeg::format::Pixel::YUV420P, width, height);
        }

        let width_usize = width as usize;
        let height_usize = height as usize;

        // 先获取 stride 信息
        let y_stride = yuv_frame.stride(0);
        let u_stride = yuv_frame.stride(1);
        let v_stride = yuv_frame.stride(2);

        // 然后分别获取可变引用
        for y in 0..height_usize {
            for x in 0..width_usize {
                let rgba_idx = (y * width_usize + x) * 4;
                let r = rgba[rgba_idx] as f32;
                let g = rgba[rgba_idx + 1] as f32;
                let b = rgba[rgba_idx + 2] as f32;

                // RGB to YUV conversion
                let y_val = (0.299 * r + 0.587 * g + 0.114 * b) as u8;
                let u_val = ((-0.169 * r - 0.331 * g + 0.5 * b) + 128.0) as u8;
                let v_val = ((0.5 * r - 0.419 * g - 0.081 * b) + 128.0) as u8;

                // 写入 Y 平面
                yuv_frame.data_mut(0)[y * y_stride + x] = y_val;

                if y % 2 == 0 && x % 2 == 0 {
                    let uv_x = x / 2;
                    let uv_y = y / 2;
                    // 写入 UV 平面
                    yuv_frame.data_mut(1)[uv_y * u_stride + uv_x] = u_val;
                    yuv_frame.data_mut(2)[uv_y * v_stride + uv_x] = v_val;
                }
            }
        }

        yuv_frame
    }
}

#[cfg(feature = "h264")]
impl Encoder for H264Encoder {
    fn encode(&mut self, frame: &Frame) -> Result<Option<EncodedPacket>> {
        let encoder = self.encoder.as_mut().ok_or_else(|| anyhow!("编码器未初始化"))?;

        // 转换为 YUV420P
        let mut yuv_frame = Self::rgba_to_yuv420p_frame(&frame.data, frame.width, frame.height);

        // 设置 PTS
        yuv_frame.set_pts(Some(self.pts));
        self.pts += 1;
        self.frame_count += 1;

        // 判断是否为关键帧
        let is_key_frame = self.frame_count % self.key_frame_interval == 0;

        // 编码
        encoder.send_frame(&yuv_frame)?;

        let mut packet = ffmpeg::packet::Packet::empty();
        match encoder.receive_packet(&mut packet) {
            Ok(_) => {
                if packet.size() > 0 {
                    let data = packet.data().unwrap_or(&[]).to_vec();
                    Ok(Some(EncodedPacket {
                        data,
                        is_key_frame: is_key_frame,
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
}

/// H264Encoder 类型别名 (当 h264 feature 未启用时使用 SimpleEncoder)
#[cfg(not(feature = "h264"))]
pub type H264Encoder = SimpleEncoder;

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
