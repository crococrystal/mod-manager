import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { checkModUpdates } from '../api.js';
import { syncUpdateCandidatesList } from '../features/updates/updateCandidateSync.js';

const EMPTY_UPDATES = {
  target: null,
  candidates: [],
  checkedProjects: 0,
  failedProjects: 0,
  checkedAtMs: null,
  cached: false,
  loading: false,
  error: ''
};

const SILENT_RETRY_MAX = 2;
const SILENT_RETRY_BASE_MS = 2500;

function createSnapshot(scope, patch = {}) {
  return {
    scope,
    ...EMPTY_UPDATES,
    ...patch
  };
}

function isRestorableSnapshot(snapshot) {
  return Boolean(snapshot && !snapshot.loading && snapshot.checkedAtMs != null);
}

function payloadToState(payload) {
  return {
    target: payload?.target ?? null,
    candidates: payload?.candidates ?? [],
    checkedProjects: payload?.checkedProjects ?? 0,
    failedProjects: payload?.failedProjects ?? 0,
    checkedAtMs: payload?.checkedAtMs ?? null,
    cached: Boolean(payload?.cached),
    loading: false,
    error: ''
  };
}

function insertCandidate(list, candidate) {
  const key = candidate?.key ?? candidate?.id;
  if (!key || list.some((item) => (item.key ?? item.id) === key)) {
    return list;
  }
  const next = [...list, candidate];
  next.sort((left, right) =>
    (left.title ?? '').localeCompare(right.title ?? '', 'ru', { sensitivity: 'base' })
  );
  return next;
}

