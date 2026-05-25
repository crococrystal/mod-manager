import { useEffect, useState } from 'react';
import { Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';

const platform = typeof navigator !== 'undefined' ? navigator.platform : '';
const isMac = /Mac|iPhone|iPod|iPad/i.test(platform);
const isWindows = /Win/i.test(platform);
const isLinux = /Linux/i.test(platform);

function isPreviewWindowsChrome() {
  if (!import.meta.env.DEV) return false;
  const v = String(import.meta.env.VITE_PREVIEW_WINDOWS_CHROME ?? '');
  return v === 'true' || v === '1';
}

const previewWindowsChrome = isPreviewWindowsChrome();
const useCustomChrome = isWindows || isLinux || previewWindowsChrome;

async function applyCustomChrome() {
  const win = getCurrentWindow();
  if (previewWindowsChrome && isMac) {
    await win.setTitleBarStyle('visible').catch(() => {});
  }
  await win.setDecorations(false).catch(() => {});
}

function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten;
    win.isMaximized().then(setMaximized).catch(() => {});
    win.onResized(() => {
      win.isMaximized().then(setMaximized).catch(() => {});
    }).then((fn) => {
      unlisten = fn;
    }).catch(() => {});
    return () => {
      unlisten?.();
    };
  }, []);

  const win = getCurrentWindow();

  return (
    <div className="winControls" data-tauri-drag-region="false">
      <button
        type="button"
        className="winControl"
        aria-label="Свернуть"
        onClick={() => void win.minimize().catch(() => {})}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        className="winControl"
        aria-label={maximized ? 'Восстановить' : 'Развернуть'}
        onClick={() => void win.toggleMaximize().catch(() => {})}
      >
        <Square size={12} />
      </button>
      <button
        type="button"
        className="winControl winControlClose"
        aria-label="Закрыть"
        onClick={() => void win.close().catch(() => {})}
      >
        <X size={14} />
      </button>
    </div>
  );
}

export function TitleBar({ children }) {
  useEffect(() => {
    if (!useCustomChrome) return;
    void applyCustomChrome();
  }, []);

  const titleBarClass = useCustomChrome ? 'titleBarWindows' : isMac ? 'titleBarMac' : 'titleBarOther';

  return (
    <header className={`titleBar ${titleBarClass}`} data-tauri-drag-region>
      <div className="titleBarSpacer" data-tauri-drag-region aria-hidden="true" />
      <div className="titleBarContent" data-tauri-drag-region>
        {children}
      </div>
      {useCustomChrome ? <WindowControls /> : null}
    </header>
  );
}
