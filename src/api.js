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

export function searchProviderCandidates({ source, displayName, filename }) {
  return invoke('search_provider_candidates', {
    request: { source, displayName, filename }
  });
}

export function lookupProviderFingerprint({ source, displayName, filename }) {
  return invoke('lookup_provider_fingerprint', {
    request: { source, displayName, filename }
  });
}

export function switchModSource({ key, source, displayName, filename, projectId, slug, title, iconUrl }) {
  return invoke('switch_mod_source', {
    request: {
      key,
      source,
      displayName,
      filename,
      projectId: projectId ?? null,
      slug: slug ?? null,
      title: title ?? null,
      iconUrl: iconUrl ?? null
    }
  });
}

export function listProviderVersions({ key, source, projectId, filename }) {
  return invoke('list_provider_versions', {
    request: {
      key,
      source,
      projectId: projectId ?? null,
      filename
    }
  });
}

export function installProviderVersion({
  key,
  source,
  projectId,
  filename,
  versionId,
  fileId,
  downloadUrl,
  downloadFilename,
  versionNumber
}) {
  return invoke('install_provider_version', {
    request: {
      key,
      source,
      projectId,
      filename,
      versionId,
      fileId: fileId ?? null,
      downloadUrl: downloadUrl ?? null,
      downloadFilename: downloadFilename ?? null,
      versionNumber: versionNumber ?? null
    }
  });
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
