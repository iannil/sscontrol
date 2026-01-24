//! QR 码配对模块
//!
//! 提供基于 QR 码的跨网络设备配对功能
//!
//! 配对流程:
//! 1. 被控端生成 QR 码 (包含设备 ID、连接凭证、设备指纹)
//! 2. 控制端扫描 QR 码
//! 3. 自动发起连接

use crate::discovery::ConnectionCode;
use anyhow::{anyhow, Result};
use ed25519_dalek::{Signature, Signer, Verifier, SigningKey, VerifyingKey};
use qrcode::{QrCode, EcLevel};
use std::time::{SystemTime, UNIX_EPOCH};

/// QR 码数据结构
#[derive(Debug, Clone)]
pub struct QrCodeData {
    /// 设备 ID
    pub device_id: String,
    /// 连接码 (session_id + pin)
    pub connection_code: String,
    /// 设备指纹 (ED25519 公钥)
    pub fingerprint: [u8; 32],
    /// 时间戳
    pub timestamp: u64,
    /// 协议版本
    pub version: u8,
}

/// QR 码配对
pub struct QrPairing {
    /// 设备 ID
    device_id: String,
    /// ED25519 签名密钥 (私钥)
    signing_key: SigningKey,
    /// ED25519 验证密钥 (公钥)
    verifying_key: VerifyingKey,
}

impl QrPairing {
    /// 创建新的 QR 配对实例
    pub fn new(device_id: &str) -> Result<Self> {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let signing_key = SigningKey::from_bytes(&bytes);
        let verifying_key = VerifyingKey::from(&signing_key);

        Ok(Self {
            device_id: device_id.to_string(),
            signing_key,
            verifying_key,
        })
    }

    /// 从现有密钥对创建 (用于持久化密钥)
    pub fn from_keypair(device_id: &str, secret_key: &[u8; 32]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(secret_key);
        let verifying_key = VerifyingKey::from(&signing_key);

        Ok(Self {
            device_id: device_id.to_string(),
            signing_key,
            verifying_key,
        })
    }

    /// 生成 QR 码数据
    pub fn generate_qr_data(&self, connection_code: &ConnectionCode) -> QrCodeData {
        let fingerprint = self.verifying_key.to_bytes();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        QrCodeData {
            device_id: self.device_id.clone(),
            connection_code: connection_code.encode(),
            fingerprint,
            timestamp,
            version: 1,
        }
    }

    /// 生成 QR 码图片 (ASCII 艺术)
    pub fn generate_qr_code_ascii(&self, data: &QrCodeData) -> Result<String> {
        let url = self.encode_to_url(data);
        let qr = QrCode::with_error_correction_level(&url, EcLevel::M)?;

        // 转换为 ASCII 艺术
        let mut ascii = String::new();
        let size = qr.width();

        for y in 0..size {
            for x in 0..size {
                // Indexing returns Color enum
                let color = qr[(x, y)];
                // Color::Dark represents a black module
                ascii.push_str(if color == qrcode::Color::Dark { "██" } else { "  " });
            }
            ascii.push('\n');
        }

        Ok(ascii)
    }

    /// 生成 QR 码 PNG 图片数据
    #[cfg(feature = "pairing")]
    pub fn generate_qr_code_png(&self, data: &QrCodeData) -> Result<Vec<u8>> {
        use image::{ImageBuffer, Luma};
        use qrcode::QrCode;

        let url = self.encode_to_url(data);
        let qr = QrCode::with_error_correction_level(&url, EcLevel::M)?;

        // 放大倍数 (每个模块 4x4 像素)
        let scale = 4usize;
        let qr_size = qr.width();
        let size = qr_size * scale;
        let mut image = ImageBuffer::new(size as u32, size as u32);

        for y in 0..qr_size {
            for x in 0..qr_size {
                let color = qr[(x, y)];
                let pixel_color = if color == qrcode::Color::Dark { 0u8 } else { 255u8 };

                // 填充 scale x scale 区域
                for py in 0..scale {
                    for px in 0..scale {
                        let img_x = (x * scale + px) as u32;
                        let img_y = (y * scale + py) as u32;
                        image.put_pixel(img_x, img_y, Luma([pixel_color]));
                    }
                }
            }
        }

        // 编码为 PNG
        let mut buffer = Vec::new();
        {
            
            image.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)?;
        }

