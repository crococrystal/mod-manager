import { installProviderVersion } from '../api.js';
import { isInstalledVersion, projectIdFor } from '../features/mods/versionListUtils.js';
import { fetchProviderVersionsCached } from './providerVersionsCache.js';

export async function installAllModUpdates({
  candidates,
  modsByKey,
  cacheScope,
  onProgress,
  onEachInstalled,
  onCandidateResolved
}) {
  const modState = new Map(modsByKey);
  const errors = [];
  let done = 0;

  for (let index = 0; index < candidates.length; index += 1) {
    const candidate = candidates[index];
    const key = candidate.key ?? candidate.id;
    const mod = modState.get(key);
    if (!mod) continue;

    onProgress?.({ current: index + 1, total: candidates.length, title: mod.displayName });

    const projectId = projectIdFor(mod);
    if (!projectId) {
      errors.push(`${mod.displayName}: нет ID проекта у поставщика.`);
      continue;
    }

    try {
      const payload = await fetchProviderVersionsCached({
        cacheScope,
        key: mod.key,
        source: mod.source,
        projectId,
        filename: mod.filename
      });
      const latest = payload.versions?.find((version) => !isInstalledVersion(mod, version));
      if (!latest) {
        onCandidateResolved?.(key);
        continue;
      }

      const result = await installProviderVersion({
        key: mod.key,
        source: mod.source,
        projectId,
        filename: mod.filename,
        versionId: latest.id,
        fileId: latest.fileId ?? undefined,
        downloadUrl: latest.downloadUrl ?? undefined,
        downloadFilename: latest.filename,
        versionNumber: latest.versionNumber
      });
      modState.set(result.key, { ...mod, ...result });
      onEachInstalled?.(result);
      done += 1;
    } catch (err) {
      errors.push(`${mod.displayName}: ${String(err)}`);
    }
  }

  return { done, errors };
}
