import { useCallback, useMemo, useState } from 'react';
import { useModUpdates } from '../../hooks/useModUpdates.js';
import { installAllModUpdates } from '../../lib/installAllModUpdates.js';
import { filterStaleUpdateCandidates, modNeedsUpdate } from './updateCandidateSync.js';

function isUpdatesInitBlocked({ scanning, workspaceInitializing }) {
  return scanning || workspaceInitializing;
}

export function useUpdatesFeature({
  enabled,
  instanceRoot,
  syncing,
  scanning,
  busy,
  bootstrapping,
  updateInstallBusy = false,
  modsByKey,
  cacheScope,
  updateModInPayload,
  setSelected,
  setSelectedKeys,
  setError,
  setInfo
}) {
  const [updateAllBusy, setUpdateAllBusy] = useState(false);
  const [updateAllProgress, setUpdateAllProgress] = useState(null);
  const [updateAllError, setUpdateAllError] = useState('');

  const workspaceInitializing = bootstrapping || syncing || scanning;
  const checkReady =
    !workspaceInitializing && !busy && !updateAllBusy && !updateInstallBusy;

  const modUpdates = useModUpdates({
    enabled,
    instanceRoot,
    checkReady
  });

  const syncedCandidates = useMemo(
    () =>
      filterStaleUpdateCandidates(modUpdates.updatesCandidates, modsByKey, cacheScope),
    [modUpdates.updatesCandidates, modsByKey, cacheScope]
  );

  const updatesInitBlocked = isUpdatesInitBlocked({
    scanning,
    workspaceInitializing
  });

  const snapshot = useMemo(
    () => ({
      target: updatesInitBlocked ? null : modUpdates.updatesTarget,
      candidates: updatesInitBlocked ? [] : syncedCandidates,
      checkedProjects: updatesInitBlocked ? 0 : modUpdates.updatesCheckedProjects,
      failedProjects: updatesInitBlocked ? 0 : modUpdates.updatesFailedProjects,
      checkedAtMs:
        updatesInitBlocked || !modUpdates.updatesReady ? null : modUpdates.updatesCheckedAtMs,
      ready: modUpdates.updatesReady && !updatesInitBlocked,
      loading: modUpdates.updatesLoadingVisible,
      error: updatesInitBlocked ? '' : modUpdates.updatesError,
      blocked: updatesInitBlocked
    }),
    [modUpdates, syncedCandidates, updatesInitBlocked]
  );

  const resolveUpdateCandidate = useCallback(
    (key) => {
      modUpdates.removeUpdateCandidate(key);
    },
    [modUpdates]
  );

  const runModUpdatesInstall = useCallback(
    async (candidates) => {
      if (!candidates.length || busy || updateAllBusy) return;

      setUpdateAllBusy(true);
      setUpdateAllError('');
      setUpdateAllProgress(null);
      setError('');

      try {
        const { done, errors } = await installAllModUpdates({
          candidates: [...candidates],
          modsByKey,
          cacheScope,
          onProgress: setUpdateAllProgress,
          onCandidateResolved: resolveUpdateCandidate,
          onEachInstalled: ({ key, ...patch }) => {
            updateModInPayload(key, patch);
            const base = modsByKey.get(key);
            const updatedMod = base ? { ...base, ...patch } : { key, ...patch };
            if (!modNeedsUpdate(updatedMod, cacheScope)) {
              resolveUpdateCandidate(key);
            }
            setSelectedKeys((current) => {
              if (!current.has(key)) return current;
              const next = new Set(current);
              next.delete(key);
              return next;
            });
            setSelected((current) => (current?.key === key ? null : current));
          }
        });

        modUpdates.syncUpdateCandidates(modsByKey, cacheScope);

        if (errors.length) {
          setUpdateAllError(errors.join('\n'));
        }
        if (done > 0) {
          setInfo(done === 1 ? 'Обновлён 1 мод.' : `Обновлено модов: ${done}.`);
        }
      } catch (err) {
        setUpdateAllError(String(err));
      } finally {
        setUpdateAllBusy(false);
        setUpdateAllProgress(null);
      }
    },
    [
      busy,
      cacheScope,
      modUpdates,
      modsByKey,
      resolveUpdateCandidate,
      setError,
      setInfo,
      setSelected,
      setSelectedKeys,
      updateAllBusy,
      updateModInPayload
    ]
  );

  return {
    workspaceInitializing,
    updatesInitBlocked,
    snapshot,
    updateAllBusy,
    updateAllProgress,
    updateAllError,
    runModUpdatesInstall,
    updatesCandidates: syncedCandidates,
    updatesCheckedAtMs: modUpdates.updatesCheckedAtMs,
    updatesLoadingVisible: modUpdates.updatesLoadingVisible,
    updatesFailedProjects: modUpdates.updatesFailedProjects,
    updatesError: modUpdates.updatesError,
    updatesReady: modUpdates.updatesReady,
    removeUpdateCandidate: modUpdates.removeUpdateCandidate,
    syncUpdateCandidates: modUpdates.syncUpdateCandidates
  };
}

export function updatesToolbarStatus({
  updatesInitBlocked,
  updatesLoadingVisible,
  updatesCheckedAtMs,
  updatesCandidates
}) {
  if (updatesInitBlocked) return 'idle';
  if (updatesLoadingVisible) return 'loading';
  if (!updatesCheckedAtMs) return 'idle';
  if (updatesCandidates.length > 0) return 'available';
  return 'current';
}

export function shouldShowUpdatesPanel({
  isUpdatesMode,
  updatesInitBlocked,
  updateAllBusy,
  updateInstallBusy = false,
  updatesLoadingVisible = false,
  updatesCandidates,
  selectedKeys
}) {
  if (!isUpdatesMode || updatesInitBlocked || updateAllBusy || updateInstallBusy) {
    return false;
  }
  const hasSelection = selectedKeys.size > 0;
  if (updatesLoadingVisible && !hasSelection) {
    return false;
  }
  if (hasSelection) return true;
  return updatesCandidates.length > 0;
}
