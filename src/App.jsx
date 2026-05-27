import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Search, Settings, SlidersHorizontal, X } from 'lucide-react';
import {
  bootstrapInstance,
  cancelBackgroundTask,
  copyModFiles,
  deleteCustomCover,
  getSettings,
  identifyModSources,
  refreshModAssets,
  refreshProviderLabels,
  saveSettings,
  scanMods,
  syncProviderData,
  updateModTags,
  uploadCover
} from './api.js';
import { TitleBar } from './components/TitleBar.jsx';
import { NoticeModal } from './components/NoticeModal.jsx';
import { UpdateModal } from './components/UpdateModal.jsx';
import { DescriptionDialog } from './features/mods/DescriptionDialog.jsx';
import { ModEditor } from './features/mods/ModEditor.jsx';
import { ModTable } from './features/mods/ModTable.jsx';
import { ProviderDialog } from './features/mods/ProviderDialog.jsx';
import { TagsDialog } from './features/mods/TagsDialog.jsx';
import { VersionDialog } from './features/mods/VersionDialog.jsx';
import { SettingsDialog } from './features/settings/SettingsDialog.jsx';
import { useAppUpdater } from './hooks/useAppUpdater.js';
import { canCheckForUpdates } from './lib/updater.js';
import { modMatchesWorkspaceFilters } from './lib/modFilters.js';
import { filters } from './lib/modMeta.jsx';
import headerAppLogo from './assets/header-app-logo.svg';
import { normalizeModsGraph } from './lib/usedBy.js';
import './styles/index.css';

const EMPTY_MODS = [];
const EMPTY_STATS = {};

function needsBootstrap(cacheStatus, { force = false } = {}) {
  if (force) return true;
  if (!cacheStatus) return true;
  return cacheStatus.needsCovers || cacheStatus.needsDependencies;
}

function coverUrlFor(mod) {
  if (!mod.coverPath) return mod.coverUrl ?? null;
  const base = convertFileSrc(mod.coverPath);
  return mod.coverModifiedAt ? `${base}?v=${mod.coverModifiedAt}` : base;
}

function statsForMods(mods) {
  return {
    total: mods.length,
    client: mods.filter((mod) => mod.side === 'client').length,
    universal: mods.filter((mod) => mod.side === 'universal').length,
    server: mods.filter((mod) => mod.side === 'server').length,
    noIndex: mods.filter((mod) => mod.source === 'manual' || mod.source === 'index').length,
    tagged: mods.filter((mod) => mod.hasTags).length
  };
}

function withLocalCovers(next) {
  const mods = (next.mods ?? []).map((mod) => ({
    ...mod,
    coverUrl: coverUrlFor(mod)
  }));
  normalizeModsGraph(mods);
  return { ...next, mods };
}

function coverUpdatePatch(result) {
  const patch = {
    coverPath: result.coverPath ?? null,
    coverModifiedAt: result.coverModifiedAt ?? null,
    coverManual: Boolean(result.coverManual)
  };
  patch.coverUrl = coverUrlFor({
    coverPath: patch.coverPath,
    coverModifiedAt: patch.coverModifiedAt
  });
  return patch;
}

function refreshedAssetsPatch(result) {
  const patch = {
    dependencies: result.dependencies ?? [],
    resolvedDependencies: result.resolvedDependencies ?? result.dependencies ?? []
  };
  if (result.coverPath) {
    patch.coverPath = result.coverPath;
    patch.coverModifiedAt = result.coverModifiedAt ?? null;
    patch.coverManual = false;
    patch.coverUrl = coverUrlFor({
      coverPath: result.coverPath,
      coverModifiedAt: result.coverModifiedAt
    });
  }
  return patch;
}

function modTagsUpdatePatch(result) {
  const patch = {
    side: result.side,
    library: Boolean(result.library),
    technical: Boolean(result.technical),
    sideMode: result.sideMode ?? result.side_mode ?? 'auto',
    manualSide: result.manualSide ?? result.manual_side,
    manualLibrary: Boolean(result.manualLibrary ?? result.manual_library),
    manualTechnical: Boolean(result.manualTechnical ?? result.manual_technical),
    providerSide: result.providerSide ?? result.provider_side,
    providerLibrary: Boolean(result.providerLibrary ?? result.provider_library),
    providerTechnical: Boolean(result.providerTechnical ?? result.provider_technical)
  };
  if (result.description != null) {
    patch.description = result.description;
  }
  return patch;
}

function applyModTagsUpdate(result, { applyPayload, updateModInPayload }) {
  if (result?.payload) {
    applyPayload(result.payload);
    return;
  }
  if (result?.key) {
    updateModInPayload(result.key, modTagsUpdatePatch(result));
  }
}

