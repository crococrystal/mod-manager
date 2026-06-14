import { listProviderVersions } from '../api.js';

const MAX_VERSIONS_CACHE_ENTRIES = 120;
const versionsCache = new Map();

function versionsCacheKey(scope, source, projectId) {
  return [scope ?? 'default', source, projectId ?? ''].join('\u0000');
}

function writeVersionsCache(key, payload) {
  versionsCache.delete(key);
  versionsCache.set(key, payload);
  while (versionsCache.size > MAX_VERSIONS_CACHE_ENTRIES) {
    const oldest = versionsCache.keys().next().value;
    versionsCache.delete(oldest);
  }
}

export function readProviderVersionsCache(scope, source, projectId) {
  return versionsCache.get(versionsCacheKey(scope, source, projectId));
}

export function invalidateProviderVersionsCache(scope, source, projectId) {
  versionsCache.delete(versionsCacheKey(scope, source, projectId));
}

export async function fetchProviderVersionsCached({
  cacheScope,
  source,
  projectId,
  forceRefresh = false,
  ...rest
}) {
  const key = versionsCacheKey(cacheScope, source, projectId);
  if (!forceRefresh) {
    const cached = versionsCache.get(key);
    if (cached) return cached;
  }

  const payload = await listProviderVersions({
    source,
    projectId,
    ...rest
  });
  writeVersionsCache(key, payload);
  return payload;
}