export function useModUpdates({ enabled, instanceRoot, checkReady = true }) {
  const activeScope = instanceRoot ?? '';
  const [snapshot, setSnapshot] = useState(() => createSnapshot(activeScope));
  const [silentRetrying, setSilentRetrying] = useState(false);
  const runRef = useRef(0);
  const silentRetryCountRef = useRef(0);
  const retryTimeoutRef = useRef(null);
  const lastCheckedScopeRef = useRef('');
  const previousScopeRef = useRef(null);
  const snapshotRef = useRef(snapshot);
  const scopeSnapshotsRef = useRef(new Map());

  useEffect(() => {
    snapshotRef.current = snapshot;
    if (isRestorableSnapshot(snapshot)) {
      scopeSnapshotsRef.current.set(snapshot.scope, snapshot);
    }
  }, [snapshot]);

  const canCheck = Boolean(enabled && activeScope && checkReady);

  if (snapshot.scope !== activeScope) {
    setSnapshot(
      createSnapshot(activeScope, {
        loading: Boolean(enabled && activeScope)
      })
    );
  }

  const inScope = snapshot.scope === activeScope;
  const state = inScope
    ? snapshot
    : createSnapshot(activeScope, { loading: Boolean(enabled && activeScope) });

  const patchSnapshot = useCallback(
    (patch) => {
      setSnapshot((current) => {
        if (current.scope !== activeScope) {
          return createSnapshot(activeScope, patch);
        }
        return { ...current, ...patch, scope: activeScope };
      });
    },
    [activeScope]
  );

  useEffect(() => {
    if (!enabled) return undefined;

    let unlisteners = [];

    void (async () => {
      unlisteners.push(
        await listen('mod-updates-check-started', (event) => {
          const runId = runRef.current;
          const payload = event.payload ?? {};
          setSnapshot((current) => {
            if (current.scope !== activeScope || runId !== runRef.current) {
              return current;
            }
            return {
              ...current,
              loading: true,
              target: payload.target ?? current.target,
              checkedProjects: payload.checkedProjects ?? current.checkedProjects
            };
          });
        })
      );

      unlisteners.push(
        await listen('mod-updates-candidate', (event) => {
          const runId = runRef.current;
          const candidate = event.payload;
          if (!candidate) return;
          setSnapshot((current) => {
            if (current.scope !== activeScope || runId !== runRef.current) {
              return current;
            }
            return {
              ...current,
              loading: true,
              candidates: insertCandidate(current.candidates, candidate)
            };
          });
        })
      );
    })();

    return () => {
      unlisteners.forEach((unlisten) => {
        if (typeof unlisten === 'function') unlisten();
      });
    };
  }, [activeScope, enabled]);

  const scheduleSilentRetry = useCallback(
    (runId, scope) => {
      const attempt = silentRetryCountRef.current;
      const delay = SILENT_RETRY_BASE_MS * attempt;
      setSilentRetrying(true);
      retryTimeoutRef.current = window.setTimeout(() => {
        retryTimeoutRef.current = null;
        if (runRef.current !== runId || scope !== activeScope) return;

        const nextRunId = runRef.current + 1;
        runRef.current = nextRunId;
        patchSnapshot({ loading: true, error: '' });

        void checkModUpdates({ forceRefresh: true })
          .then((payload) => {
            if (runRef.current !== nextRunId || scope !== activeScope) return;

            const failed = payload?.failedProjects ?? 0;
            if (failed > 0 && silentRetryCountRef.current < SILENT_RETRY_MAX) {
              silentRetryCountRef.current += 1;
              patchSnapshot({
                ...payloadToState({ ...payload, failedProjects: 0 }),
                loading: true
              });
              scheduleSilentRetry(nextRunId, scope);
              return;
            }

            setSilentRetrying(false);
            silentRetryCountRef.current = 0;
            patchSnapshot(payloadToState(payload));
          })
          .catch((err) => {
            if (runRef.current !== nextRunId || scope !== activeScope) return;
            setSilentRetrying(false);
            silentRetryCountRef.current = 0;
            patchSnapshot({ loading: false, error: String(err) });
          });
      }, delay);
    },
    [activeScope, patchSnapshot]
  );

  const runUpdatesCheck = useCallback(
    ({ forceRefresh = false, background = false } = {}) => {
      if (!enabled || !activeScope) return;

      if (retryTimeoutRef.current) {
        window.clearTimeout(retryTimeoutRef.current);
        retryTimeoutRef.current = null;
      }
      setSilentRetrying(false);

      const scope = activeScope;
      const runId = runRef.current + 1;
      runRef.current = runId;
      silentRetryCountRef.current = 0;
      if (!background) {
        setSnapshot(createSnapshot(scope, { loading: true }));
      }

      void checkModUpdates({ forceRefresh })
        .then((payload) => {
          if (runRef.current !== runId || scope !== activeScope) return;

          const failed = payload?.failedProjects ?? 0;
          if (failed > 0 && silentRetryCountRef.current < SILENT_RETRY_MAX) {
            silentRetryCountRef.current += 1;
            if (!background) {
              patchSnapshot({
                ...payloadToState({ ...payload, failedProjects: 0 }),
                loading: true
              });
            }
            scheduleSilentRetry(runId, scope);
            return;
          }

          patchSnapshot(payloadToState(payload));
        })
        .catch((err) => {
          if (runRef.current !== runId || scope !== activeScope) return;
          if (background && isRestorableSnapshot(snapshotRef.current)) return;
          patchSnapshot({ ...EMPTY_UPDATES, loading: false, error: String(err) });
        });
    },
    [activeScope, enabled, patchSnapshot, scheduleSilentRetry]
  );

  useLayoutEffect(() => {
    const previousScope = previousScopeRef.current;
    if (previousScope && snapshotRef.current.scope === previousScope) {
      scopeSnapshotsRef.current.set(previousScope, snapshotRef.current);
    }
    previousScopeRef.current = activeScope;

    const restored = scopeSnapshotsRef.current.get(activeScope);
    const canRestore = isRestorableSnapshot(restored);

    runRef.current += 1;
    silentRetryCountRef.current = 0;
    setSilentRetrying(false);
    if (retryTimeoutRef.current) {
      window.clearTimeout(retryTimeoutRef.current);
      retryTimeoutRef.current = null;
    }
    lastCheckedScopeRef.current = '';
    setSnapshot(
      canRestore
        ? { ...restored, scope: activeScope }
        : createSnapshot(activeScope, {
            loading: Boolean(enabled && activeScope)
          })
    );
  }, [activeScope, enabled]);

  useEffect(() => {
    if (!canCheck || !activeScope) return;
    if (lastCheckedScopeRef.current === activeScope) return;
    lastCheckedScopeRef.current = activeScope;
    const restored = scopeSnapshotsRef.current.get(activeScope);
    runUpdatesCheck({
      forceRefresh: false,
      background: isRestorableSnapshot(restored)
    });
  }, [canCheck, activeScope, runUpdatesCheck]);

  useEffect(
    () => () => {
      if (retryTimeoutRef.current) {
        window.clearTimeout(retryTimeoutRef.current);
      }
    },
    []
  );

  const removeUpdateCandidate = useCallback(
    (key) => {
      if (!key) return;
      setSnapshot((current) => {
        if (current.scope !== activeScope) return current;
        const next = current.candidates.filter((item) => (item.key ?? item.id) !== key);
        return next.length === current.candidates.length ? current : { ...current, candidates: next };
      });
    },
    [activeScope]
  );

  const syncUpdateCandidates = useCallback(
    (modsByKey, cacheScope) => {
      setSnapshot((current) => {
        if (current.scope !== activeScope) return current;
        const next = syncUpdateCandidatesList(current.candidates, modsByKey, cacheScope);
        const unchanged =
          next.length === current.candidates.length &&
          next.every(
            (item, index) =>
              (item.key ?? item.id) === (current.candidates[index]?.key ?? current.candidates[index]?.id) &&
              item.summary === current.candidates[index]?.summary
          );
        return unchanged ? current : { ...current, candidates: next };
      });
    },
    [activeScope]
  );

  return {
    updatesScope: activeScope,
    updatesTarget: inScope ? state.target : null,
    updatesCandidates: inScope ? state.candidates : [],
    updatesCheckedProjects: inScope ? state.checkedProjects : 0,
    updatesFailedProjects: inScope ? state.failedProjects : 0,
    updatesCheckedAtMs: inScope ? state.checkedAtMs : null,
    updatesLoading: inScope ? state.loading : Boolean(enabled && activeScope),
    updatesLoadingVisible: inScope ? state.loading && !silentRetrying : false,
    updatesError: inScope ? state.error : '',
    updatesReady: inScope && checkReady && !state.loading && state.checkedAtMs != null,
    runUpdatesCheck,
    removeUpdateCandidate,
    syncUpdateCandidates
  };
};
