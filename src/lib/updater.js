import { isTauri } from '@tauri-apps/api/core';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export function canCheckForUpdates() {
  return isTauri();
}

export async function checkForAppUpdate() {
  if (!canCheckForUpdates()) return null;
  return check();
}

export async function installAppUpdate(update, onProgress) {
  if (!update) return;
  await update.downloadAndInstall((event) => {
    onProgress?.(event);
  });
  await relaunch();
}
