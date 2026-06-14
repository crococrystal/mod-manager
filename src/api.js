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

export function identifyModSources() {
  return invoke('identify_mod_sources');
}

export function refreshModAssets(key) {
  return invoke('refresh_mod_assets', { request: { key } });
}

export function refreshProviderLabels(key) {
  return invoke('refresh_provider_labels', { request: { key } });
}

export function bootstrapInstance(force = false) {
  return invoke('bootstrap_instance', { force });
}

export function cancelBackgroundTask() {
  return invoke('cancel_background_task');
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

export function checkModUpdates({ forceRefresh = false } = {}) {
  return invoke('check_mod_updates', {
    request: { forceRefresh }
  });
}

export function searchProviderCatalog({ source, query, offset = 0 }) {
  return invoke('search_provider_catalog', {
    request: { source, query, offset }
  });
}

export function previewCatalogInstall({ source, projectId, versionId, forceRefresh = false }) {
  return invoke('preview_catalog_install', {
    request: {
      source,
      projectId,
      versionId: versionId ?? null,
      forceRefresh
    }
  });
}

export function catalogProjectDetails({ source, projectId, forceRefresh = false }) {
  return invoke('catalog_project_details', {
    request: { source, projectId, forceRefresh }
  });
}

export function installFromCatalog({ source, projectId, versionId }) {
  return invoke('install_from_catalog', {
    request: { source, projectId, versionId: versionId ?? null }
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

export function deleteModFiles(keys) {
  return invoke('delete_mod_files', { keys });
}

export function disableModFiles(keys) {
  return invoke('disable_mod_files', { keys });
}

export function enableModFiles(keys) {
  return invoke('enable_mod_files', { keys });
}

export function uploadCover({ key, dataUrl }) {
  return invoke('upload_cover', { key, dataUrl });
}

export function deleteCustomCover(key) {
  return invoke('delete_custom_cover', { key });
}

export function syncProviderData({ identify = false, labels = false, assets = false }) {
  return invoke('sync_provider_data', { request: { identify, labels, assets } });
}

export function getDataUsage() {
  return invoke('get_data_usage');
}
