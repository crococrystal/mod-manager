import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { searchProviderCatalog } from '../../api.js';

const MAX_SEARCH_CACHE_ENTRIES = 80;

function searchCacheKey(scope, source, query) {
  return ['v3', scope ?? 'default', source, query.trim()].join('\u0000');
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

function mergeCandidates(existing, incoming) {
  if (!incoming.length) return existing;
  const seen = new Set(existing.map((item) => item.id));
  const appended = incoming.filter((item) => !seen.has(item.id));
  return appended.length ? [...existing, ...appended] : existing;
}

function filterUpdateResults(results, query) {
  const needle = query.trim().toLowerCase();
  if (!needle) return results;
  return results.filter((item) => {
    const title = item.title?.toLowerCase() ?? '';
    const summary = item.summary?.toLowerCase() ?? '';
    return title.includes(needle) || summary.includes(needle);
  });
}

export function useCatalogSearch({
  query,
  setQuery,
  canSearch,
  curseforgeApiKeySet,
  cacheScope,
  updatesSnapshot
}) {
  const [source, setSource] = useState(null);
  const [results, setResults] = useState([]);
  const [target, setTarget] = useState(null);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [error, setError] = useState('');
  const [installSelection, setInstallSelection] = useState(null);
  const cacheRef = useRef(new Map());
  const searchRunRef = useRef(0);
  const nextOffsetRef = useRef(0);
  const loadMoreRunRef = useRef(0);
  const stateRef = useRef({});

  stateRef.current = {
    source,
    query,
    hasMore,
    loading,
    loadingMore,
    curseforgeApiKeySet
  };

  const closeInstall = useCallback(() => {
    setInstallSelection(null);
  }, []);

  const clearQuery = useCallback(() => {
    setQuery('');
  }, [setQuery]);

  const reset = useCallback(() => {
    searchRunRef.current += 1;
    loadMoreRunRef.current += 1;
    nextOffsetRef.current = 0;
    setSource(null);
    setQuery('');
    setResults([]);
    setTarget(null);
    setLoading(false);
    setLoadingMore(false);
    setHasMore(false);
    setError('');
    setInstallSelection(null);
  }, [setQuery]);

  const toggleSource = useCallback(
    (nextSource) => {
      searchRunRef.current += 1;
      loadMoreRunRef.current += 1;
      nextOffsetRef.current = 0;
      setError('');
      setInstallSelection(null);
      setHasMore(false);
      setLoadingMore(false);
      const cached = readCachedSearch(cacheRef.current, cacheScope, nextSource, query);
      setSource((current) => {
        if (current === nextSource) {
          setQuery('');
          setResults([]);
          setTarget(null);
          setLoading(false);
          return null;
        }
        if (nextSource === 'updates') {
          setResults([]);
          setTarget(null);
          setLoading(false);
        } else if (cached) {
          setResults(cached.candidates ?? []);
          setTarget(cached.target ?? null);
          setHasMore(Boolean(cached.hasMore));
          nextOffsetRef.current = cached.nextOffset ?? cached.candidates?.length ?? 0;
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
      if (source === 'updates') return;
      setInstallSelection({ source, candidate });
    },
    [source]
  );

  const applySearchPayload = useCallback((payload, { append = false } = {}) => {
    const incoming = payload?.candidates ?? [];
    setTarget(payload?.target ?? null);
    setResults((current) => (append ? mergeCandidates(current, incoming) : incoming));
    setHasMore(Boolean(payload?.hasMore));
    nextOffsetRef.current = payload?.nextOffset ?? (append ? nextOffsetRef.current + incoming.length : incoming.length);
  }, []);

  const loadMore = useCallback(() => {
    const state = stateRef.current;
    if (!state.source || state.source === 'updates' || !state.hasMore || state.loading || state.loadingMore) {
      return;
    }

    const needle = state.query.trim();
    if (state.source === 'curseforge' && !state.curseforgeApiKeySet) return;

    const searchRunAtStart = searchRunRef.current;
    const runId = loadMoreRunRef.current + 1;
    loadMoreRunRef.current = runId;
    setLoadingMore(true);

    void searchProviderCatalog({
      source: state.source,
      query: needle,
      offset: nextOffsetRef.current
    })
      .then((payload) => {
        if (loadMoreRunRef.current !== runId || searchRunRef.current !== searchRunAtStart) return;
        applySearchPayload(payload, { append: true });
      })
      .catch((err) => {
        if (loadMoreRunRef.current !== runId || searchRunRef.current !== searchRunAtStart) return;
        setError(String(err));
      })
      .finally(() => {
        if (loadMoreRunRef.current !== runId || searchRunRef.current !== searchRunAtStart) return;
        setLoadingMore(false);
      });
  }, [applySearchPayload]);

  useEffect(() => {
    if (!source || !canSearch) {
      searchRunRef.current += 1;
      loadMoreRunRef.current = searchRunRef.current;
      nextOffsetRef.current = 0;
      setResults([]);
      setTarget(null);
      setLoading(false);
      setLoadingMore(false);
      setHasMore(false);
      setError('');
      return undefined;
    }

    if (source === 'updates') {
      return undefined;
    }

    const needle = query.trim();
    if (source === 'curseforge' && !curseforgeApiKeySet) {
      searchRunRef.current += 1;
      loadMoreRunRef.current = searchRunRef.current;
      nextOffsetRef.current = 0;
      setResults([]);
      setTarget(null);
      setLoading(false);
      setLoadingMore(false);
      setHasMore(false);
      setError('Для поиска на CurseForge нужен API key в настройках.');
      return undefined;
    }

    const runId = searchRunRef.current + 1;
    searchRunRef.current = runId;
    loadMoreRunRef.current = runId;
    nextOffsetRef.current = 0;
    setError('');
    setHasMore(false);
    setLoadingMore(false);

    const cached = readCachedSearch(cacheRef.current, cacheScope, source, needle);
    if (cached) {
      applySearchPayload(cached);
      setLoading(false);
      return undefined;
    }

    setLoading(true);

    const delay = needle ? 320 : 0;
    const timer = window.setTimeout(() => {
      void searchProviderCatalog({ source, query: needle, offset: 0 })
        .then((payload) => {
          if (searchRunRef.current !== runId) return;
          writeCachedSearch(cacheRef.current, cacheScope, source, needle, payload ?? {});
          applySearchPayload(payload);
        })
        .catch((err) => {
          if (searchRunRef.current !== runId) return;
          setResults([]);
          setTarget(null);
          setHasMore(false);
          nextOffsetRef.current = 0;
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
  }, [source, query, canSearch, curseforgeApiKeySet, cacheScope, applySearchPayload]);

  const updatesResults = useMemo(() => {
    if (source !== 'updates') return [];
    return filterUpdateResults(updatesSnapshot?.candidates ?? [], query);
  }, [source, updatesSnapshot?.candidates, query]);

  const visibleResults = source === 'updates' ? updatesResults : results;
  const visibleTarget = source === 'updates' ? (updatesSnapshot?.target ?? null) : target;
  const visibleLoading =
    source === 'updates' ? Boolean(updatesSnapshot?.loading) : loading;
  const visibleError = source === 'updates' ? (updatesSnapshot?.error ?? '') : error;

  return {
    source,
    mode: Boolean(source),
    results: visibleResults,
    target: visibleTarget,
    loading: visibleLoading,
    loadingMore,
    hasMore: source === 'updates' ? false : hasMore,
    error: visibleError,
    updatesBlocked: source === 'updates' ? Boolean(updatesSnapshot?.blocked) : false,
    installSelection,
    toggleSource,
    clearQuery,
    reset,
    selectCandidate,
    closeInstall,
    loadMore
  };
}