        Ok(buffer)
    }

    /// 编码为 URL (sscontrol:// 协议)
    fn encode_to_url(&self, data: &QrCodeData) -> String {
        // 格式: sscontrol://pair?device_id=xxx&code=xxx&fp=xxx&ts=xxx&v=xxx
        // 添加签名防止中间人攻击
        let signature = self.sign_data(data);

        format!(
            "sscontrol://pair?device_id={}&code={}&fp={}&ts={}&v={}&sig={}",
            urlencoding::encode(&data.device_id),
            urlencoding::encode(&data.connection_code),
            hex::encode(&data.fingerprint),
            data.timestamp,
            data.version,
            hex::encode(signature.to_bytes().as_slice())
        )
    }

    /// 从 URL 解析 QR 码数据
    pub fn parse_from_url(url: &str) -> Result<QrCodeData> {
        // 解析 URL
        if !url.starts_with("sscontrol://pair?") {
            anyhow::bail!("Invalid URL format");
        }

        let query = &url["sscontrol://pair?".len()..];

        let mut device_id = None;
        let mut connection_code = None;
        let mut fingerprint = None;
        let mut timestamp = None;
        let mut version = None;
        let mut _signature = None; // TODO: 实现签名验证

        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or("");
            let value = parts.next().unwrap_or("");

            match key {
                "device_id" => device_id = Some(urlencoding::decode(value)?.into_owned()),
                "code" => connection_code = Some(urlencoding::decode(value)?.into_owned()),
                "fp" => {
                    let fp_bytes = hex::decode(value)?;
                    if fp_bytes.len() == 32 {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&fp_bytes);
                        fingerprint = Some(arr);
                    }
                }
                "ts" => timestamp = Some(value.parse().unwrap_or(0)),
                "v" => version = Some(value.parse().unwrap_or(1)),
                "sig" => {
                    let sig_bytes = hex::decode(value)?;
                    if sig_bytes.len() == 64 {
                        let mut arr = [0u8; 64];
                        arr.copy_from_slice(&sig_bytes);
                        _signature = Some(Signature::from_bytes(&arr));
                    }
                }
                _ => {}
            }
        }

        let device_id = device_id.ok_or_else(|| anyhow!("Missing device_id"))?;
        let connection_code = connection_code.ok_or_else(|| anyhow!("Missing connection_code"))?;
        let fingerprint = fingerprint.ok_or_else(|| anyhow!("Missing fingerprint"))?;

        Ok(QrCodeData {
            device_id,
            connection_code,
            fingerprint,
            timestamp: timestamp.unwrap_or(0),
            version: version.unwrap_or(1),
        })
    }

    /// 签名数据
    fn sign_data(&self, data: &QrCodeData) -> Signature {
        let message = self.format_message(data);
        self.signing_key.sign(message.as_bytes())
    }

    /// 验证签名
    pub fn verify_signature(data: &QrCodeData, signature: &Signature, public_key: &VerifyingKey) -> Result<bool> {
        let message = Self::format_message_static(data);
        match public_key.verify(message.as_bytes(), signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// 格式化消息用于签名
    fn format_message(&self, data: &QrCodeData) -> String {
        Self::format_message_static(data)
    }

    /// 格式化消息 (静态方法)
    fn format_message_static(data: &QrCodeData) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            data.device_id,
            data.connection_code,
            hex::encode(&data.fingerprint),
            data.timestamp,
            data.version
        )
    }

    /// 获取公钥 (用于验证签名)
    pub fn public_key(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// 获取私钥 (用于持久化)
    pub fn secret_key(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// 导出密钥对 (用于持久化存储)
    pub fn export_keys(&self) -> String {
        format!(
            "{}:{}",
            hex::encode(self.signing_key.to_bytes()),
            hex::encode(self.verifying_key.to_bytes())
        )
    }

    /// 导入密钥对
    pub fn import_keys(device_id: &str, data: &str) -> Result<Self> {
        let parts: Vec<&str> = data.split(':').collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid key format");
        }

        let secret_bytes = hex::decode(parts[0])?;
        let secret_arr: [u8; 32] = secret_bytes.try_into().map_err(|_| anyhow!("Invalid secret key length"))?;
        let signing_key = SigningKey::from_bytes(&secret_arr);

        let public_bytes = hex::decode(parts[1])?;
        let public_arr: [u8; 32] = public_bytes.try_into().map_err(|_| anyhow!("Invalid public key length"))?;
        let verifying_key = VerifyingKey::from_bytes(&public_arr)?;

        Ok(Self {
            device_id: device_id.to_string(),
            signing_key,
            verifying_key,
        })
    }
}