function refreshedProviderLabelsPatch(result) {
  return modTagsUpdatePatch(result);
}

function syncProgressLabel(progress) {
  if (!progress) return 'Синхронизация · Подготовка…';
  if (!progress.total) return 'Синхронизация · Подготовка…';
  const tail = progress.name ? ` · ${progress.name}` : '';
  return `Синхронизация · ${progress.index}/${progress.total}${tail}`;
}

function isTextEntryTarget(target) {
  return (
    target instanceof HTMLElement &&
    Boolean(target.closest('input, textarea, select, [contenteditable="true"]'))
  );
}

function hasActiveTextSelection() {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
    return false;
  }
  return selection.toString().length > 0;
}

function hasTauriRuntime() {
  return typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__);
}

async function listenEvent(name, handler) {
  if (!hasTauriRuntime()) {
    return () => {};
  }
  return listen(name, handler);
}

const SORT_DEFAULT_DIRECTION = {
  tag: 'asc',
  name: 'asc',
  description: 'asc',
  version: 'desc',
  date: 'desc',
  source: 'asc'
};

function tagSortValue(mod) {
  return [
    mod.side ?? '',
    mod.library ? 'library' : '',
    mod.technical ? 'technical' : ''
  ].join(' ');
}

function sortValue(mod, key) {
  switch (key) {
    case 'tag':
      return tagSortValue(mod);
    case 'name':
      return mod.displayName ?? '';
    case 'description':
      return mod.description ?? '';
    case 'version':
      return mod.installedVersion ?? '';
    case 'date':
      return new Date(mod.modifiedAt ?? 0).getTime() || 0;
    case 'source':
      return mod.source ?? '';
    default:
      return mod.displayName ?? '';
  }
}

function sortMods(mods, sort) {
  const direction = sort?.direction === 'desc' ? -1 : 1;
  return mods
    .map((mod, index) => ({ mod, index }))
    .sort((left, right) => {
      const leftValue = sortValue(left.mod, sort?.key);
      const rightValue = sortValue(right.mod, sort?.key);
      let result;
      if (typeof leftValue === 'number' && typeof rightValue === 'number') {
        result = leftValue - rightValue;
      } else {
        result = String(leftValue).localeCompare(String(rightValue), 'ru', {
          numeric: true,
          sensitivity: 'base'
        });
      }
      return result === 0 ? left.index - right.index : result * direction;
    })
    .map((item) => item.mod);
}

