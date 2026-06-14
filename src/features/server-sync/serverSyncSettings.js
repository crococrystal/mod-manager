export const defaultServerSync = () => ({
  enabled: false,
  sshHost: '',
  serverModsPath: '',
  distributionModsPath: '',
  deleteExtraRemoteJars: true
});

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
  return {
    ...defaultServerSync(),
    ...serverSync,
    sshHost: normalizeSshHost(serverSync?.sshHost),
    serverModsPath: normalizeRemotePath(serverSync?.serverModsPath),
    distributionModsPath: normalizeRemotePath(serverSync?.distributionModsPath)
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
