import { create } from 'zustand';
import type { Config, ConnectionState, ConnectionStats, ScreenInfo, ServiceStatus } from './api';
import * as api from './api';
import { events } from './api';

interface AppState {
  // 配置
  config: Config | null;
  configLoading: boolean;
  configError: string | null;

  // 连接状态
  connectionState: ConnectionState;
  connectionStats: ConnectionStats | null;

  // 屏幕
  screens: ScreenInfo[];
  screensLoading: boolean;

  // 服务状态
  serviceStatus: ServiceStatus | null;
  serviceStatusLoading: boolean;

  // 操作
  loadConfig: () => Promise<void>;
  updateConfig: (config: Config) => Promise<void>;
  resetConfig: () => Promise<void>;
  startConnection: () => Promise<void>;
  stopConnection: () => Promise<void>;
  loadScreens: () => Promise<void>;
  loadServiceStatus: () => Promise<void>;
  installService: () => Promise<void>;
  uninstallService: () => Promise<void>;
  startService: () => Promise<void>;
  stopService: () => Promise<void>;
}

export const useAppStore = create<AppState>((set, get) => ({
  // 初始状态
  config: null,
  configLoading: false,
  configError: null,
  connectionState: 'disconnected',
  connectionStats: null,
  screens: [],
  screensLoading: false,
  serviceStatus: null,
  serviceStatusLoading: false,

  // 加载配置
  loadConfig: async () => {
    set({ configLoading: true, configError: null });
    try {
      const config = await api.configApi.getConfig();
      set({ config, configLoading: false });
    } catch (error) {
      set({ configError: String(error), configLoading: false });
    }
  },

  // 更新配置
  updateConfig: async (config: Config) => {
    await api.configApi.updateConfig(config);
    set({ config });
  },

  // 重置配置
  resetConfig: async () => {
    const config = await api.configApi.resetConfig();
    set({ config });
  },

  // 启动连接
  startConnection: async () => {
    await api.connectionApi.startConnection();
    set({ connectionState: 'connecting' });
  },

  // 停止连接
  stopConnection: async () => {
    await api.connectionApi.stopConnection();
    set({ connectionState: 'disconnected', connectionStats: null });
  },

  // 加载屏幕列表
  loadScreens: async () => {
    set({ screensLoading: true });
    try {
      const screens = await api.screenApi.getScreens();
      set({ screens, screensLoading: false });
    } catch (error) {
      console.error('加载屏幕失败:', error);
      set({ screensLoading: false });
    }
  },

  // 加载服务状态
  loadServiceStatus: async () => {
    set({ serviceStatusLoading: true });
    try {
      const status = await api.serviceApi.status();
      set({ serviceStatus: status, serviceStatusLoading: false });
    } catch (error) {
      console.error('加载服务状态失败:', error);
      set({ serviceStatusLoading: false });
    }
  },

  // 安装服务
  installService: async () => {
    await api.serviceApi.install();
    get().loadServiceStatus();
  },

  // 卸载服务
  uninstallService: async () => {
    await api.serviceApi.uninstall();
    get().loadServiceStatus();
  },

  // 启动服务
  startService: async () => {
    await api.serviceApi.start();
    get().loadServiceStatus();
  },

  // 停止服务
  stopService: async () => {
    await api.serviceApi.stop();
    get().loadServiceStatus();
  },
}));

// 设置事件监听
export function setupEventListeners() {
  // 连接状态变化
  events.onConnectionStatus((event) => {
    useAppStore.setState({ connectionState: event.status });
  });

  // 统计信息更新
  events.onStatisticsUpdate((event) => {
    useAppStore.setState({
      connectionStats: {
        status: event.status,
        frames_sent: event.framesSent,
        fps: event.fps,
        bytes_sent: event.bytesSent,
        uptime: event.uptime,
        latency: 0,
        reconnect_count: event.reconnectCount,
      },
    });
  });
}
