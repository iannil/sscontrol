use crate::state::{AppState, ConnectionState, ConnectionStats};
use tauri::{AppHandle, Emitter, State};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Instant;

/// 运行中的连接句柄
struct RunningConnection {
    _abort_handle: tokio::task::AbortHandle,
    start_time: Instant,
    frames_sent: Arc<Mutex<u64>>,
    bytes_sent: Arc<Mutex<u64>>,
    reconnect_count: Arc<Mutex<u64>>,
}

static RUNNING_CONNECTION: Mutex<Option<RunningConnection>> = Mutex::const_new(None);

/// 启动连接
#[tauri::command]
pub async fn start_connection(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // 检查是否已有运行中的连接
    let mut running = RUNNING_CONNECTION.lock().await;
    if running.is_some() {
        return Err("连接已在运行中".to_string());
    }

    // 获取配置
    let config = state.config.read().await;
    let server_url = config.server.url.clone();
    let device_id = config.server.device_id.clone();
    let capture_config = config.capture.clone();
    let api_key = config.security.api_key.clone();
    drop(config);

    // 更新状态
    *state.connection_state.lock().await = ConnectionState::Connecting;

    // 创建统计追踪器
    let frames_sent = Arc::new(Mutex::new(0u64));
    let bytes_sent = Arc::new(Mutex::new(0u64));
    let reconnect_count = Arc::new(Mutex::new(0u64));
    let frames_sent_clone = frames_sent.clone();
    let bytes_sent_clone = bytes_sent.clone();
    let reconnect_count_clone = reconnect_count.clone();
    let connection_state = state.connection_state.clone();
    let app_clone = app.clone();

    // 启动连接任务
    let task = tokio::spawn(async move {
        // 创建视频客户端
        use sscontrol::network::{VideoClient, VideoClientConfig};

        let client_config = VideoClientConfig {
            auto_reconnect: true,
            reconnect_interval_ms: 2000,
            max_reconnect_attempts: None,
            connect_timeout_secs: 10,
            api_key,
            use_tls: server_url.starts_with("wss://"),
        };

        let client = VideoClient::with_config(server_url.clone(), device_id.clone(), client_config);

        // 发送状态更新
        let _ = app_clone.emit("connection-status", serde_json::json!({
            "status": "connecting",
        }));

        // 连接到服务器
        match client.connect().await {
            Ok(_) => {
                *connection_state.lock().await = ConnectionState::Connected;
                *reconnect_count_clone.lock().await = 0;

                let _ = app_clone.emit("connection-status", serde_json::json!({
                    "status": "connected",
                }));

                // 开始捕获和发送视频
                let capturer_result = sscontrol::capture::create_capturer(capture_config.screen_index);
                let mut capturer = match capturer_result {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = app_clone.emit("connection-status", serde_json::json!({
                            "status": "disconnected",
                            "error": format!("创建捕获器失败: {}", e),
                        }));
                        return;
                    }
                };

                let start = Instant::now();
                let mut frame_count = 0u64;
                let mut last_fps_update = Instant::now();

                loop {
                    // 检查连接状态
                    if !client.is_connected().await {
                        *connection_state.lock().await = ConnectionState::Disconnected;
                        break;
                    }

                    // 捕获帧
                    match capturer.capture() {
                        Ok(frame) => {
                            let data_len = frame.data.len();

                            // 发送帧
                            if client.send_packet(frame.data, false).await.is_err() {
                                *connection_state.lock().await = ConnectionState::Disconnected;
                                break;
                            }

                            // 更新统计
                            *frames_sent_clone.lock().await += 1;
                            *bytes_sent_clone.lock().await += data_len as u64;
                            frame_count += 1;

                            // 每秒发送一次统计更新
                            if last_fps_update.elapsed().as_secs() >= 1 {
                                let elapsed = start.elapsed().as_secs();
                                let fps = if elapsed > 0 {
                                    frame_count as f64 / elapsed as f64
                                } else {
                                    0.0
                                };

                                let _ = app_clone.emit("statistics-update", serde_json::json!({
                                    "status": "connected",
                                    "framesSent": *frames_sent_clone.lock().await,
                                    "fps": fps,
                                    "bytesSent": *bytes_sent_clone.lock().await,
                                    "uptime": elapsed,
                                    "reconnectCount": *reconnect_count_clone.lock().await,
                                }));

                                last_fps_update = Instant::now();
                            }
                        }
                        Err(e) => {
                            tracing::error!("捕获帧失败: {}", e);
                        }
                    }

                    // 控制帧率
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000 / capture_config.fps as u64)).await;
                }
            }
            Err(e) => {
                tracing::error!("连接失败: {}", e);
                *connection_state.lock().await = ConnectionState::Disconnected;

                let _ = app_clone.emit("connection-status", serde_json::json!({
                    "status": "disconnected",
                    "error": e.to_string(),
                }));
            }
        }
    });

    let abort_handle = task.abort_handle();
    *running = Some(RunningConnection {
        _abort_handle: abort_handle,
        start_time: Instant::now(),
        frames_sent,
        bytes_sent,
        reconnect_count,
    });

    Ok("连接已启动".to_string())
}

/// 停止连接
#[tauri::command]
pub async fn stop_connection(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut running = RUNNING_CONNECTION.lock().await;

    if running.is_none() {
        return Err("没有运行中的连接".to_string());
    }

    // 中断任务
    *running = None;

    *state.connection_state.lock().await = ConnectionState::Disconnected;

    // 发送状态更新
    let _ = app.emit("connection-status", serde_json::json!({
        "status": "disconnected",
    }));

    Ok("连接已停止".to_string())
}

/// 获取连接状态
#[tauri::command]
pub async fn get_connection_status(
    state: State<'_, AppState>,
) -> Result<String, String> {
    Ok(state.connection_state.lock().await.as_str().to_string())
}

/// 获取统计信息
#[tauri::command]
pub async fn get_statistics(
    state: State<'_, AppState>,
) -> Result<ConnectionStats, String> {
    let conn_state = state.connection_state.lock().await;
    let running = RUNNING_CONNECTION.lock().await;

    let stats = if let Some(conn) = &*running {
        let frames = *conn.frames_sent.lock().await;
        let bytes = *conn.bytes_sent.lock().await;
        let reconnects = *conn.reconnect_count.lock().await;
        let uptime = conn.start_time.elapsed().as_secs();

        ConnectionStats {
            status: conn_state.as_str().to_string(),
            frames_sent: frames,
            fps: 0.0, // 计算复杂，简化为 0
            bytes_sent: bytes,
            uptime,
            latency: 0,
            reconnect_count: reconnects,
        }
    } else {
        ConnectionStats::default()
    };

    Ok(stats)
}