function Stat({ label, value = 0, tone }) {
  return (
    <div className={`stat ${tone}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function App() {
  const [payload, setPayload] = useState(null);
  const [settings, setSettings] = useState(null);
  const [selected, setSelected] = useState(null);
  const [selectedKeys, setSelectedKeys] = useState(() => new Set());
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [busy, setBusy] = useState(false);
  const [bootstrapping, setBootstrapping] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState('');
  const [info, setInfo] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [progress, setProgress] = useState(null);
  const [providerKey, setProviderKey] = useState(null);
  const [versionKey, setVersionKey] = useState(null);
  const [tagsKey, setTagsKey] = useState(null);
  const [tagsSavingKey, setTagsSavingKey] = useState(null);
  const [relationsKey, setRelationsKey] = useState(null);
  const [descriptionKey, setDescriptionKey] = useState(null);
  const [descriptionSavingKey, setDescriptionSavingKey] = useState(null);
  const [refreshingAssetsKey, setRefreshingAssetsKey] = useState(null);
  const [coverSavingKey, setCoverSavingKey] = useState(null);
  const [labelsRefreshingKey, setLabelsRefreshingKey] = useState(null);
  const [sort, setSort] = useState({ key: 'name', direction: 'asc' });
  const watcherReloadingRef = useRef(false);
  const watcherReloadPendingRef = useRef(false);
  const selectionAnchorRef = useRef(null);
  const bootstrapRunRef = useRef(0);
  const syncRunRef = useRef(0);
  const syncingRef = useRef(false);
  const sourceIdentifyRunRef = useRef(0);
  const startupUpdateCheckedRef = useRef(false);
  const updater = useAppUpdater();

  useEffect(() => {
    syncingRef.current = syncing;
  }, [syncing]);

  const mods = payload?.mods ?? EMPTY_MODS;
  const stats = payload?.stats ?? EMPTY_STATS;

  const visible = useMemo(() => {
    const filtered = mods.filter((mod) => modMatchesWorkspaceFilters(mod, { query, filter }));
    return sortMods(filtered, sort);
  }, [mods, query, filter, sort]);

  const canShowWorkspace = Boolean(settings?.instanceRoot);

  const applyPayload = useCallback((next) => {
    const normalized = withLocalCovers(next);
    setPayload(normalized);
    setSettings(normalized.settings);
    setSelected((current) => {
      if (!normalized.mods.length) return null;
      if (!current?.filename) return normalized.mods[0];
      return normalized.mods.find((mod) => mod.filename === current.filename) ?? normalized.mods[0];
    });
  }, []);

  const updateModInPayload = useCallback((key, patch) => {
    setPayload((current) => {
      if (!current) return current;
      let touched = false;
      const mods = current.mods.map((mod) => {
        if (mod.key !== key) return mod;
        touched = true;
        return { ...mod, ...patch };
      });
      if (!touched) return current;
      normalizeModsGraph(mods);
      return { ...current, mods, stats: statsForMods(mods) };
    });
    setSelected((current) => (current?.key === key ? { ...current, ...patch } : current));
  }, []);

  const loadSettings = useCallback(async () => {
    const next = await getSettings();
    setSettings(next);
    return next;
  }, []);

  const reload = useCallback(
    async ({ silent = false } = {}) => {
      if (!silent) {
        setBusy(true);
        setError('');
        setInfo('');
      }
      try {
        const next = await scanMods();
        applyPayload(next);
      } catch (err) {
        setError(String(err));
      } finally {
        if (!silent) setBusy(false);
      }
    },
    [applyPayload]
  );

  const runBootstrap = useCallback(
    async (cacheStatus, { force = false } = {}) => {
      if (!cacheStatus?.instanceRoot && !force) return;
      if (!needsBootstrap(cacheStatus, { force })) return;

      const runId = ++bootstrapRunRef.current;
      setBootstrapping(true);
      setError('');
      setProgress(null);
      try {
        const result = await bootstrapInstance(force);
        if (runId !== bootstrapRunRef.current) return;
        if (!result?.skipped) {
          await reload({ silent: true });
        }
      } catch (err) {
        if (runId !== bootstrapRunRef.current) return;
        const message = String(err);
        if (message.includes('Прервано') || message.includes('другая сборка')) return;
        setError(message);
      } finally {
        if (runId === bootstrapRunRef.current) {
          setBootstrapping(false);
          setProgress(null);
        }
      }
    },
    [reload]
  );

  useEffect(() => {
    loadSettings()
      .then(async (next) => {
        if (!next.instanceRoot) {
          setSettingsOpen(true);
          return;
        }
        const payload = await scanMods();
        applyPayload(payload);
        if (needsBootstrap(next.cacheStatus)) {
          void runBootstrap(next.cacheStatus);
        }
      })
      .catch((err) => setError(String(err)));
  }, [loadSettings, runBootstrap]);

  useEffect(() => {
    if (!settings || startupUpdateCheckedRef.current) return;
    startupUpdateCheckedRef.current = true;
    if (settings.autoCheckUpdates === false) return;
    if (!canCheckForUpdates()) return;
    void updater.check({ silent: true });
  }, [settings, updater]);

  useEffect(() => {
    if (!visible.length) {
      setSelected(null);
      setSelectedKeys((current) => (current.size ? new Set() : current));
      return;
    }
    if (!selected || !visible.some((mod) => mod.filename === selected.filename)) {
      const next = visible[0];
      setSelected(next);
      selectionAnchorRef.current = next.key;
      setSelectedKeys(new Set([next.key]));
    }
  }, [visible, selected]);

  useEffect(() => {
    const visibleKeys = new Set(visible.map((mod) => mod.key));
    setSelectedKeys((current) => {
      const next = new Set([...current].filter((key) => visibleKeys.has(key)));
      const same = next.size === current.size && [...next].every((key) => current.has(key));
      return same ? current : next;
    });
  }, [visible]);

  const moveSelection = useCallback(
    (delta) => {
      if (!visible.length) return;
      const pivotKey =
        relationsKey ??
        providerKey ??
        tagsKey ??
        versionKey ??
        descriptionKey ??
        selected?.key;
      const index = visible.findIndex((mod) => mod.key === pivotKey);
      const nextIndex =
        index < 0
          ? delta > 0
            ? 0
            : visible.length - 1
          : (index + delta + visible.length) % visible.length;
      const next = visible[nextIndex];
      setSelected(next);
      selectionAnchorRef.current = next.key;
      setSelectedKeys(new Set([next.key]));
      setRelationsKey((current) => (current ? next.key : current));
      setProviderKey((current) => (current ? next.key : current));
      setTagsKey((current) => (current ? next.key : current));
      setVersionKey((current) => (current ? next.key : current));
      setDescriptionKey((current) => (current ? next.key : current));
    },
    [descriptionKey, providerKey, relationsKey, selected?.key, tagsKey, versionKey, visible]
  );

  const modModalNav = useCallback(
    (mod) => {
      if (!mod) return undefined;
      const index = visible.findIndex((item) => item.key === mod.key);
      if (index < 0) return undefined;
      return {
        canPrev: index > 0,
        canNext: index < visible.length - 1,
        onPrev: () => moveSelection(-1),
        onNext: () => moveSelection(1)
      };
    },
    [moveSelection, visible]
  );

  const copyModKeys = useCallback(async (keys) => {
    if (!keys.length) return;
    setError('');
    try {
      const count = await copyModFiles(keys);
      setInfo(`Скопировано файлов: ${count}.`);
      setSelectedKeys(new Set());
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    function handleKeyDown(event) {
      const target = event.target;
      if (isTextEntryTarget(target) || hasActiveTextSelection()) {
        return;
      }

      const command = event.metaKey || event.ctrlKey;
      if (command && event.key.toLowerCase() === 'a' && canShowWorkspace) {
        if (target instanceof HTMLElement && target.closest('.descriptionCell')) {
          return;
        }
        event.preventDefault();
        setSelectedKeys(new Set(visible.map((mod) => mod.key)));
        return;
      }

      if (command && event.key.toLowerCase() === 'c' && canShowWorkspace) {
        const keys = selectedKeys.size ? [...selectedKeys] : selected?.key ? [selected.key] : [];
        if (!keys.length) return;
        event.preventDefault();
        void copyModKeys(keys);
        return;
      }

      const prevKey =
        event.key === 'ArrowUp' || event.key === 'ArrowLeft';
      const nextKey =
        event.key === 'ArrowDown' || event.key === 'ArrowRight';
      if (!prevKey && !nextKey) return;
      event.preventDefault();
      moveSelection(prevKey ? -1 : 1);
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [canShowWorkspace, copyModKeys, moveSelection, selected?.key, selectedKeys, visible]);

  useEffect(() => {
    let unlistenMods;
    (async () => {
      unlistenMods = await listenEvent('mods-folder-changed', async () => {
        if (bootstrapping) return;
        if (watcherReloadingRef.current) {
          watcherReloadPendingRef.current = true;
          return;
        }

        watcherReloadingRef.current = true;
        try {
          do {
            watcherReloadPendingRef.current = false;
            await reload({ silent: true });
            const next = await getSettings();
            setSettings(next);
            if (needsBootstrap(next.cacheStatus)) {
              void runBootstrap(next.cacheStatus);
            }
          } while (watcherReloadPendingRef.current && !bootstrapping);
        } finally {
          watcherReloadingRef.current = false;
        }
      });
    })();
    return () => unlistenMods?.();
  }, [reload, runBootstrap, bootstrapping]);

  useEffect(() => {
    let unlistenProgress;
    let unlistenCover;
    let unlistenDependencies;
    let unlistenLabels;
    let unlistenSource;
    (async () => {
      unlistenProgress = await listenEvent('prefetch-progress', (event) => {
        const payload = event.payload;
        if (payload?.status === 'done') {
          setProgress(null);
          return;
        }
        setProgress(payload);
      });
      unlistenCover = await listenEvent('cover-ready', (event) => {
        const { key, coverPath, coverModifiedAt } = event.payload ?? {};
        if (!key || !coverPath) return;
        const base = convertFileSrc(coverPath);
        const coverUrl = coverModifiedAt ? `${base}?v=${coverModifiedAt}` : base;
        updateModInPayload(key, { coverPath, coverUrl, coverModifiedAt, coverManual: false });
      });
      unlistenDependencies = await listenEvent('dependencies-ready', (event) => {
        const { key, dependencies } = event.payload ?? {};
        if (!key || !Array.isArray(dependencies)) return;
        updateModInPayload(key, { dependencies });
      });
      unlistenLabels = await listenEvent('labels-ready', (event) => {
        const { key, ...rest } = event.payload ?? {};
        if (!key) return;
        updateModInPayload(key, modTagsUpdatePatch({ key, ...rest }));
      });
      unlistenSource = await listenEvent('mod-source-ready', (event) => {
        const { key, ...patch } = event.payload ?? {};
        if (!key) return;
        updateModInPayload(key, patch);
      });
    })();
    return () => {
      unlistenProgress?.();
      unlistenCover?.();
      unlistenDependencies?.();
      unlistenLabels?.();
      unlistenSource?.();
    };
  }, [updateModInPayload]);

  const handleSaveSettings = useCallback(
    async (nextSettings, options = {}) => {
      const instanceChanged = nextSettings.instanceRoot !== (settings?.instanceRoot ?? null);
      const previousCurseForgeKey = (settings?.curseforgeApiKey ?? '').trim();
      const nextCurseForgeKey = (nextSettings.curseforgeApiKey ?? '').trim();
      const curseForgeKeyChanged =
        Boolean(nextCurseForgeKey) && previousCurseForgeKey !== nextCurseForgeKey;
      if (instanceChanged) {
        sourceIdentifyRunRef.current += 1;
        setProgress(null);
      }
      setBusy(true);
      setError('');
      try {
        const saved = await saveSettings(nextSettings);
        setSettings(saved);
        const shouldRefreshProviderLinks = Boolean(saved.instanceRoot && curseForgeKeyChanged);
        let providerLinksCoveredByBootstrap = false;
        if (options.scan !== false && saved.instanceRoot) {
          const next = await scanMods();
          applyPayload(next);
          if (options.bootstrap && saved.instanceRoot) {
            providerLinksCoveredByBootstrap = needsBootstrap(saved.cacheStatus, {
              force: options.forceBootstrap
            });
            if (providerLinksCoveredByBootstrap) {
              void runBootstrap(saved.cacheStatus, { force: options.forceBootstrap });
            }
          } else if (needsBootstrap(saved.cacheStatus)) {
            providerLinksCoveredByBootstrap = true;
            void runBootstrap(saved.cacheStatus);
          }
        }
        if (shouldRefreshProviderLinks && !providerLinksCoveredByBootstrap) {
          const runId = ++sourceIdentifyRunRef.current;
          void identifyModSources()
            .then((next) => {
              if (runId !== sourceIdentifyRunRef.current) return;
              applyPayload(next);
            })
            .catch((err) => {
              if (runId !== sourceIdentifyRunRef.current) return;
              setError(String(err));
            });
        }
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyPayload, runBootstrap, settings?.curseforgeApiKey, settings?.instanceRoot]
  );

  const patchMod = useCallback(
    async (key, patch) => {
      setBusy(true);
      setError('');
      try {
        const result = await updateModTags({ key, ...patch });
        applyModTagsUpdate(result, { applyPayload, updateModInPayload });
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyPayload, updateModInPayload]
  );

  const patchModTagsInstant = useCallback(
    async (key, patch, options = {}) => {
      const modeOnly = Object.keys(patch).length === 1 && patch.sideMode != null;
      if (options.optimistic) {
        updateModInPayload(key, options.optimistic);
      }
      if (!modeOnly) {
        setTagsSavingKey(key);
      }
      setError('');
      try {
        const result = await updateModTags({ key, ...patch });
        applyModTagsUpdate(result, { applyPayload, updateModInPayload });
      } catch (err) {
        setError(String(err));
        throw err;
      } finally {
        if (!modeOnly) {
          setTagsSavingKey(null);
        }
      }
    },
    [applyPayload, updateModInPayload]
  );

  const patchModDescriptionInstant = useCallback(
    async (key, patch) => {
      setDescriptionSavingKey(key);
      setError('');
      try {
        const result = await updateModTags({ key, ...patch });
        applyModTagsUpdate(result, { applyPayload, updateModInPayload });
      } catch (err) {
        setError(String(err));
      } finally {
        setDescriptionSavingKey((current) => (current === key ? null : current));
      }
    },
    [applyPayload, updateModInPayload]
  );

  const handleRefreshProviderLabels = useCallback(
    async (key) => {
      setLabelsRefreshingKey(key);
      setError('');
      try {
        const result = await refreshProviderLabels(key);
        updateModInPayload(key, refreshedProviderLabelsPatch(result));
      } catch (err) {
        setError(String(err));
      } finally {
        setLabelsRefreshingKey((current) => (current === key ? null : current));
      }
    },
    [updateModInPayload]
  );

  const handleUploadCover = useCallback(
    async (key, dataUrl) => {
      setCoverSavingKey(key);
      setError('');
      try {
        const result = await uploadCover({ key, dataUrl });
        updateModInPayload(key, coverUpdatePatch(result));
      } catch (err) {
        setError(String(err));
      } finally {
        setCoverSavingKey((current) => (current === key ? null : current));
      }
    },
    [updateModInPayload]
  );

  const handleDeleteCover = useCallback(
    async (key) => {
      setCoverSavingKey(key);
      setError('');
      try {
        const result = await deleteCustomCover(key);
        updateModInPayload(key, coverUpdatePatch(result));
      } catch (err) {
        setError(String(err));
      } finally {
        setCoverSavingKey((current) => (current === key ? null : current));
      }
    },
    [updateModInPayload]
  );

  const handleRunSync = useCallback(
    async (options) => {
      const runId = ++syncRunRef.current;
      setSyncing(true);
      setProgress({ index: 0, total: 0, phase: 'sync', name: '' });
      setError('');
      try {
        const result = await syncProviderData(options);
        if (runId !== syncRunRef.current) return;
        if (result?.payload) {
          applyPayload(result.payload);
        }
        const parts = [];
        if (options.identify) parts.push(`привязано: ${result?.linked ?? 0}`);
        if (options.labels) parts.push(`меток: ${result?.labelsRefreshed ?? 0}`);
        if (options.assets) parts.push(`обложек: ${result?.assetsRefreshed ?? 0}`);
        setInfo(`Синхронизация · готово · ${parts.join(', ')}`);
        return result;
      } catch (err) {
        if (runId !== syncRunRef.current) return;
        const message = String(err);
        if (message.includes('Прервано')) return;
        setError(message);
        throw err;
      } finally {
        if (runId === syncRunRef.current) {
          setSyncing(false);
          setProgress(null);
        }
      }
    },
    [applyPayload]
  );

  const handleClearData = useCallback(async () => {
    setBusy(true);
    setError('');
    try {
      const saved = await getSettings();
      setSettings(saved);
      if (saved.instanceRoot) {
        const next = await scanMods();
        applyPayload(next);
        void runBootstrap(saved.cacheStatus, { force: true });
      }
      setInfo('Данные приложения удалены.');
    } catch (err) {
      setError(String(err));
      throw err;
    } finally {
      setBusy(false);
    }
  }, [applyPayload, runBootstrap]);

  const handleCancelProgress = useCallback(() => {
    if (!bootstrapping && !syncing) return;
    bootstrapRunRef.current += 1;
    syncRunRef.current += 1;
    setBootstrapping(false);
    setSyncing(false);
    setProgress(null);
    void cancelBackgroundTask();
  }, [bootstrapping, syncing]);

  const handleRefreshModAssets = useCallback(
    async (key) => {
      setRefreshingAssetsKey(key);
      setError('');
      try {
        const result = await refreshModAssets(key);
        updateModInPayload(key, refreshedAssetsPatch(result));
      } catch (err) {
        setError(String(err));
      } finally {
        setRefreshingAssetsKey((current) => (current === key ? null : current));
      }
    },
    [updateModInPayload]
  );

  const handleProviderApplied = useCallback(
    (patch) => {
      const { key, coverUrl, ...sourcePatch } = patch;
      const current = mods.find((item) => item.key === key);
      const nextPatch = { ...sourcePatch };
      if (coverUrl && !current?.coverManual) {
        nextPatch.coverUrl = coverUrl;
        nextPatch.coverPath = null;
        nextPatch.coverManual = false;
        nextPatch.coverModifiedAt = null;
      }
      updateModInPayload(key, nextPatch);
      setProviderKey(null);
      if (sourcePatch.source === 'modrinth' || sourcePatch.source === 'curseforge') {
        void handleRefreshProviderLabels(key);
      }
    },
    [handleRefreshProviderLabels, mods, updateModInPayload]
  );

  const handleSort = useCallback((key) => {
    setSort((current) => {
      if (current.key === key) {
        return {
          key,
          direction: current.direction === 'asc' ? 'desc' : 'asc'
        };
      }
      return {
        key,
        direction: SORT_DEFAULT_DIRECTION[key] ?? 'asc'
      };
    });
  }, []);

  const openRelationsForMod = useCallback((mod) => {
    if (!mod) return;
    setSelected(mod);
    setRelationsKey(mod.key);
  }, []);
  const closeRelations = useCallback(() => setRelationsKey(null), []);
  const providerMod = useMemo(
    () => mods.find((item) => item.key === providerKey) ?? null,
    [mods, providerKey]
  );
  const versionMod = useMemo(
    () => mods.find((item) => item.key === versionKey) ?? null,
    [mods, versionKey]
  );
  const tagsMod = useMemo(
    () => mods.find((item) => item.key === tagsKey) ?? null,
    [mods, tagsKey]
  );
  const descriptionMod = useMemo(
    () => mods.find((item) => item.key === descriptionKey) ?? null,
    [mods, descriptionKey]
  );
  const handleVersionInstalled = useCallback(
    ({ key, ...patch }) => {
      updateModInPayload(key, patch);
      setVersionKey(null);
    },
    [updateModInPayload]
  );
  const handleSelectMod = useCallback(
    (mod) => {
      if (!mod) return;
      const fresh = mods.find((item) => item.filename === mod.filename) ?? mod;
      if (!modMatchesWorkspaceFilters(fresh, { query, filter })) {
        if (query.trim()) setQuery('');
        if (!modMatchesWorkspaceFilters(fresh, { query: '', filter })) {
          setFilter('all');
        }
      }
      setSelected(fresh);
      selectionAnchorRef.current = fresh.key;
      setSelectedKeys(new Set([fresh.key]));
      setRelationsKey((current) => (current ? fresh.key : current));
    },
    [mods, query, filter]
  );
  const handleTableSelectDrag = useCallback((mod, select) => {
    setSelected(mod);
    setRelationsKey((current) => (current ? mod.key : current));
    selectionAnchorRef.current = mod.key;
    setSelectedKeys((current) => {
      if (select) {
        if (current.has(mod.key)) return current;
        const next = new Set(current);
        next.add(mod.key);
        return next;
      }
      if (!current.has(mod.key)) return current;
      const next = new Set(current);
      next.delete(mod.key);
      return next;
    });
  }, []);

  const handleTableSelect = useCallback(
    (mod, event) => {
      setSelected(mod);
      setRelationsKey((current) => (current ? mod.key : current));

      if (event?.shiftKey) {
        const anchorKey = selectionAnchorRef.current ?? selected?.key;
        if (anchorKey) {
          const anchorIndex = visible.findIndex((item) => item.key === anchorKey);
          const targetIndex = visible.findIndex((item) => item.key === mod.key);
          if (anchorIndex >= 0 && targetIndex >= 0) {
            const start = Math.min(anchorIndex, targetIndex);
            const end = Math.max(anchorIndex, targetIndex);
            setSelectedKeys(new Set(visible.slice(start, end + 1).map((item) => item.key)));
            return;
          }
        }
      }

      const command = event?.metaKey || event?.ctrlKey;
      if (command) {
        selectionAnchorRef.current = mod.key;
        setSelectedKeys((current) => {
          const next = new Set(current);
          if (next.has(mod.key)) {
            next.delete(mod.key);
          } else {
            next.add(mod.key);
          }
          return next;
        });
        return;
      }

      selectionAnchorRef.current = mod.key;
      setSelectedKeys(new Set([mod.key]));
    },
    [visible, selected?.key]
  );

  const progressPercent =
    progress?.total > 0 ? Math.min(100, Math.round((progress.index / progress.total) * 100)) : 0;
  const progressIndeterminate = Boolean((bootstrapping || syncing) && !(progress?.total > 0));
  const uiLocked = busy;

  const progressLabel = syncing
    ? syncProgressLabel(progress)
    : bootstrapping && progress
    ? `Подготовка · ${progress.index}/${progress.total}${
        progress.name ? ` · ${progress.name}` : ''
      }`
    : '';
  const showProgress = bootstrapping || syncing;

  const toolbar = canShowWorkspace ? (
    <div className="topToolbar" data-tauri-drag-region>
      <img
        src={headerAppLogo}
        alt="Mod Manager"
        className="topToolbarLogo"
        data-tauri-drag-region
      />
      <div className="segments" data-tauri-drag-region>
        {filters.map((item) => {
          const isActive = filter === item.id;
          const Icon = item.icon ?? SlidersHorizontal;
          const showIcon = Boolean(item.icon) || !isActive;
          return (
            <button
              key={item.id}
              className={isActive ? 'active' : ''}
              onClick={() => setFilter(item.id)}
              type="button"
              disabled={busy}
              title={item.label}
              aria-label={item.label}
              data-tauri-drag-region="false"
            >
              {showIcon ? <Icon className={`tagIcon ${item.tone ?? ''}`} size={13} /> : null}
              {isActive ? <span>{item.label}</span> : null}
            </button>
          );
        })}
        <button
          type="button"
          className={`segmentsSettings${settingsOpen ? ' active' : ''}`}
          onClick={() => setSettingsOpen(true)}
          disabled={busy}
          aria-label="Настройки"
          title="Настройки"
          data-tauri-drag-region="false"
        >
          <Settings size={13} />
        </button>
      </div>
      <label className="search" data-tauri-drag-region="false">
        <Search size={14} />
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Поиск по названию или файлу"
          data-tauri-drag-region="false"
        />
      </label>
    </div>
  ) : (
    <div className="topToolbar topToolbarEmpty" data-tauri-drag-region>
      <img
        src={headerAppLogo}
        alt="Mod Manager"
        className="topToolbarLogo"
        data-tauri-drag-region
      />
      <div className="segments" data-tauri-drag-region>
        <button
          type="button"
          className={`segmentsSettings${settingsOpen ? ' active' : ''}`}
          onClick={() => setSettingsOpen(true)}
          disabled={busy}
          aria-label="Настройки"
          title="Настройки"
          data-tauri-drag-region="false"
        >
          <Settings size={13} />
        </button>
      </div>
    </div>
  );

  return (
    <main className="appShell">
      <TitleBar>{toolbar}</TitleBar>

      <div className="appBody">
      {canShowWorkspace ? (
        <>
          <section className="stats">
            <Stat label="Клиент" value={stats.client} tone="client" />
            <Stat label="Оба" value={stats.universal} tone="universal" />
            <Stat label="Сервер" value={stats.server} tone="server" />
            <Stat label="Сторонние" value={stats.noIndex} tone="manual" />
          </section>

          <NoticeModal tone="bad" message={error} onClose={() => setError('')} />
          <NoticeModal tone="ok" message={info && !error && !bootstrapping && !syncing ? info : ''} onClose={() => setInfo('')} />

          <section className="workspace">
            <ModTable
              mods={visible}
              selected={selected}
              selectedKeys={selectedKeys}
              sort={sort}
              onSort={handleSort}
              onSelect={handleTableSelect}
              onSelectDrag={handleTableSelectDrag}
              onCoverClick={openRelationsForMod}
              onSourceClick={(mod) => setProviderKey(mod.key)}
              onVersionClick={(mod) => setVersionKey(mod.key)}
              onTagsClick={(mod) => setTagsKey(mod.key)}
              onDescriptionClick={(mod) => setDescriptionKey(mod.key)}
            />
            <aside>
              {selected ? (
                <ModEditor
                  mod={selected}
                  mods={mods}
                  busy={busy}
                  onPatch={patchMod}
                  onUploadCover={handleUploadCover}
                  onDeleteCover={handleDeleteCover}
                  onRefreshAssets={handleRefreshModAssets}
                  assetsRefreshing={refreshingAssetsKey === selected.key}
                  coverSaving={coverSavingKey === selected.key}
                  relationsOpenKey={relationsKey}
                  relationsModNav={relationsKey ? modModalNav(selected) : undefined}
                  onOpenRelations={openRelationsForMod}
                  onCloseRelations={closeRelations}
                  onSelectMod={handleSelectMod}
                />
              ) : (
                <div className="empty">Выбери мод в списке</div>
              )}
            </aside>
          </section>
        </>
      ) : (
        <section className="setupState">
          <h2>Выбери сборку</h2>
          <p>Укажи папку инстанса PrismLauncher — обложки и зависимости подтянутся один раз в фоне.</p>
          <button type="button" onClick={() => setSettingsOpen(true)} disabled={busy || bootstrapping || syncing}>
            Открыть настройки
          </button>
        </section>
      )}
      </div>

      {showProgress ? (
        <footer className="prefetchProgressWrap">
          <div className="prefetchProgressTrack" aria-hidden="true">
            <div
              className={`prefetchProgressBar${progressIndeterminate ? ' indeterminate' : ''}`}
              style={progressIndeterminate ? undefined : { width: `${progressPercent}%` }}
            />
          </div>
          <div className="prefetchProgressRow">
            <p className="prefetchProgressLabel">{progressLabel}</p>
            <button
              type="button"
              className="prefetchProgressCancel"
              onClick={handleCancelProgress}
              aria-label="Отменить"
              title="Отменить"
            >
              <X size={14} strokeWidth={2} />
            </button>
          </div>
        </footer>
      ) : null}

      {settingsOpen ? (
        <SettingsDialog
          settings={settings}
          busy={uiLocked}
          syncing={syncing}
          updater={updater}
          onClose={() => setSettingsOpen(false)}
          onSave={handleSaveSettings}
          onRunSync={handleRunSync}
          onClearData={handleClearData}
        />
      ) : null}

      {updater.showUpdateModal && !settingsOpen ? (
        <UpdateModal
          currentVersion={updater.pendingUpdate?.currentVersion ?? updater.appVersion}
          version={updater.pendingUpdate?.version}
          status={updater.status}
          progress={updater.progress}
          error={updater.error}
          onInstall={updater.install}
          onDismiss={updater.dismissModal}
        />
      ) : null}

      <ProviderDialog
        mod={providerMod}
        modNav={providerMod ? modModalNav(providerMod) : undefined}
        busy={busy}
        curseforgeApiKeySet={settings?.curseforgeApiKeySet}
        onClose={() => !busy && setProviderKey(null)}
        onApplied={handleProviderApplied}
      />

      <VersionDialog
        mod={versionMod}
        modNav={versionMod ? modModalNav(versionMod) : undefined}
        busy={busy}
        onClose={() => !busy && setVersionKey(null)}
        onInstalled={handleVersionInstalled}
      />

      <TagsDialog
        mod={tagsMod}
        modNav={tagsMod ? modModalNav(tagsMod) : undefined}
        savingKey={tagsSavingKey}
        labelsRefreshing={labelsRefreshingKey}
        onClose={() => {
          if (tagsSavingKey || labelsRefreshingKey) return;
          setTagsKey(null);
        }}
        onSave={patchModTagsInstant}
        onRefresh={handleRefreshProviderLabels}
      />

      <DescriptionDialog
        mod={descriptionMod}
        modNav={descriptionMod ? modModalNav(descriptionMod) : undefined}
        savingKey={descriptionSavingKey}
        busy={busy}
        onClose={() => {
          if (busy) return;
          setDescriptionKey(null);
        }}
        onSave={patchModDescriptionInstant}
      />
    </main>
  );
}

export default App;
