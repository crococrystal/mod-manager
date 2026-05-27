import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

async function syncNativeShadow(win, maximized) {
  const enabled = !maximized;
  await win.setShadow(enabled).catch(() => {});
  if (enabled) {
    await invoke('refresh_window_shadow').catch(() => {});
  }
}

export function WindowFrame({ children }) {
  const [maximized, setMaximized] = useState(false);

  const refreshShadow = useCallback(async (win) => {
    const max = await win.isMaximized().catch(() => false);
    setMaximized(max);
    await syncNativeShadow(win, max);
  }, []);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten;
    const retryId = window.setTimeout(() => void refreshShadow(win), 120);

    void refreshShadow(win);

    win
      .onResized(() => {
        void refreshShadow(win);
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});

    return () => {
      window.clearTimeout(retryId);
      unlisten?.();
    };
  }, [refreshShadow]);

  const className = maximized ? 'windowFrame isMaximized' : 'windowFrame';

  return (
    <div className={className}>
      {children}
      <div id="app-modal-root" className="appModalRoot" />
    </div>
  );
}
