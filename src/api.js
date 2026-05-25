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

export function updateModTags(patch) {
  return invoke('update_mod_tags', { patch });
}
