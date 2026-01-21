//! 简单的 WebSocket 测试服务器
//!
//! 用于接收和显示 sscontrol 发送的视频数据

use futures_util::{SinkExt, StreamExt};
use std::time::SystemTime;
use tokio_tungstenite::tungstenite::protocol::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("WebSocket 测试服务器");
    println!("监听: ws://127.0.0.1:8080");
    println!("按 Ctrl+C 退出\n");

    let addr = "127.0.0.1:8080";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("服务器已启动: {}\n", addr);

    let mut connection_count = 0u64;
    let mut total_bytes = 0u64;

    while let Ok((stream, addr)) = listener.accept().await {
        connection_count += 1;
        let conn_id = connection_count;

        println!("[{}] 新连接来自: {}", conn_id, addr);

        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // 处理这个连接
        tokio::spawn(async move {
            let mut bytes_received = 0u64;
            let mut frames_received = 0u64;
            let start_time = SystemTime::now();

            println!("[{}] 连接已建立", conn_id);

            loop {
                match ws_receiver.next().await {
                    Some(Ok(msg)) => {
                        match msg {
                            Message::Binary(data) => {
                                bytes_received += data.len() as u64;
                                frames_received += 1;

                                // 解析帧头
                                if data.len() >= 24 {
                                    let magic = &data[0..4];
                                    if magic == [0xFF, 0xFF, 0xFF, 0xFF] {
                                        let width = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                                        let height = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
                                        let timestamp = u64::from_be_bytes(data[12..20].try_into().unwrap());
                                        let data_size = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);

                                        if frames_received % 30 == 0 {
                                            let elapsed = start_time.elapsed().unwrap_or_default().as_secs_f64();
                                            let fps = if elapsed > 0.0 {
                                                frames_received as f64 / elapsed
                                            } else {
                                                0.0
                                            };
                                            let mbps = if elapsed > 0.0 {
                                                (bytes_received as f64 / 1_000_000.0) / elapsed
                                            } else {
                                                0.0
                                            };

                                            println!("[{}] 帧: #{} | {}x{} | {} bytes | FPS: {:.1} | {:.2} MB/s",
                                                conn_id,
                                                frames_received,
                                                width,
                                                height,
                                                data_size,
                                                fps,
                                                mbps
                                            );
                                        }
                                    }
                                }
                            }
                            Message::Text(text) => {
                                println!("[{}] 文本消息: {}", conn_id, text);
                            }
                            Message::Close(_) => {
                                println!("[{}] 连接关闭", conn_id);
                                break;
                            }
                            _ => {}
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("[{}] 错误: {}", conn_id, e);
                        break;
                    }
                    None => {
                        println!("[{}] 连接关闭", conn_id);
                        break;
                    }
                }
            }

            let elapsed = start_time.elapsed().unwrap_or_default().as_secs_f64();
            let fps = if elapsed > 0.0 { frames_received as f64 / elapsed } else { 0.0 };
            let total_mb = bytes_received as f64 / 1_000_000.0;

            println!("[{}] 连接统计: {} 帧 | {:.2} MB | 平均 FPS: {:.1} | 用时: {:.1}s",
                conn_id,
                frames_received,
                total_mb,
                fps,
                elapsed
            );
        });
    }

    Ok(())
}
