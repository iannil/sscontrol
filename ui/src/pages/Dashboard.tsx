import { useEffect, useState } from 'react';
import { useAppStore } from '../lib/store';
import type { ConnectionState } from '../lib/api';

function StatCard({ label, value, unit }: { label: string; value: number | string; unit?: string }) {
  return (
    <div className="stat-card">
      <div className="stat-card-label">{label}</div>
      <div className="stat-card-value">
        {value}
        {unit && <span className="stat-card-unit">{unit}</span>}
      </div>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
}

function formatUptime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m ${secs}s`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  } else {
    return `${secs}s`;
  }
}

export default function Dashboard() {
  const { connectionState, connectionStats, startConnection, stopConnection, loadConfig } = useAppStore();
  const [isStarting, setIsStarting] = useState(false);
  const [isStopping, setIsStopping] = useState(false);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  const statusText: Record<ConnectionState, string> = {
    disconnected: '未连接',
    connecting: '连接中...',
    connected: '已连接',
    reconnecting: '重连中...',
  };

  const statusClass: Record<ConnectionState, string> = {
    disconnected: 'disconnected',
    connecting: 'connecting',
    connected: 'connected',
    reconnecting: 'connecting',
  };

  const handleStart = async () => {
    setIsStarting(true);
    try {
      await startConnection();
    } catch (error) {
      console.error('启动连接失败:', error);
    } finally {
      setIsStarting(false);
    }
  };

  const handleStop = async () => {
    setIsStopping(true);
    try {
      await stopConnection();
    } catch (error) {
      console.error('停止连接失败:', error);
    } finally {
      setIsStopping(false);
    }
  };

  return (
    <div>
      <div className="page-header">
        <h2>主控面板</h2>
        <p>监控和管理远程桌面连接</p>
      </div>

      {/* Connection Status Card */}
      <div className="card">
        <div className="card-header">
          <h3>连接状态</h3>
          <div className={`status-badge status-${statusClass[connectionState]}`}>
            {statusText[connectionState]}
          </div>
        </div>
        <div className="card-body">
          <div className="connection-indicator">
            <div className={`connection-indicator-dot ${statusClass[connectionState]}`} />
            <span>
              {connectionState === 'connected'
                ? '正在传输屏幕数据'
                : connectionState === 'connecting'
                  ? '正在建立连接...'
                  : connectionState === 'reconnecting'
                    ? '正在重新连接...'
                    : '未连接到服务器'}
            </span>
          </div>
        </div>
        <div style={{ marginTop: 16, display: 'flex', gap: 8 }}>
          {connectionState === 'connected' || connectionState === 'connecting' || connectionState === 'reconnecting' ? (
            <button
              className="button button-danger"
              onClick={handleStop}
              disabled={isStopping}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="6" y="6" width="12" height="12" />
              </svg>
              停止连接
            </button>
          ) : (
            <button
              className="button button-primary"
              onClick={handleStart}
              disabled={isStarting}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
              启动连接
            </button>
          )}
          <button
            className="button"
            onClick={() => window.location.href = '/configuration'}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="3" />
              <path d="M12 1v6m0 6v6M1 12h6m6 0h6" />
            </svg>
            配置设置
          </button>
        </div>
      </div>

      {/* Statistics Grid */}
      <div className="grid grid-3">
        <StatCard
          label="帧率"
          value={connectionStats?.fps.toFixed(1) || '0.0'}
          unit="FPS"
        />
        <StatCard
          label="已发送帧数"
          value={connectionStats?.frames_sent.toLocaleString() || '0'}
        />
        <StatCard
          label="传输数据量"
          value={connectionStats ? formatBytes(connectionStats.bytes_sent) : '0 B'}
        />
        <StatCard
          label="运行时间"
          value={connectionStats ? formatUptime(connectionStats.uptime) : '0s'}
        />
        <StatCard
          label="重连次数"
          value={connectionStats?.reconnect_count || 0}
          unit="次"
        />
        <StatCard
          label="状态"
          value={statusText[connectionState]}
        />
      </div>

      {/* Quick Actions */}
      <div className="card" style={{ marginTop: 16 }}>
        <div className="card-header">
          <h3>快捷操作</h3>
        </div>
        <div className="card-body">
          <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            <button className="button" onClick={() => window.location.href = '/screens'}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
                <line x1="8" y1="21" x2="16" y2="21" />
                <line x1="12" y1="17" x2="12" y2="21" />
              </svg>
              选择屏幕
            </button>
            <button className="button" onClick={() => window.location.href = '/configuration'}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M12 20h9" />
                <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
              </svg>
              编辑配置
            </button>
            <button className="button" onClick={() => window.location.href = '/service'}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M22 12h-6l-2 3h-6l-2-3H2" />
                <path d="M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z" />
              </svg>
              服务管理
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
