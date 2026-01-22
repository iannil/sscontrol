//! 连接码生成和解析
//!
//! 连接码格式: XXXX-XXXX-XXXX-XXXX (16 字符 Base32)
//!
//! 编码内容 (10 bytes = 80 bits = 16 base32 chars):
//! - session_id: 3 bytes (随机，16M 种可能)
//! - timestamp: 3 bytes (Unix 时间戳 / 60，分钟精度，可用 32 年)
//! - pin: 2 bytes (4 位 PIN 码 0000-9999)
//! - checksum: 2 bytes (CRC16 of first 8 bytes)

use crc::{Crc, CRC_16_IBM_SDLC};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// 连接码长度 (不含分隔符)
const CODE_LENGTH: usize = 16;

/// 连接码数据长度 (bytes)
const DATA_LENGTH: usize = 10;

/// 默认有效期 (秒)
pub const DEFAULT_TTL_SECS: u64 = 300;

/// CRC16 计算器
const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);

/// 连接码错误
#[derive(Debug, Error)]
pub enum ConnectionCodeError {
    #[error("Invalid code format")]
    InvalidFormat,

    #[error("Invalid checksum")]
    InvalidChecksum,

    #[error("Code has expired")]
    Expired,

    #[error("Invalid PIN")]
    InvalidPin,

    #[error("Base32 decode error")]
    DecodeError,
}

/// 连接码结构
#[derive(Debug, Clone)]
pub struct ConnectionCode {
    /// 会话 ID (3 bytes, 0-16777215)
    pub session_id: u32,

    /// 创建时间戳 (分钟精度，3 bytes)
    pub timestamp: u32,

    /// 4 位 PIN 码 (0000-9999)
    pub pin: u16,

    /// 有效期 (秒)
    pub ttl: u64,
}

impl ConnectionCode {
    /// 生成新的连接码
    pub fn generate() -> Self {
        Self::generate_with_ttl(DEFAULT_TTL_SECS)
    }

    /// 生成指定有效期的连接码
    pub fn generate_with_ttl(ttl: u64) -> Self {
        let mut rng = rand::thread_rng();

        // 3 bytes session_id (0 - 16777215)
        let session_id: u32 = rng.gen::<u32>() & 0x00FFFFFF;
        let pin: u16 = rng.gen_range(0..10000);

        let timestamp = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
            / 60) as u32
            & 0x00FFFFFF; // 3 bytes

