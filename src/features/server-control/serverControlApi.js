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

export function checkServerControlStatus({ sshHost, serverRootPath } = {}) {
  return invoke('check_server_control_status', {
    request: buildRequest({ sshHost, serverRootPath })
  });
}

export function startServerControl({ sshHost, serverRootPath } = {}) {
  return invoke('start_server_control', {
    request: buildRequest({ sshHost, serverRootPath })
  });
}

export function stopServerControl({ sshHost, serverRootPath } = {}) {
  return invoke('stop_server_control', {
    request: buildRequest({ sshHost, serverRootPath })
  });
}

export function readServerLaunchScript({ sshHost, serverRootPath } = {}) {
  return invoke('read_server_launch_script', {
    request: buildRequest({ sshHost, serverRootPath })
  });
}

export function writeServerLaunchScript({ sshHost, serverRootPath, content } = {}) {
  return invoke('write_server_launch_script', {
    request: {
      ...(buildRequest({ sshHost, serverRootPath }) ?? {}),
      content: content ?? ''
    }
  });
}
