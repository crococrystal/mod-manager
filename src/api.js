import { invoke } from '@tauri-apps/api/core';

export function getSettings() {
  return invoke('get_settings');
}

export function saveSettings(settings) {
  return invoke('save_settings', { settings });
}

export function scanMods() {
  return invoke('scan_mods');
}

export function bootstrapInstance(force = false) {
  return invoke('bootstrap_instance', { force });
}

export function clearAppData() {
  return invoke('clear_app_data');
}

export function updateModTags(patch) {
  return invoke('update_mod_tags', { patch });
}

export function uploadCover({ key, dataUrl }) {
  return invoke('upload_cover', { key, dataUrl });
}
