import { useCallback, useEffect, useRef, useState } from 'react';
import { searchProviderCatalog } from '../../api.js';

const MAX_SEARCH_CACHE_ENTRIES = 80;

function searchCacheKey(scope, source, query) {
  return [scope ?? 'default', source, query.trim()].join('\u0000');
}

function readCachedSearch(cache, scope, source, query) {
  return cache.get(searchCacheKey(scope, source, query));
}

function writeCachedSearch(cache, scope, source, query, payload) {
  const key = searchCacheKey(scope, source, query);
  cache.delete(key);
  cache.set(key, payload);
  while (cache.size > MAX_SEARCH_CACHE_ENTRIES) {
    const oldest = cache.keys().next().value;
    cache.delete(oldest);
  }
}

export function useCatalogSearch({
  query,
  setQuery,
  canSearch,
  curseforgeApiKeySet,
  cacheScope
}) {
  const [source, setSource] = useState(null);
  const [results, setResults] = useState([]);
  const [target, setTarget] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [installSelection, setInstallSelection] = useState(null);
  const cacheRef = useRef(new Map());
  const searchRunRef = useRef(0);

  const closeInstall = useCallback(() => {
    setInstallSelection(null);
  }, []);

  const clearQuery = useCallback(() => {
    setQuery('');
  }, [setQuery]);

  const reset = useCallback(() => {
    searchRunRef.current += 1;
    setSource(null);
    setQuery('');
    setResults([]);
    setTarget(null);
    setLoading(false);
    setError('');
    setInstallSelection(null);
  }, [setQuery]);

  const toggleSource = useCallback(
    (nextSource) => {
      searchRunRef.current += 1;
      setError('');
      setInstallSelection(null);
      const cached = readCachedSearch(cacheRef.current, cacheScope, nextSource, query);
      setSource((current) => {
        if (current === nextSource) {
          setQuery('');
          setResults([]);
          setTarget(null);
          setLoading(false);
          return null;
        }
        if (cached) {
          setResults(cached.candidates ?? []);
          setTarget(cached.target ?? null);
          setLoading(false);
        } else {
          setResults([]);
          setTarget(null);
        }
        return nextSource;
      });
    },
    [cacheScope, query, setQuery]
  );

  const selectCandidate = useCallback(
    (candidate) => {
      if (!source || !candidate) return;
      setInstallSelection({ source, candidate });
    },
    [source]
  );

  useEffect(() => {
    if (!source || !canSearch) {
      searchRunRef.current += 1;
      setResults([]);
      setTarget(null);
      setLoading(false);
      setError('');
      return undefined;
    }

    const needle = query.trim();
    if (source === 'curseforge' && !curseforgeApiKeySet) {
      searchRunRef.current += 1;
      setResults([]);
      setTarget(null);
      setLoading(false);
      setError('Для поиска на CurseForge нужен API key в настройках.');
      return undefined;
    }

    const runId = searchRunRef.current + 1;
    searchRunRef.current = runId;
    setError('');

    const cached = readCachedSearch(cacheRef.current, cacheScope, source, needle);
    if (cached) {
      setTarget(cached.target ?? null);
      setResults(cached.candidates ?? []);
      setLoading(false);
      return undefined;
    }

    setLoading(true);

    const delay = needle ? 320 : 0;
    const timer = window.setTimeout(() => {
      void searchProviderCatalog({ source, query: needle })
        .then((payload) => {
          if (searchRunRef.current !== runId) return;
          writeCachedSearch(cacheRef.current, cacheScope, source, needle, payload ?? {});
          setTarget(payload?.target ?? null);
          setResults(payload?.candidates ?? []);
        })
        .catch((err) => {
          if (searchRunRef.current !== runId) return;
          setResults([]);
          setTarget(null);
          setError(String(err));
        })
        .finally(() => {
          if (searchRunRef.current !== runId) return;
          setLoading(false);
        });
    }, delay);

    return () => {
      window.clearTimeout(timer);
    };
  }, [source, query, canSearch, curseforgeApiKeySet, cacheScope]);

  return {
    source,
    mode: Boolean(source),
    results,
    target,
    loading,
    error,
    installSelection,
    toggleSource,
    clearQuery,
    reset,
    selectCandidate,
    closeInstall
  };
}
