import { invoke } from '@tauri-apps/api/core';

export function checkNeoForgeUpdate({ sshHost } = {}) {
  return invoke('check_neoforge_update', {
    request: sshHost ? { sshHost } : null
  });
}

export function getNeoForgeVersionCatalog() {
  return invoke('get_neoforge_version_catalog');
}

export function refreshNeoForgeRow({ row, sshHost } = {}) {
  return invoke('refresh_neoforge_row', {
    request: {
      row,
      sshHost: sshHost || null
    }
  });
}

export function applyNeoForgeUpdate({
  targetVersion,
  updateClient = false,
  updateServer = false,
  sshHost
} = {}) {
  return invoke('apply_neoforge_update', {
    request: {
      targetVersion: targetVersion || null,
      updateClient,
      updateServer,
      sshHost: sshHost || null
    }
  });
}
