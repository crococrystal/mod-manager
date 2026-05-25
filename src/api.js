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

export function switchModSource({ key, source }) {
  return invoke('switch_mod_source', { request: { key, source } });
}

export function copyModFiles(keys) {
  return invoke('copy_mod_files', { keys });
}

export function uploadCover({ key, dataUrl }) {
  return invoke('upload_cover', { key, dataUrl });
}

export function deleteCustomCover(key) {
  return invoke('delete_custom_cover', { key });
}