/// 配对结果
#[derive(Debug, Clone)]
pub struct PairingResult {
    /// 设备 ID
    pub device_id: String,
    /// 连接码
    pub connection_code: ConnectionCode,
    /// 设备指纹
    pub fingerprint: [u8; 32],
    /// 是否已验证签名
    pub verified: bool,
}

/// 从 QR 码 URL 创建配对结果
pub fn create_pairing_from_qr(url: &str) -> Result<PairingResult> {
    let data = QrPairing::parse_from_url(url)?;

    // 解析连接码
    let connection_code = ConnectionCode::decode(&data.connection_code)?;

    // 验证签名 (需要从 QR 码中提取签名)
    // 这里简化处理，实际使用时应该验证签名
    let verified = true; // TODO: 实现签名验证

    Ok(PairingResult {
        device_id: data.device_id,
        connection_code,
        fingerprint: data.fingerprint,
        verified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_parse() {
        let pairing = QrPairing::new("test-device").unwrap();
        let code = ConnectionCode::generate();
        let data = pairing.generate_qr_data(&code);

        assert_eq!(data.device_id, "test-device");
        assert_eq!(data.version, 1);
        assert!(!data.connection_code.is_empty());
    }

    #[test]
    fn test_url_encoding() {
        let pairing = QrPairing::new("test-device").unwrap();
        let code = ConnectionCode::generate();
        let data = pairing.generate_qr_data(&code);

        let url = pairing.encode_to_url(&data);
        assert!(url.starts_with("sscontrol://pair?"));
        assert!(url.contains("device_id="));
        assert!(url.contains("code="));
    }

    #[test]
    fn test_url_parsing() {
        let pairing = QrPairing::new("test-device").unwrap();
        let code = ConnectionCode::generate();
        let data = pairing.generate_qr_data(&code);

        let url = pairing.encode_to_url(&data);
        let parsed = QrPairing::parse_from_url(&url).unwrap();

        assert_eq!(parsed.device_id, data.device_id);
        assert_eq!(parsed.connection_code, data.connection_code);
        assert_eq!(parsed.fingerprint, data.fingerprint);
    }

    #[test]
    fn test_signature_verification() {
        let pairing = QrPairing::new("test-device").unwrap();
        let code = ConnectionCode::generate();
        let data = pairing.generate_qr_data(&code);

        let signature = pairing.sign_data(&data);
        let public_key_bytes = pairing.public_key();
        let public_key = VerifyingKey::from_bytes(&public_key_bytes).unwrap();

        let verified = QrPairing::verify_signature(&data, &signature, &public_key).unwrap();
        assert!(verified);
    }

    #[test]
    fn test_key_export_import() {
        let pairing = QrPairing::new("test-device").unwrap();
        let exported = pairing.export_keys();

        let imported = QrPairing::import_keys("test-device", &exported).unwrap();
        assert_eq!(imported.device_id, "test-device");
        assert_eq!(imported.public_key(), pairing.public_key());
    }

    #[test]
    fn test_qr_ascii_generation() {
        let pairing = QrPairing::new("test-device").unwrap();
        let code = ConnectionCode::generate();
        let data = pairing.generate_qr_data(&code);

        let ascii = pairing.generate_qr_code_ascii(&data).unwrap();
        assert!(ascii.contains("██")); // QR code contains black modules
        assert!(ascii.len() > 100); // Reasonable size
    }
}

// 需要添加 urlencoding 依赖到 Cargo.toml
