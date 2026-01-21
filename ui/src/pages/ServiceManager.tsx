import { useEffect, useState } from 'react';
import { useAppStore } from '../lib/store';

export default function ServiceManager() {
  const {
    serviceStatus,
    serviceStatusLoading,
    loadServiceStatus,
    installService,
    uninstallService,
    startService,
    stopService,
  } = useAppStore();
  const [actionLoading, setActionLoading] = useState<string | null>(null);

  useEffect(() => {
    loadServiceStatus();
    // 定期刷新状态
    const interval = setInterval(loadServiceStatus, 3000);
    return () => clearInterval(interval);
  }, [loadServiceStatus]);

  const handleInstall = async () => {
    setActionLoading('install');
    try {
      await installService();
    } catch (error) {
      console.error('安装服务失败:', error);
      alert('安装服务失败: ' + error);
    } finally {
      setActionLoading(null);
    }
  };

  const handleUninstall = async () => {
    if (!confirm('确定要卸载系统服务吗？')) return;

    setActionLoading('uninstall');
    try {
      await uninstallService();
    } catch (error) {
      console.error('卸载服务失败:', error);
      alert('卸载服务失败: ' + error);
    } finally {
      setActionLoading(null);
    }
  };

  const handleStart = async () => {
    setActionLoading('start');
    try {
      await startService();
    } catch (error) {
      console.error('启动服务失败:', error);
      alert('启动服务失败: ' + error);
    } finally {
      setActionLoading(null);
    }
  };

  const handleStop = async () => {
    setActionLoading('stop');
    try {
      await stopService();
    } catch (error) {
      console.error('停止服务失败:', error);
      alert('停止服务失败: ' + error);
    } finally {
      setActionLoading(null);
    }
  };

  const getStatusText = () => {
    if (!serviceStatus) return '未知';

    if (!serviceStatus.installed) {
      return '未安装';
    }

    switch (serviceStatus.status) {
      case 'running':
        return '运行中';
      case 'stopped':
        return '已停止';
      case 'unknown':
        return '未知状态';
      default:
        if (serviceStatus.status.startsWith('failed')) {
          return '运行失败';
        }
        if (serviceStatus.status.startsWith('error')) {
          return '状态错误';
        }
        return serviceStatus.status;
    }
  };

  const getStatusClass = () => {
    if (!serviceStatus) return 'disconnected';

    if (!serviceStatus.installed) {
      return 'disconnected';
    }

    switch (serviceStatus.status) {
      case 'running':
        return 'connected';
      case 'stopped':
        return 'disconnected';
      default:
        return 'disconnected';
    }
  };

  const isLoading = serviceStatusLoading || actionLoading !== null;

  return (
    <div>
      <div className="page-header">
        <h2>服务管理</h2>
        <p>管理系统服务安装和运行状态</p>
      </div>

      {/* Service Status Card */}
      <div className="service-status-card">
        <div className="service-status-icon">
          {isLoading ? (
            <div className="spinner" style={{ width: 24, height: 24 }} />
          ) : serviceStatus?.installed && serviceStatus.status === 'running' ? (
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
              <polyline points="22 4 12 14.01 9 11.01" />
            </svg>
          ) : (
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M22 12h-6l-2 3h-6l-2-3H2" />
              <path d="M5.45 5.11L2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z" />
            </svg>
          )}
        </div>
        <div className="service-status-info">
          <h3>系统服务</h3>
          <p>
            <span className={`status-badge status-${getStatusClass()}`}>
              {getStatusText()}
            </span>
          </p>
        </div>
        <div className="service-actions">
          {!serviceStatus?.installed ? (
            <button
              className="button button-primary"
              onClick={handleInstall}
              disabled={actionLoading === 'install'}
            >
              {actionLoading === 'install' ? '安装中...' : '安装服务'}
            </button>
          ) : (
            <>
              {serviceStatus.status === 'running' ? (
                <button
                  className="button button-danger"
                  onClick={handleStop}
                  disabled={actionLoading === 'stop'}
                >
                  {actionLoading === 'stop' ? '停止中...' : '停止服务'}
                </button>
              ) : (
                <button
                  className="button button-success"
                  onClick={handleStart}
                  disabled={actionLoading === 'start'}
                >
                  {actionLoading === 'start' ? '启动中...' : '启动服务'}
                </button>
              )}
              <button
                className="button"
                onClick={handleUninstall}
                disabled={actionLoading === 'uninstall'}
              >
                {actionLoading === 'uninstall' ? '卸载中...' : '卸载服务'}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Information Cards */}
      <div className="grid grid-2">
        <div className="card">
          <div className="card-header">
            <h3>关于系统服务</h3>
          </div>
          <div className="card-body">
            <p style={{ marginBottom: 12 }}>
              将 SSControl 安装为系统服务后，可以在系统启动时自动运行，无需用户登录。
            </p>
            <ul style={{ paddingLeft: 20, lineHeight: 1.8 }}>
              <li>开机自动启动</li>
              <li>无需用户登录</li>
              <li>后台稳定运行</li>
              <li>系统级别权限</li>
            </ul>
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <h3>平台信息</h3>
          </div>
          <div className="card-body">
            <table style={{ width: '100%' }}>
              <tbody>
                <tr>
                  <td style={{ padding: '8px 0', color: 'var(--text-secondary)' }}>平台:</td>
                  <td style={{ padding: '8px 0', textAlign: 'right' }}>
                    {navigator.platform.includes('Mac')
                      ? 'macOS'
                      : navigator.platform.includes('Win')
                        ? 'Windows'
                        : 'Linux'}
                  </td>
                </tr>
                <tr>
                  <td style={{ padding: '8px 0', color: 'var(--text-secondary)' }}>服务类型:</td>
                  <td style={{ padding: '8px 0', textAlign: 'right' }}>
                    {navigator.platform.includes('Mac')
                      ? 'LaunchAgent'
                      : navigator.platform.includes('Win')
                        ? 'Windows Service'
                        : 'systemd'}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>

      {/* Permissions Notice */}
      <div className="card" style={{ marginTop: 16 }}>
        <div className="card-header">
          <h3>权限说明</h3>
        </div>
        <div className="card-body">
          <p style={{ marginBottom: 12 }}>
            <strong>macOS 用户注意:</strong> 服务模式需要授予以下权限:
          </p>
          <ul style={{ paddingLeft: 20, lineHeight: 1.8 }}>
            <li><strong>屏幕录制</strong>: 系统设置 → 隐私与安全性 → 屏幕录制</li>
            <li><strong>辅助功能</strong>: 系统设置 → 隐私与安全性 → 辅助功能</li>
          </ul>
          <p style={{ marginTop: 12, color: 'var(--text-secondary)' }}>
            安装服务后，请确保在系统设置中授予相应权限。
          </p>
        </div>
      </div>
    </div>
  );
}
