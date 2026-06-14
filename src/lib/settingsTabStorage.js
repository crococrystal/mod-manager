const STORAGE_KEY = 'mod-manager.settingsTab';
const VALID_TABS = new Set(['general', 'server', 'data']);

export function readSettingsTab(fallback = 'general') {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    return VALID_TABS.has(value) ? value : fallback;
  } catch {
    return fallback;
  }
}

export function writeSettingsTab(tab) {
  if (!VALID_TABS.has(tab)) return;
  try {
    localStorage.setItem(STORAGE_KEY, tab);
  } catch {
    // ignore quota / private mode
  }
}
