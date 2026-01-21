import { useEffect, useState } from 'react';
import { useAppStore } from '../lib/store';
import type { ScreenInfo } from '../lib/api';

function ScreenCard({
  screen,
  selected,
  onClick,
}: {
  screen: ScreenInfo;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <div
      className={`screen-card ${selected ? 'selected' : ''}`}
      onClick={onClick}
    >
      <div className="screen-preview">
        <svg
          width="48"
          height="48"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1"
        >
          <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
          <line x1="8" y1="21" x2="16" y2="21" />
          <line x1="12" y1="17" x2="12" y2="21" />
        </svg>
      </div>
      <div className="screen-info">
        <div>
          <div className="screen-name">
            {screen.name}
            {screen.is_primary && <span className="screen-badge">主屏幕</span>}
          </div>
          <div className="screen-resolution">
            {screen.width} × {screen.height}
            {screen.scale_factor !== 1 && ` (${screen.scale_factor}x)`}
          </div>
        </div>
        {selected && (
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            style={{ color: 'var(--accent)' }}
          >
            <polyline points="20 6 9 17 4 12" />
          </svg>
        )}
      </div>
    </div>
  );
}

export default function ScreenSelect() {
  const { screens, screensLoading, loadScreens, config, updateConfig } = useAppStore();
  const [selectedScreen, setSelectedScreen] = useState<number | null | undefined>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    loadScreens();
  }, [loadScreens]);

  useEffect(() => {
    if (config) {
      setSelectedScreen(config.capture.screen_index);
    }
  }, [config]);

  const handleScreenSelect = async (index: number | null) => {
    if (!config) return;

    setSelectedScreen(index);
    setSaving(true);

    try {
      const newConfig = {
        ...config,
        capture: {
          ...config.capture,
          screen_index: index,
        },
      };
      await updateConfig(newConfig);
    } catch (error) {
      console.error('选择屏幕失败:', error);
      // 恢复原来的选择
      setSelectedScreen(config.capture.screen_index);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div>
      <div className="page-header">
        <h2>屏幕选择</h2>
        <p>选择要共享的屏幕</p>
      </div>

      {screensLoading ? (
        <div className="loading">
          <div className="spinner" />
        </div>
      ) : screens.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state-icon">🖥️</div>
          <div className="empty-state-title">未检测到屏幕</div>
          <p>请确保您的系统已连接显示器</p>
        </div>
      ) : (
        <>
          {saving && (
            <div className="card" style={{ marginBottom: 16 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <div className="spinner" style={{ width: 16, height: 16 }} />
                <span>正在保存配置...</span>
              </div>
            </div>
          )}

          <div className="screen-list">
            {/* 主屏幕选项 */}
            <ScreenCard
              screen={{
                index: -1,
                name: '主屏幕 (默认)',
                width: 1920,
                height: 1080,
                is_primary: true,
                scale_factor: 1,
              }}
              selected={selectedScreen === null}
              onClick={() => handleScreenSelect(null)}
            />
            {screens.map((screen) => (
              <ScreenCard
                key={screen.index}
                screen={screen}
                selected={selectedScreen === screen.index}
                onClick={() => handleScreenSelect(screen.index)}
              />
            ))}
          </div>

          <div className="card" style={{ marginTop: 16 }}>
            <div className="card-header">
              <h3>当前选择</h3>
            </div>
            <div className="card-body">
              {selectedScreen === null ? (
                <p>主屏幕 (默认)</p>
              ) : (
                <p>
                  屏幕 {selectedScreen}{' '}
                  {screens.find((s) => s.index === selectedScreen)?.is_primary && '(主屏幕)'}
                </p>
              )}
              {config && (
                <p style={{ marginTop: 8, fontSize: 13 }}>
                  捕获分辨率:{' '}
                  {config.capture.width || config.capture.height
                    ? `${config.capture.width || '原始'} × ${config.capture.height || '原始'}`
                    : '原始分辨率'}
                  <br />
                  目标帧率: {config.capture.fps} FPS
                </p>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
