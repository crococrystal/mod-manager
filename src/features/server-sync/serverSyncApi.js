import { invoke } from '@tauri-apps/api/core';

export function testServerSync({ sshHost } = {}) {
  return invoke('test_server_sync', {
    request: sshHost ? { sshHost } : null
  });
}

export function previewServerSyncLane(lane) {
  return invoke('preview_server_sync_lane', { request: { lane } });
}

export function syncModsToServerLane(lane) {
  return invoke('sync_mods_to_server_lane', { request: { lane } });
}

export function getServerSyncStatuses() {
  return invoke('get_server_sync_statuses');
}

export function cancelServerSyncLane(lane) {
  return invoke('cancel_server_sync_lane', { request: { lane } });
}
