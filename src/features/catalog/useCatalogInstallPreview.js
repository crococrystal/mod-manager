import { useCallback, useEffect, useRef, useState } from 'react';
import { previewCatalogInstall } from '../../api.js';

const MAX_PREVIEW_CACHE_ENTRIES = 120;
const previewCache = new Map();

function previewCacheKey(scope, source, candidate, installedStateKey) {
  return [scope ?? 'default', source, candidate?.id ?? '', installedStateKey ?? ''].join('\u0000');
}

function writePreviewCache(key, preview) {
  previewCache.delete(key);
  previewCache.set(key, preview);
  while (previewCache.size > MAX_PREVIEW_CACHE_ENTRIES) {
    const oldest = previewCache.keys().next().value;
    previewCache.delete(oldest);
  }
}

export function useCatalogInstallPreview({ candidate, source, cacheScope, installedStateKey }) {
  const [preview, setPreview] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const runRef = useRef(0);

  const load = useCallback(
    async (forceRefresh = false) => {
      if (!candidate || !source) {
        runRef.current += 1;
        setPreview(null);
        setLoading(false);
        setError('');
        return null;
      }

      const key = previewCacheKey(cacheScope, source, candidate, installedStateKey);
      if (!forceRefresh) {
        const cached = previewCache.get(key);
        if (cached) {
          setPreview(cached);
          setLoading(false);
          setError('');
          return cached;
        }
      }

      const runId = runRef.current + 1;
      runRef.current = runId;
      setLoading(true);
      setError('');

      try {
        const next = await previewCatalogInstall({
          source,
          projectId: candidate.id,
          forceRefresh: forceRefresh || Boolean(installedStateKey)
        });
        if (runRef.current !== runId) return next;
        writePreviewCache(key, next);
        setPreview(next);
        return next;
      } catch (err) {
        if (runRef.current !== runId) return null;
        setError(String(err));
        return null;
      } finally {
        if (runRef.current !== runId) return;
        setLoading(false);
      }
    },
    [candidate, source, cacheScope, installedStateKey]
  );

  useEffect(() => {
    void load(false);
    return () => {
      runRef.current += 1;
    };
  }, [load]);

  return { preview, loading, error, reload: load };
}
