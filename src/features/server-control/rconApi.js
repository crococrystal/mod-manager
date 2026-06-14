import { invoke } from '@tauri-apps/api/core';

function buildRequest({ sshHost, serverRootPath } = {}) {
  const request = {};
  if (sshHost?.trim()) {
    request.sshHost = sshHost.trim();
  }
  if (serverRootPath?.trim()) {
    request.serverRootPath = serverRootPath.trim();
  }
  return Object.keys(request).length ? request : null;
}

export function checkServerRcon({ sshHost, serverRootPath } = {}) {
  return invoke('check_server_rcon', {
    request: buildRequest({ sshHost, serverRootPath })
  });
}

export function sendServerRconCommand({ sshHost, serverRootPath, command } = {}) {
  return invoke('send_server_rcon_command', {
    request: {
      ...(buildRequest({ sshHost, serverRootPath }) ?? {}),
      command: command ?? ''
    }
  });
}
