export const defaultServerSync = () => ({
  enabled: false,
  sshHost: '',
  serverModsPath: '',
  distributionModsPath: '',
  deleteExtraRemoteJars: true,
  serverOs: 'auto',
  serverStartScript: '',
  serverRootPath: ''
});

export function deriveServerRootPath(serverModsPath) {
  let path = String(serverModsPath ?? '').trim().replace(/\\/g, '/');
  while (path.endsWith('/')) {
    path = path.slice(0, -1);
  }
  if (!path) return '';
  if (path.toLowerCase().endsWith('/mods')) {
    return path.slice(0, -5).replace(/\/$/, '');
  }
  return path;
}

export function normalizeSshHost(value) {
  return String(value ?? '').trim().toLowerCase();
}

export function normalizeRemotePath(value) {
  let text = String(value ?? '').trim();
  while (text.length >= 2) {
    const quote = text[0];
    if ((quote === '"' || quote === "'") && text.at(-1) === quote) {
      text = text.slice(1, -1).trim();
    } else {
      break;
    }
  }
  return text.replace(/\\/g, '/');
}

export function normalizeServerSyncDraft(serverSync) {
  const serverModsPath = normalizeRemotePath(serverSync?.serverModsPath);
  const serverRootPath = normalizeRemotePath(serverSync?.serverRootPath);
  return {
    ...defaultServerSync(),
    ...serverSync,
    sshHost: normalizeSshHost(serverSync?.sshHost),
    serverModsPath,
    distributionModsPath: normalizeRemotePath(serverSync?.distributionModsPath),
    serverRootPath: serverRootPath || deriveServerRootPath(serverModsPath),
    serverStartScript: String(serverSync?.serverStartScript ?? '').trim()
  };
}

export function serverSyncFromSettings(settings) {
  return normalizeServerSyncDraft(settings?.serverSync ?? {});
}

export function withServerSync(settings, serverSync) {
  return {
    instanceRoot: settings?.instanceRoot ?? null,
    curseforgeApiKey: settings?.curseforgeApiKey ?? '',
    autoPrefetchCovers: settings?.autoPrefetchCovers ?? true,
    autoPrefetchDependencies: settings?.autoPrefetchDependencies ?? true,
    autoCheckUpdates: settings?.autoCheckUpdates ?? true,
    recentInstances: settings?.recentInstances ?? [],
    serverSync: normalizeServerSyncDraft(serverSync)
  };
}

export function appendServerSyncFields(settingsPayload, draft) {
  return {
    ...settingsPayload,
    serverSync: serverSyncFromSettings(draft)
  };
}