        Self {
            session_id,
            timestamp,
            pin,
            ttl,
        }
    }

    /// 编码为字符串 (XXXX-XXXX-XXXX-XXXX 格式)
    pub fn encode(&self) -> String {
        let bytes = self.to_bytes();
        let encoded = base32::encode(base32::Alphabet::Crockford, &bytes);

        // 格式化为 XXXX-XXXX-XXXX-XXXX
        let chars: Vec<char> = encoded.chars().collect();
        format!(
            "{}-{}-{}-{}",
            chars[0..4].iter().collect::<String>(),
            chars[4..8].iter().collect::<String>(),
            chars[8..12].iter().collect::<String>(),
            chars[12..16].iter().collect::<String>(),
        )
    }

    /// 编码为紧凑字符串 (无分隔符)
    pub fn encode_compact(&self) -> String {
        let bytes = self.to_bytes();
        base32::encode(base32::Alphabet::Crockford, &bytes)
    }

    /// 从字符串解码
    pub fn decode(code: &str) -> Result<Self, ConnectionCodeError> {
        // 移除分隔符和空格
        let clean: String = code
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect();

        if clean.len() != CODE_LENGTH {
            return Err(ConnectionCodeError::InvalidFormat);
        }

        // Base32 解码
        let bytes = base32::decode(base32::Alphabet::Crockford, &clean)
            .ok_or(ConnectionCodeError::DecodeError)?;

        if bytes.len() != DATA_LENGTH {
            return Err(ConnectionCodeError::InvalidFormat);
        }

        Self::from_bytes(&bytes)
    }

    /// 验证连接码是否有效 (未过期)
    pub fn is_valid(&self) -> bool {
        let now = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
            / 60) as u32
            & 0x00FFFFFF; // 3 bytes, must match timestamp format

        let elapsed_minutes = now.saturating_sub(self.timestamp);
        let ttl_minutes = (self.ttl / 60) as u32;

        elapsed_minutes <= ttl_minutes
    }

    /// 验证 PIN 码
    pub fn verify_pin(&self, pin: u16) -> bool {
        self.pin == pin
    }

    /// 获取剩余有效时间 (秒)
    pub fn remaining_secs(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        let created_secs = (self.timestamp as u64) * 60;
        let expires_secs = created_secs + self.ttl;

        expires_secs.saturating_sub(now)
    }

    /// 转换为字节数组 (10 bytes)
    /// Layout: session_id[3] + timestamp[3] + pin[2] + checksum[2]
    fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(DATA_LENGTH);

        // session_id (3 bytes, big-endian, use lower 3 bytes)
        data.push(((self.session_id >> 16) & 0xFF) as u8);
        data.push(((self.session_id >> 8) & 0xFF) as u8);
        data.push((self.session_id & 0xFF) as u8);

        // timestamp (3 bytes, big-endian, use lower 3 bytes)
        data.push(((self.timestamp >> 16) & 0xFF) as u8);
        data.push(((self.timestamp >> 8) & 0xFF) as u8);
        data.push((self.timestamp & 0xFF) as u8);

        // pin (2 bytes, big-endian)
        data.extend_from_slice(&self.pin.to_be_bytes());

        // 计算 checksum (对前 8 bytes: session_id + timestamp + pin)
        let checksum = CRC16.checksum(&data[..8]);
        data.extend_from_slice(&checksum.to_be_bytes());

        data
    }

    /// 从字节数组解析
    fn from_bytes(bytes: &[u8]) -> Result<Self, ConnectionCodeError> {
        if bytes.len() != DATA_LENGTH {
            return Err(ConnectionCodeError::InvalidFormat);
        }

        // 验证 checksum (对前 8 bytes)
        let expected_checksum = CRC16.checksum(&bytes[..8]);
        let actual_checksum = u16::from_be_bytes([bytes[8], bytes[9]]);

        if expected_checksum != actual_checksum {
            return Err(ConnectionCodeError::InvalidChecksum);
        }

        // session_id (3 bytes)
        let session_id = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);

        // timestamp (3 bytes)
        let timestamp = ((bytes[3] as u32) << 16) | ((bytes[4] as u32) << 8) | (bytes[5] as u32);

        // pin (2 bytes)
        let pin = u16::from_be_bytes([bytes[6], bytes[7]]);

        Ok(Self {
            session_id,
            timestamp,
            pin,
            ttl: DEFAULT_TTL_SECS,
        })
    }

    /// 获取 session_id 的十六进制字符串
    pub fn session_id_hex(&self) -> String {
        format!("{:06x}", self.session_id)
    }
}

impl std::fmt::Display for ConnectionCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_encode() {
        let code = ConnectionCode::generate();
        let encoded = code.encode();

        // 验证格式: XXXX-XXXX-XXXX-XXXX
        assert_eq!(encoded.len(), 19); // 16 + 3 dashes
        assert!(encoded.chars().filter(|c| *c == '-').count() == 3);

        println!("Generated code: {}", encoded);
        println!("PIN: {:04}", code.pin);
        println!("Session ID: {}", code.session_id_hex());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = ConnectionCode::generate();
        let encoded = original.encode();
        println!("Encoded: {}", encoded);

        let decoded = ConnectionCode::decode(&encoded).expect("Decode failed");

        assert_eq!(original.session_id, decoded.session_id);
        assert_eq!(original.timestamp, decoded.timestamp);
        assert_eq!(original.pin, decoded.pin);
    }

    #[test]
    fn test_decode_with_spaces() {
        let code = ConnectionCode::generate();
        let encoded = code.encode();

        // 带空格的输入
        let with_spaces = encoded.replace("-", " - ");
        let decoded = ConnectionCode::decode(&with_spaces).expect("Decode failed");

        assert_eq!(code.session_id, decoded.session_id);
    }

    #[test]
    fn test_validity() {
        let code = ConnectionCode::generate_with_ttl(60); // 1 分钟有效期
        assert!(code.is_valid());
        assert!(code.remaining_secs() <= 60);
    }

    #[test]
    fn test_pin_verification() {
        let code = ConnectionCode::generate();
        assert!(code.verify_pin(code.pin));
        assert!(!code.verify_pin((code.pin + 1) % 10000));
    }

    #[test]
    fn test_invalid_checksum() {
        let code = ConnectionCode::generate();
        let mut bytes = code.to_bytes();

        // 修改数据破坏 checksum
        bytes[0] ^= 0xFF;

        let result = ConnectionCode::from_bytes(&bytes);
        assert!(matches!(result, Err(ConnectionCodeError::InvalidChecksum)));
    }

    #[test]
    fn test_session_id_hex() {
        let code = ConnectionCode {
            session_id: 0x123456,
            timestamp: 0,
            pin: 0,
            ttl: 300,
        };
        assert_eq!(code.session_id_hex(), "123456");
    }
}
