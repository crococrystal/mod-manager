import { useCallback, useEffect, useRef, useState } from 'react';
import { catalogProjectDetails } from '../../api.js';

const MAX_DETAILS_CACHE_ENTRIES = 120;
const detailsCache = new Map();

function detailsCacheKey(scope, source, candidate) {
  return [scope ?? 'default', source, candidate?.id ?? ''].join('\u0000');
}

function writeDetailsCache(key, details) {
  detailsCache.delete(key);
  detailsCache.set(key, details);
  while (detailsCache.size > MAX_DETAILS_CACHE_ENTRIES) {
    const oldest = detailsCache.keys().next().value;
    detailsCache.delete(oldest);
  }
}

export function useCatalogProjectDetails({ candidate, source, cacheScope }) {
  const [details, setDetails] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const runRef = useRef(0);

  const load = useCallback(
    async (forceRefresh = false) => {
      if (!candidate || !source) {
        runRef.current += 1;
        setDetails(null);
        setLoading(false);
        setError('');
        return null;
      }

      const key = detailsCacheKey(cacheScope, source, candidate);
      if (!forceRefresh) {
        const cached = detailsCache.get(key);
        if (cached) {
          setDetails(cached);
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
        const next = await catalogProjectDetails({
          source,
          projectId: candidate.id,
          forceRefresh
        });
        if (runRef.current !== runId) return next;
        writeDetailsCache(key, next);
        setDetails(next);
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
    [candidate, source, cacheScope]
  );

  useEffect(() => {
    void load(false);
    return () => {
      runRef.current += 1;
    };
  }, [load]);

  return { details, loading, error, reload: load };
}
