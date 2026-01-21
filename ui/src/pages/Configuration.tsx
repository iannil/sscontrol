import { useEffect, useState } from 'react';
import { useAppStore } from '../lib/store';
import type { Config } from '../lib/api';

export default function Configuration() {
  const { config, configLoading, loadConfig, updateConfig, resetConfig } = useAppStore();
  const [editingConfig, setEditingConfig] = useState<Config | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (config && !editingConfig) {
      setEditingConfig(JSON.parse(JSON.stringify(config)));
    }
  }, [config]);

  const handleSave = async () => {
    if (!editingConfig) return;

    setSaving(true);
    setSaved(false);
    try {
      await updateConfig(editingConfig);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (error) {
      console.error('保存配置失败:', error);
    } finally {
      setSaving(false);
    }
  };

  const handleReset = async () => {
    if (!confirm('确定要重置为默认配置吗？')) return;

    try {
      await resetConfig();
    } catch (error) {
      console.error('重置配置失败:', error);
    }
  };

  if (configLoading || !editingConfig) {
    return (
      <div className="loading">
        <div className="spinner" />
      </div>
    );
  }

  const updateServer = (field: keyof typeof editingConfig.server, value: string) => {
    setEditingConfig({
      ...editingConfig,
      server: { ...editingConfig.server, [field]: value },
    });
  };

  const updateCapture = (field: keyof typeof editingConfig.capture, value: string | number | null) => {
    setEditingConfig({
      ...editingConfig,
      capture: { ...editingConfig.capture, [field]: value },
    });
  };

  const updateLogging = (field: keyof typeof editingConfig.logging, value: string | null) => {
    setEditingConfig({
      ...editingConfig,
      logging: { ...editingConfig.logging, [field]: value },
    });
  };

  const updateSecurity = (field: keyof typeof editingConfig.security, value: string | number | boolean | null) => {
    setEditingConfig({
      ...editingConfig,
      security: { ...editingConfig.security, [field]: value },
    });
  };

  return (
    <div>
      <div className="page-header">
        <h2>配置管理</h2>
        <p>管理应用程序配置</p>
      </div>

      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginBottom: 16 }}>
        <button className="button" onClick={handleReset}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
          </svg>
          重置默认
        </button>
        <button
          className="button button-primary"
          onClick={handleSave}
          disabled={saving}
        >
          {saved ? (
            <>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <polyline points="20 6 9 17 4 12" />
              </svg>
              已保存
            </>
          ) : (
            <>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
                <polyline points="17 21 17 13 7 13 7 21" />
                <polyline points="7 3 7 8 15 8" />
              </svg>
              {saving ? '保存中...' : '保存配置'}
            </>
          )}
        </button>
      </div>

      {/* Server Configuration */}
      <div className="card">
        <div className="card-header">
          <h3>服务器配置</h3>
        </div>
        <div className="form-section">
          <div className="input-group">
            <label>WebSocket 服务器地址</label>
            <input
              type="text"
              className="input"
              value={editingConfig.server.url}
              onChange={(e) => updateServer('url', e.target.value)}
              placeholder="ws://localhost:8080"
            />
          </div>
          <div className="input-group">
            <label>设备 ID</label>
            <input
              type="text"
              className="input"
              value={editingConfig.server.device_id}
              onChange={(e) => updateServer('device_id', e.target.value)}
              placeholder="自动生成的 UUID"
            />
          </div>
        </div>
      </div>

      {/* Capture Configuration */}
      <div className="card">
        <div className="card-header">
          <h3>屏幕捕获配置</h3>
        </div>
        <div className="form-row">
          <div className="input-group">
            <label>目标帧率 (FPS)</label>
            <input
              type="number"
              className="input"
              value={editingConfig.capture.fps}
              onChange={(e) => updateCapture('fps', parseInt(e.target.value) || 30)}
              min="1"
              max="60"
            />
          </div>
          <div className="input-group">
            <label>屏幕索引 (空 = 主显示器)</label>
            <input
              type="number"
              className="input"
              value={editingConfig.capture.screen_index ?? ''}
              onChange={(e) => updateCapture('screen_index', e.target.value ? parseInt(e.target.value) : null)}
              placeholder="留空使用主显示器"
            />
          </div>
        </div>
        <div className="form-row">
          <div className="input-group">
            <label>捕获宽度 (空 = 原始宽度)</label>
            <input
              type="number"
              className="input"
              value={editingConfig.capture.width ?? ''}
              onChange={(e) => updateCapture('width', e.target.value ? parseInt(e.target.value) : null)}
              placeholder="留空使用原始宽度"
            />
          </div>
          <div className="input-group">
            <label>捕获高度 (空 = 原始高度)</label>
            <input
              type="number"
              className="input"
              value={editingConfig.capture.height ?? ''}
              onChange={(e) => updateCapture('height', e.target.value ? parseInt(e.target.value) : null)}
              placeholder="留空使用原始高度"
            />
          </div>
        </div>
      </div>

      {/* Logging Configuration */}
      <div className="card">
        <div className="card-header">
          <h3>日志配置</h3>
        </div>
        <div className="form-row">
          <div className="input-group">
            <label>日志级别</label>
            <select
              className="input"
              value={editingConfig.logging.level}
              onChange={(e) => updateLogging('level', e.target.value)}
            >
              <option value="trace">Trace</option>
              <option value="debug">Debug</option>
              <option value="info">Info</option>
              <option value="warn">Warn</option>
              <option value="error">Error</option>
            </select>
          </div>
          <div className="input-group">
            <label>日志文件路径 (空 = 仅控制台)</label>
            <input
              type="text"
              className="input"
              value={editingConfig.logging.file ?? ''}
              onChange={(e) => updateLogging('file', e.target.value || null)}
              placeholder="留空仅输出到控制台"
            />
          </div>
        </div>
      </div>

      {/* Security Configuration */}
      <div className="card">
        <div className="card-header">
          <h3>安全配置</h3>
        </div>
        <div className="form-row">
          <div className="input-group">
            <label>API Key</label>
            <input
              type="password"
              className="input"
              value={editingConfig.security.api_key ?? ''}
              onChange={(e) => updateSecurity('api_key', e.target.value || null)}
              placeholder="可选，用于服务器认证"
            />
          </div>
          <div className="input-group">
            <label>Token 有效期 (秒)</label>
            <input
              type="number"
              className="input"
              value={editingConfig.security.token_ttl}
              onChange={(e) => updateSecurity('token_ttl', parseInt(e.target.value) || 300)}
              min="60"
            />
          </div>
        </div>
        <div className="form-row">
          <div className="input-group">
            <label>TLS 证书路径</label>
            <input
              type="text"
              className="input"
              value={editingConfig.security.tls_cert ?? ''}
              onChange={(e) => updateSecurity('tls_cert', e.target.value || null)}
              placeholder="可选，用于 TLS 连接"
            />
          </div>
          <div className="input-group">
            <label>TLS 私钥路径</label>
            <input
              type="text"
              className="input"
              value={editingConfig.security.tls_key ?? ''}
              onChange={(e) => updateSecurity('tls_key', e.target.value || null)}
              placeholder="可选，用于 TLS 连接"
            />
          </div>
        </div>
        <div className="checkbox-group">
          <input
            type="checkbox"
            id="require_tls"
            checked={editingConfig.security.require_tls}
            onChange={(e) => updateSecurity('require_tls', e.target.checked)}
          />
          <label htmlFor="require_tls">强制使用 TLS</label>
        </div>
      </div>
    </div>
  );
}
