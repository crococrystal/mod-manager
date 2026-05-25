import { isTauri } from '@tauri-apps/api/core';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export function canCheckForUpdates() {
  return isTauri();
}

export function formatUpdateError(err) {
  const message = String(err);
  if (message.includes('404')) {
    return 'Не удалось скачать обновление (файл не найден на сервере). Попробуй позже или скачай dmg вручную.';
  }
  return message;
}

export async function checkForAppUpdate() {
  if (!canCheckForUpdates()) return null;
  return check();
}

export async function installAppUpdate(update, onProgress) {
  if (!update) return;

  let downloaded = 0;
  let total = 0;

  await update.downloadAndInstall((event) => {
    if (event.event === 'Started') {
      downloaded = 0;
      total = event.data.contentLength ?? 0;
      onProgress?.({ phase: 'download', downloaded, total, percent: total ? 0 : null });
      return;
    }
    if (event.event === 'Progress') {
      downloaded += event.data.chunkLength;
      const percent = total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
      onProgress?.({ phase: 'download', downloaded, total, percent });
      return;
    }
    if (event.event === 'Finished') {
      onProgress?.({ phase: 'install', downloaded, total, percent: 100 });
    }
  });

  await relaunch();
}
