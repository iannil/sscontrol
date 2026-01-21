import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

// ===== 类型定义 =====

export interface Config {
  server: ServerConfig;
  capture: CaptureConfig;
  logging: LoggingConfig;
  security: SecurityConfig;
}

export interface ServerConfig {
  url: string;
  device_id: string;
}

export interface CaptureConfig {
  fps: number;
  screen_index: number | null;
  width: number | null;
  height: number | null;
}

export interface LoggingConfig {
  level: string;
  file: string | null;
}

export interface SecurityConfig {
  api_key: string | null;
  tls_cert: string | null;
  tls_key: string | null;
  require_tls: boolean;
  token_ttl: number;
}

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';

export interface ConnectionStats {
  status: ConnectionState;
  frames_sent: number;
  fps: number;
  bytes_sent: number;
  uptime: number;
  latency: number;
  reconnect_count: number;
}

export interface ScreenInfo {
  index: number;
  width: number;
  height: number;
  is_primary: boolean;
  name: string;
  scale_factor: number;
}

export interface ServiceStatus {
  installed: boolean;
  status: string;
}

// ===== 配置命令 =====

export const configApi = {
  getConfig: (): Promise<Config> => invoke('get_config'),
  getConfigPath: (): Promise<string> => invoke('get_config_path'),
  updateConfig: (config: Config): Promise<void> => invoke('update_config', { config }),
  resetConfig: (): Promise<Config> => invoke('reset_config'),
  exportConfig: (path: string): Promise<void> => invoke('export_config', { path }),
  importConfig: (path: string): Promise<Config> => invoke('import_config', { path }),
};

// ===== 连接命令 =====

export const connectionApi = {
  startConnection: (): Promise<string> => invoke('start_connection'),
  stopConnection: (): Promise<string> => invoke('stop_connection'),
  getConnectionStatus: (): Promise<ConnectionState> => invoke('get_connection_status'),
  getStatistics: (): Promise<ConnectionStats> => invoke('get_statistics'),
};

// ===== 屏幕命令 =====

export const screenApi = {
  getScreens: (): Promise<ScreenInfo[]> => invoke('get_screens'),
  captureScreenPreview: (screenIndex: number | null): Promise<string> =>
    invoke('capture_screen_preview', { screenIndex }),
};

// ===== 服务命令 =====

export const serviceApi = {
  install: (): Promise<string> => invoke('service_install'),
  uninstall: (): Promise<string> => invoke('service_uninstall'),
  start: (): Promise<string> => invoke('service_start'),
  stop: (): Promise<string> => invoke('service_stop'),
  status: (): Promise<ServiceStatus> => invoke('service_status'),
};

// ===== 事件监听 =====

export interface ConnectionStatusEvent {
  status: ConnectionState;
  error?: string;
}

export interface StatisticsUpdateEvent {
  status: ConnectionState;
  framesSent: number;
  fps: number;
  bytesSent: number;
  uptime: number;
  reconnectCount: number;
}

export const events = {
  onConnectionStatus: (callback: (event: ConnectionStatusEvent) => void): Promise<UnlistenFn> =>
    listen<ConnectionStatusEvent>('connection-status', (event) => callback(event.payload)),

  onStatisticsUpdate: (callback: (event: StatisticsUpdateEvent) => void): Promise<UnlistenFn> =>
    listen<StatisticsUpdateEvent>('statistics-update', (event) => callback(event.payload)),
};
