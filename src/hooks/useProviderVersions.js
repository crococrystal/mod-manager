import { useEffect, useRef, useState } from 'react';
import {
  fetchProviderVersionsCached,
  readProviderVersionsCache
} from '../lib/providerVersionsCache.js';
import { projectIdFor } from '../features/mods/versionListUtils.js';

function cachedVersionsPayload(mod, cacheScope) {
  if (!mod) return null;
  const projectId = projectIdFor(mod);
  if (!projectId) return null;
  return readProviderVersionsCache(cacheScope, mod.source, projectId) ?? null;
}

export function useProviderVersions({ mod, cacheScope }) {
  const [payload, setPayload] = useState(() => cachedVersionsPayload(mod, cacheScope));
  const [loading, setLoading] = useState(
    () => Boolean(mod && projectIdFor(mod) && !cachedVersionsPayload(mod, cacheScope))
  );
  const [error, setError] = useState('');
  const runRef = useRef(0);

  useEffect(() => {
    if (!mod) {
      runRef.current += 1;
      setPayload(null);
      setLoading(false);
      setError('');
      return;
    }

    const projectId = projectIdFor(mod);
    const runId = runRef.current + 1;
    runRef.current = runId;
    setError('');

    if (!projectId) {
      setPayload(null);
      setError('Сначала выбери проект мода у поставщика.');
      setLoading(false);
      return;
    }

    const cached = readProviderVersionsCache(cacheScope, mod.source, projectId);
    if (cached) {
      setPayload(cached);
      setLoading(false);
      return;
    }

    setPayload(null);
    setLoading(true);
    void fetchProviderVersionsCached({
      cacheScope,
      key: mod.key,
      source: mod.source,
      projectId,
      filename: mod.filename
    })
      .then((next) => {
        if (runRef.current !== runId) return;
        setPayload(next);
      })
      .catch((err) => {
        if (runRef.current !== runId) return;
        setError(String(err));
      })
      .finally(() => {
        if (runRef.current !== runId) return;
        setLoading(false);
      });
  }, [mod?.key, mod?.source, cacheScope]);

  return {
    payload,
    loading,
    error,
    target: payload?.target ?? null,
    versions: payload?.versions ?? []
  };
}
