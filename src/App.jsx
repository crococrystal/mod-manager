import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { X } from 'lucide-react';
import {
  bootstrapInstance,
  cancelBackgroundTask,
  copyModFiles,
  deleteModFiles,
  disableModFiles,
  enableModFiles,
  deleteCustomCover,
  getSettings,
  identifyModSources,
  refreshModAssets,
  refreshProviderLabels,
  saveSettings,
  scanMods,
  syncProviderData,
  updateModTags,
  uploadCover,
  installProviderVersion
} from './api.js';
import { CatalogInstallDialog } from './features/catalog/CatalogInstallDialog.jsx';
import { CatalogSearchPanel } from './features/catalog/CatalogSearchPanel.jsx';
import { collectInstalledProjectIds, isCatalogItemInstalled } from './features/catalog/catalogInstalledStatus.js';
import { useCatalogSearch } from './features/catalog/useCatalogSearch.js';
import { AppToolbar } from './components/AppToolbar.jsx';
import { StatsBar } from './components/StatsBar.jsx';
import { TitleBar } from './components/TitleBar.jsx';
import { NoticeModal } from './components/NoticeModal.jsx';
import { UpdateModal } from './components/UpdateModal.jsx';
import { DescriptionDialog } from './features/mods/DescriptionDialog.jsx';
import { ModContextMenu } from './features/mods/ModContextMenu.jsx';
import { ModEditor } from './features/mods/ModEditor.jsx';
import { ModTable } from './features/mods/ModTable.jsx';
import { ProviderDialog } from './features/mods/ProviderDialog.jsx';
import { TagsDialog } from './features/mods/TagsDialog.jsx';
import { VersionDialog } from './features/mods/VersionDialog.jsx';
import { UpdatesVersionPanel } from './features/mods/UpdatesVersionPanel.jsx';
import { SettingsDialog } from './features/settings/SettingsDialog.jsx';
import { useAppUpdater } from './hooks/useAppUpdater.js';
import {
  formatSingleUpdateConfirmMessage,
  formatUpdateAllConfirmMessage
} from './lib/updateConfirmMessages.js';
import { projectIdFor } from './features/mods/versionListUtils.js';
import { canCheckForUpdates } from './lib/updater.js';
import { readSettingsTab, writeSettingsTab } from './lib/settingsTabStorage.js';
import { buildWorkspaceFooter } from './features/updates/buildWorkspaceFooter.js';
import {
  buildServerSyncFooter,
  mergeWorkspaceFooters
} from './features/server-sync/buildServerSyncFooter.js';
import { ServerSyncDoneStats } from './features/server-sync/ServerSyncDoneStats.jsx';
import { useServerSyncProgress } from './features/server-sync/useServerSyncProgress.js';
import {
  shouldShowUpdatesPanel,
  updatesToolbarStatus,
  useUpdatesFeature
} from './features/updates/useUpdatesFeature.js';
import { modNeedsUpdate } from './features/updates/updateCandidateSync.js';
import { modByKey } from './lib/modMeta.jsx';
import { modMatchesWorkspaceFilters } from './lib/modFilters.js';
import { openExternalUrl } from './lib/openExternalUrl.js';
import { normalizeModsGraph } from './lib/usedBy.js';
import './styles/index.css';
import './styles/catalog.css';

const EMPTY_MODS = [];
const EMPTY_STATS = {};
const EMPTY_SET = new Set();

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

function App() {
  const [payload, setPayload] = useState(null);
  const [settings, setSettings] = useState(null);
  const [selected, setSelected] = useState(null);
  const [selectedKeys, setSelectedKeys] = useState(() => new Set());
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [busy, setBusy] = useState(false);
  const [bootstrapping, setBootstrapping] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState('');
  const [info, setInfo] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsTab, setSettingsTabState] = useState(() => readSettingsTab());
  const setSettingsTab = useCallback((tab) => {
    writeSettingsTab(tab);
    setSettingsTabState(tab);
  }, []);
  const serverSync = useServerSyncProgress();
  const [progress, setProgress] = useState(null);
  const [providerKey, setProviderKey] = useState(null);
  const [versionKey, setVersionKey] = useState(null);
  const [updateConfirm, setUpdateConfirm] = useState(null);
  const [updateInstallVersionId, setUpdateInstallVersionId] = useState(null);
  const [tagsKey, setTagsKey] = useState(null);
  const [tagsSavingKey, setTagsSavingKey] = useState(null);
  const [relationsKey, setRelationsKey] = useState(null);
  const [descriptionKey, setDescriptionKey] = useState(null);
  const [descriptionSavingKey, setDescriptionSavingKey] = useState(null);
  const [refreshingAssetsKey, setRefreshingAssetsKey] = useState(null);
  const [coverSavingKey, setCoverSavingKey] = useState(null);
  const [deleteConfirmMods, setDeleteConfirmMods] = useState([]);
  const [deletingModKeys, setDeletingModKeys] = useState(() => new Set());
  const [modContextMenu, setModContextMenu] = useState(null);
  const [labelsRefreshingKey, setLabelsRefreshingKey] = useState(null);
  const [sort, setSort] = useState({ key: 'name', direction: 'asc' });
  const watcherReloadingRef = useRef(false);
  const watcherReloadPendingRef = useRef(false);
  const selectionAnchorRef = useRef(null);
  const bootstrapRunRef = useRef(0);
  const syncRunRef = useRef(0);
  const syncingRef = useRef(false);
  const sourceIdentifyRunRef = useRef(0);
  const suppressModAutoSelectRef = useRef(false);
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
  const modsByKey = useMemo(() => modByKey(mods), [mods]);

  const applyPayload = useCallback((next) => {
    const normalized = withLocalCovers(next);
    setPayload(normalized);
    setSettings(normalized.settings);
    setSelected((current) => {
      if (!normalized.mods.length) return null;
      if (suppressModAutoSelectRef.current) {
        suppressModAutoSelectRef.current = false;
        return null;
      }
      if (!current?.filename) return normalized.mods[0];
      return normalized.mods.find((mod) => mod.filename === current.filename) ?? normalized.mods[0];
    });
  }, []);

  const updateModInPayload = useCallback((key, patch, { normalizeGraph = true } = {}) => {
    setPayload((current) => {
      if (!current) return current;
      let touched = false;
      const mods = current.mods.map((mod) => {
        if (mod.key !== key) return mod;
        touched = true;
        return { ...mod, ...patch };
      });
      if (!touched) return current;
      if (normalizeGraph) normalizeModsGraph(mods);
      return { ...current, mods, stats: statsForMods(mods) };
    });
    setSelected((current) => (current?.key === key ? { ...current, ...patch } : current));
  }, []);

  const finalizeModsGraph = useCallback(() => {
    setPayload((current) => {
      if (!current?.mods?.length) return current;
      const mods = current.mods.map((mod) => ({ ...mod }));
      normalizeModsGraph(mods);
      return { ...current, mods, stats: statsForMods(mods) };
    });
  }, []);

  const {
    workspaceInitializing,
    updatesInitBlocked,
    snapshot: updatesSnapshot,
    updateAllBusy,
    updateAllProgress,
    updateAllError,
    runModUpdatesInstall,
    updatesCandidates,
    updatesCheckedAtMs,
    updatesLoadingVisible,
    updatesFailedProjects,
    updatesError,
    updatesReady,
    removeUpdateCandidate,
    syncUpdateCandidates
  } = useUpdatesFeature({
    enabled: canShowWorkspace,
    instanceRoot: settings?.instanceRoot,
    syncing,
    scanning,
    busy,
    bootstrapping,
    updateInstallBusy: Boolean(updateInstallVersionId),
    modsByKey,
    cacheScope: settings?.instanceRoot,
    updateModInPayload,
    finalizeModsGraph,
    setSelected,
    setSelectedKeys,
    setError,
    setInfo
  });

  const {
    source: catalogSource,
    mode: catalogMode,
    results: catalogResults,
    target: catalogTarget,
    loading: catalogLoading,
    loadingMore: catalogLoadingMore,
    hasMore: catalogHasMore,
    error: catalogError,
    updatesBlocked: catalogUpdatesBlocked,
    installSelection: catalogInstallSelection,
    toggleSource: toggleSearchSource,
    clearQuery: clearCatalogQuery,
    reset: resetCatalogSearch,
    selectCandidate: selectCatalogCandidate,
    closeInstall: closeCatalogInstall,
    loadMore: loadMoreCatalogResults
  } = useCatalogSearch({
    query,
    setQuery,
    canSearch: canShowWorkspace,
    curseforgeApiKeySet: settings?.curseforgeApiKeySet,
    cacheScope: settings?.instanceRoot,
    updatesSnapshot
  });

  const catalogInstalledProjectIdsBySource = useMemo(
    () => ({
      modrinth: collectInstalledProjectIds(mods, 'modrinth'),
      curseforge: collectInstalledProjectIds(mods, 'curseforge')
    }),
    [mods]
  );

  const catalogInstalledProjectIds =
    catalogInstalledProjectIdsBySource[catalogSource] ?? catalogInstalledProjectIdsBySource.modrinth;

  const catalogInstallInstalledStateKey = useMemo(
    () =>
      mods
        .flatMap((mod) => [mod.modrinthId, mod.curseforgeId, mod.source, mod.displayName])
        .filter(Boolean)
        .sort((a, b) => a.localeCompare(b))
        .join('|'),
    [mods]
  );

  const catalogInstallAlreadyInstalled = useMemo(() => {
    const candidate = catalogInstallSelection?.candidate;
    const source = catalogInstallSelection?.source;
    if (!candidate?.id || !source) return false;
    const installedProjectIds =
      catalogInstalledProjectIdsBySource[source] ?? catalogInstalledProjectIdsBySource.modrinth;
    return isCatalogItemInstalled({
      catalogSource: source,
      item: candidate,
      mods,
      installedProjectIds
    });
  }, [catalogInstallSelection, catalogInstalledProjectIdsBySource, mods]);

  const catalogModForItem = useCallback(
    (item) => {
      if (catalogSource !== 'updates') return null;
      return modsByKey.get(item.key ?? item.id) ?? null;
    },
    [catalogSource, modsByKey]
  );

  const isUpdatesMode = catalogMode && catalogSource === 'updates';

  const updatesListMods = useMemo(() => {
    if (!isUpdatesMode) return [];
    return catalogResults
      .map((item) => modsByKey.get(item.key ?? item.id))
      .filter(Boolean);
  }, [isUpdatesMode, catalogResults, modsByKey]);

  const showUpdatesPanel = shouldShowUpdatesPanel({
    isUpdatesMode,
    updatesInitBlocked,
    updateAllBusy,
    updateInstallBusy: Boolean(updateInstallVersionId),
    updatesLoadingVisible,
    updatesCandidates,
    selectedKeys
  });
  const showAside = !catalogMode || (isUpdatesMode && showUpdatesPanel);
  const updatesStatus = updatesToolbarStatus({
    updatesInitBlocked,
    updatesLoadingVisible,
    updatesCheckedAtMs,
    updatesCandidates
  });

  const updatesSelectedMod =
    isUpdatesMode && selected ? modsByKey.get(selected.key) ?? selected : null;

  useEffect(() => {
    if (!isUpdatesMode) return;
    const candidateKeys = new Set(updatesCandidates.map((item) => item.key ?? item.id));
    setSelectedKeys((current) => {
      const next = new Set([...current].filter((key) => candidateKeys.has(key)));
      return next.size === current.size ? current : next;
    });
    setSelected((current) => (current && candidateKeys.has(current.key) ? current : null));
  }, [isUpdatesMode, updatesCandidates]);

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
    if (isUpdatesMode) return;
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
  }, [visible, selected, isUpdatesMode]);

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

  const deleteConfirmFreshMods = useMemo(() => {
    const byKey = new Map(mods.map((item) => [item.key, item]));
    return deleteConfirmMods
      .map((item) => byKey.get(item.key) ?? item)
      .filter((item) => item?.key);
  }, [mods, deleteConfirmMods]);

  const requestDeleteMods = useCallback(
    (items) => {
      const byKey = new Map(mods.map((item) => [item.key, item]));
      const freshMods = [...new Map(
        items
          .map((item) => byKey.get(item.key) ?? item)
          .filter((item) => item?.key)
          .map((item) => [item.key, item])
      ).values()];
      if (!freshMods.length) return;

      const deletingKeys = new Set(freshMods.map((item) => item.key));
      const blockers = freshMods.flatMap((item) =>
        (item.usedBy ?? [])
          .filter((key) => !deletingKeys.has(key))
          .map((key) => byKey.get(key)?.displayName ?? key)
      );
      if (blockers.length) {
        const target = freshMods.length === 1 ? freshMods[0].displayName : `${freshMods.length} модов`;
        setError(`Нельзя удалить ${target}: используется для ${[...new Set(blockers)].join(', ')}.`);
        setModContextMenu(null);
        return;
      }

      setDeleteConfirmMods(freshMods);
      setModContextMenu(null);
    },
    [mods]
  );

  const confirmDeleteMods = useCallback(async () => {
    const keys = deleteConfirmFreshMods.map((item) => item.key);
    if (!keys.length) return;

    setDeletingModKeys(new Set(keys));
    setError('');
    try {
      await deleteModFiles(keys);
      const next = await scanMods();
      applyPayload(next);
      setInfo(
        keys.length === 1
          ? `Мод удалён: ${deleteConfirmFreshMods[0].displayName}.`
          : `Удалено модов: ${keys.length}.`
      );
      setDeleteConfirmMods([]);
    } catch (err) {
      setError(String(err));
    } finally {
      setDeletingModKeys(new Set());
    }
  }, [applyPayload, deleteConfirmFreshMods]);

  const contextMenuMods = useMemo(() => {
    if (!modContextMenu?.keys?.length) return [];
    const byKey = new Map(mods.map((item) => [item.key, item]));
    return modContextMenu.keys.map((key) => byKey.get(key)).filter(Boolean);
  }, [mods, modContextMenu]);
  const contextMenuLabel = contextMenuMods.length === 1 ? contextMenuMods[0].displayName : '';
  const contextMenuPageUrl =
    modContextMenu?.mode === 'updates' || contextMenuMods.length !== 1
      ? null
      : contextMenuMods[0].sourceUrl;
  const contextMenuCanDisable = contextMenuMods.some((item) => !item.disabled);
  const contextMenuCanEnable = contextMenuMods.some((item) => item.disabled);

  const closeModContextMenu = useCallback(() => {
    setModContextMenu(null);
  }, []);

  const handleModContextMenu = useCallback(
    (mod, event) => {
      event.preventDefault();
      event.stopPropagation();
      if (!mod) return;

      const fresh = mods.find((item) => item.key === mod.key) ?? mod;
      const keepSelection = selectedKeys.has(fresh.key) && selectedKeys.size > 1;
      const keys = keepSelection ? [...selectedKeys] : [fresh.key];
      if (!keepSelection) {
        setSelected(fresh);
        selectionAnchorRef.current = fresh.key;
        setSelectedKeys(new Set([fresh.key]));
      }
      setModContextMenu({ x: event.clientX, y: event.clientY, keys, mode: isUpdatesMode ? 'updates' : undefined });
    },
    [isUpdatesMode, mods, selectedKeys]
  );

  const handleContextCopyMods = useCallback(() => {
    const keys = contextMenuMods.map((item) => item.key);
    if (keys.length) void copyModKeys(keys);
    setModContextMenu(null);
  }, [contextMenuMods, copyModKeys]);

  const handleContextOpenPage = useCallback(() => {
    if (contextMenuPageUrl) void openExternalUrl(contextMenuPageUrl);
    setModContextMenu(null);
  }, [contextMenuPageUrl]);

  const handleContextDeleteMods = useCallback(() => {
    requestDeleteMods(contextMenuMods);
  }, [contextMenuMods, requestDeleteMods]);

  const handleContextDisableMods = useCallback(async () => {
    const keys = contextMenuMods.filter((item) => !item.disabled).map((item) => item.key);
    if (!keys.length) return;
    setModContextMenu(null);
    setBusy(true);
    setError('');
    try {
      await disableModFiles(keys);
      const next = await scanMods();
      applyPayload(next);
      setInfo(
        keys.length === 1
          ? `Мод отключён: ${contextMenuMods.find((item) => item.key === keys[0])?.displayName ?? 'мод'}.`
          : `Отключено модов: ${keys.length}.`
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [applyPayload, contextMenuMods]);

  const handleContextEnableMods = useCallback(async () => {
    const keys = contextMenuMods.filter((item) => item.disabled).map((item) => item.key);
    if (!keys.length) return;
    setModContextMenu(null);
    setBusy(true);
    setError('');
    try {
      await enableModFiles(keys);
      const next = await scanMods();
      applyPayload(next);
      setInfo(
        keys.length === 1
          ? `Мод включён: ${contextMenuMods.find((item) => item.key === keys[0])?.displayName ?? 'мод'}.`
          : `Включено модов: ${keys.length}.`
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [applyPayload, contextMenuMods]);

  const handleContextUpdateMods = useCallback(() => {
    const keys = contextMenuMods.map((item) => item.key);
    if (!keys.length) return;
    setModContextMenu(null);
    setUpdateConfirm({ scope: 'selected', keys });
  }, [contextMenuMods]);

  useEffect(() => {
    function handleKeyDown(event) {
      const target = event.target;
      if (isTextEntryTarget(target) || hasActiveTextSelection()) {
        return;
      }

      const command = event.metaKey || event.ctrlKey;
      if (command && event.key.toLowerCase() === 'a' && canShowWorkspace) {
        if (!isUpdatesMode && target instanceof HTMLElement && target.closest('.descriptionCell')) {
          return;
        }
        event.preventDefault();
        const list = isUpdatesMode ? updatesListMods : visible;
        setSelectedKeys(new Set(list.map((mod) => mod.key)));
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
  }, [canShowWorkspace, copyModKeys, isUpdatesMode, moveSelection, selected?.key, selectedKeys, updatesListMods, visible]);

  useEffect(() => {
    function suppressNativeContextMenu(event) {
      if (isTextEntryTarget(event.target)) return;
      event.preventDefault();
    }

    window.addEventListener('contextmenu', suppressNativeContextMenu);
    return () => window.removeEventListener('contextmenu', suppressNativeContextMenu);
  }, []);

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
        suppressModAutoSelectRef.current = true;
        setProgress(null);
        setSelected(null);
        setSelectedKeys(new Set());
        resetCatalogSearch();
      }
      setBusy(true);
      setError('');
      try {
        const saved = await saveSettings(nextSettings);
        setSettings(saved);
        const shouldRefreshProviderLinks = Boolean(saved.instanceRoot && curseForgeKeyChanged);
        let providerLinksCoveredByBootstrap = false;
        if (options.scan !== false && saved.instanceRoot) {
          if (instanceChanged) {
            setScanning(true);
          }
          try {
            const next = await scanMods();
            applyPayload(next);
            if (instanceChanged) {
              suppressModAutoSelectRef.current = true;
              setSelected(null);
              setSelectedKeys(new Set());
            }
            if (options.bootstrap && saved.instanceRoot) {
              providerLinksCoveredByBootstrap = needsBootstrap(saved.cacheStatus, {
                force: options.forceBootstrap
              });
              if (providerLinksCoveredByBootstrap) {
                setBootstrapping(true);
                void runBootstrap(saved.cacheStatus, { force: options.forceBootstrap });
              }
            } else if (needsBootstrap(saved.cacheStatus)) {
              providerLinksCoveredByBootstrap = true;
              setBootstrapping(true);
              void runBootstrap(saved.cacheStatus);
            }
          } finally {
            if (instanceChanged) {
              setScanning(false);
            }
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
    [applyPayload, resetCatalogSearch, runBootstrap, settings?.curseforgeApiKey, settings?.instanceRoot]
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
    setScanning(true);
    setError('');
    try {
      const saved = await getSettings();
      setSettings(saved);
      if (saved.instanceRoot) {
        const next = await scanMods();
        applyPayload(next);
        void runBootstrap(saved.cacheStatus, { force: true });
      }
    } catch (err) {
      setError(String(err));
      throw err;
    } finally {
      setScanning(false);
      setBusy(false);
    }
  }, [applyPayload, runBootstrap]);

  const handleCancelProgress = useCallback(() => {
    if (
      (!settingsOpen || settingsTab !== 'server') &&
      (serverSync.server.syncing || serverSync.distribution.syncing)
    ) {
      if (serverSync.server.syncing) void serverSync.cancel('server');
      if (serverSync.distribution.syncing) void serverSync.cancel('distribution');
      return;
    }
    if (!bootstrapping && !syncing) return;
    bootstrapRunRef.current += 1;
    syncRunRef.current += 1;
    setBootstrapping(false);
    setSyncing(false);
    setProgress(null);
    void cancelBackgroundTask();
  }, [bootstrapping, serverSync, settingsOpen, settingsTab, syncing]);

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

  const handleUpdatesVersionInstalled = useCallback(
    ({ key, ...patch }) => {
      updateModInPayload(key, patch);
      setSelected((current) => (current?.key === key ? null : current));
      setSelectedKeys((current) => {
        if (!current.has(key)) return current;
        const next = new Set(current);
        next.delete(key);
        return next;
      });
      const base = modsByKey.get(key);
      const updatedMod = base ? { ...base, ...patch } : { key, ...patch };
      if (!modNeedsUpdate(updatedMod, settings?.instanceRoot)) {
        removeUpdateCandidate(key);
      }
    },
    [modsByKey, removeUpdateCandidate, settings?.instanceRoot, updateModInPayload]
  );

  useEffect(() => {
    if (!isUpdatesMode || updatesLoadingVisible) return;
    syncUpdateCandidates(modsByKey, settings?.instanceRoot);
  }, [
    isUpdatesMode,
    modsByKey,
    settings?.instanceRoot,
    syncUpdateCandidates,
    updatesCandidates,
    updatesLoadingVisible
  ]);

  const handleToggleSearchSource = useCallback(
    (nextSource) => {
      if (nextSource === 'updates') {
        if (catalogSource !== 'updates') {
          setQuery('');
          setSelected(null);
          setSelectedKeys(new Set());
        }
      } else if (catalogSource === 'updates') {
        setSelected(null);
        setSelectedKeys(new Set());
      }
      toggleSearchSource(nextSource);
    },
    [catalogSource, setQuery, toggleSearchSource]
  );

  const handleUpdateAllMods = useCallback(async () => {
    await runModUpdatesInstall(updatesCandidates);
  }, [runModUpdatesInstall, updatesCandidates]);

  const handleUpdateSelectedMods = useCallback(
    async (keys) => {
      const keySet = new Set(keys);
      const candidates = updatesCandidates.filter((item) => keySet.has(item.key ?? item.id));
      await runModUpdatesInstall(candidates);
    },
    [runModUpdatesInstall, updatesCandidates]
  );

  const performUpdatesVersionInstall = useCallback(
    async (mod, version) => {
      const projectId = projectIdFor(mod);
      if (!mod || !version || !projectId) return;

      setUpdateInstallVersionId(version.id);
      setError('');

      try {
        const result = await installProviderVersion({
          key: mod.key,
          source: mod.source,
          projectId,
          filename: mod.filename,
          versionId: version.id,
          fileId: version.fileId ?? undefined,
          downloadUrl: version.downloadUrl ?? undefined,
          downloadFilename: version.filename,
          versionNumber: version.versionNumber
        });
        handleUpdatesVersionInstalled(result);
      } catch (err) {
        setError(String(err));
      } finally {
        setUpdateInstallVersionId(null);
      }
    },
    [handleUpdatesVersionInstalled, settings?.instanceRoot]
  );

  const updateConfirmBusy = updateAllBusy || Boolean(updateInstallVersionId);

  const updateConfirmMessage = useMemo(() => {
    if (!updateConfirm) return '';
    if (updateConfirm.scope === 'all') {
      return formatUpdateAllConfirmMessage(updatesCandidates.length);
    }
    if (updateConfirm.scope === 'selected') {
      return formatUpdateAllConfirmMessage(updateConfirm.keys.length);
    }
    return formatSingleUpdateConfirmMessage(updateConfirm.mod, updateConfirm.version);
  }, [updateConfirm, updatesCandidates.length]);

  const confirmPendingUpdate = useCallback(async () => {
    if (!updateConfirm || updateConfirmBusy) return;
    const pending = updateConfirm;
    setUpdateConfirm(null);
    if (pending.scope === 'all') {
      await handleUpdateAllMods();
      return;
    }
    if (pending.scope === 'selected') {
      await handleUpdateSelectedMods(pending.keys);
      return;
    }
    await performUpdatesVersionInstall(pending.mod, pending.version);
  }, [
    handleUpdateAllMods,
    handleUpdateSelectedMods,
    performUpdatesVersionInstall,
    updateConfirm,
    updateConfirmBusy
  ]);

  const handleCatalogInstalled = useCallback(
    async (result) => {
      closeCatalogInstall();
      setBusy(true);
      setError('');
      try {
        let next = await scanMods();
        const installedKeys = [...new Set(result?.installedKeys ?? [])];
        if (installedKeys.length) {
          await Promise.allSettled(installedKeys.map((key) => refreshModAssets(key)));
          next = await scanMods();
        }
        applyPayload(next);
        const installed =
          next.mods?.find((mod) => mod.key === result?.mainKey) ?? next.mods?.[0] ?? null;
        if (installed) {
          setSelected(installed);
          resetCatalogSearch();
        }
        setInfo('Мод установлен.');
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyPayload, closeCatalogInstall, resetCatalogSearch]
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
  const handleTableSelectDrag = useCallback((mod, select, options = {}) => {
    setSelected(mod);
    setRelationsKey((current) => (current ? mod.key : current));
    selectionAnchorRef.current = mod.key;
    setSelectedKeys((current) => {
      if (options.reset) {
        return new Set([mod.key]);
      }
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
      if (!isUpdatesMode) {
        setRelationsKey((current) => (current ? mod.key : current));
      }

      const list = isUpdatesMode ? updatesListMods : visible;

      if (event?.shiftKey) {
        const anchorKey = selectionAnchorRef.current ?? selected?.key;
        if (anchorKey) {
          const anchorIndex = list.findIndex((item) => item.key === anchorKey);
          const targetIndex = list.findIndex((item) => item.key === mod.key);
          if (anchorIndex >= 0 && targetIndex >= 0) {
            const start = Math.min(anchorIndex, targetIndex);
            const end = Math.max(anchorIndex, targetIndex);
            setSelectedKeys(new Set(list.slice(start, end + 1).map((item) => item.key)));
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

      if (isUpdatesMode && selectedKeys.size === 1 && selectedKeys.has(mod.key)) {
        setSelected(null);
        selectionAnchorRef.current = null;
        setSelectedKeys(new Set());
        return;
      }

      selectionAnchorRef.current = mod.key;
      setSelectedKeys(new Set([mod.key]));
    },
    [isUpdatesMode, selectedKeys, selected?.key, updatesListMods, visible]
  );

  const updatesCenterLoadingVisible =
    isUpdatesMode &&
    catalogLoading &&
    catalogResults.length === 0 &&
    !updateAllBusy;

  const serverSyncFooter = buildServerSyncFooter({
    enabled: !settingsOpen || settingsTab !== 'server',
    server: serverSync.server,
    distribution: serverSync.distribution,
    visibleResult: serverSync.visibleResult
  });

  const workspaceFooter = mergeWorkspaceFooters(
    buildWorkspaceFooter({
      workspaceInitializing,
      bootstrapping,
      syncing,
      scanning,
      progress,
      updateAllBusy,
      updateAllProgress,
      updatesError,
      updateAllError,
      updatesFailedProjects,
      updatesLoadingVisible,
      isUpdatesMode,
      updatesCenterLoadingVisible
    }),
    serverSyncFooter
  );
  const uiLocked = busy || updateAllBusy;
  const deleteBusy = deletingModKeys.size > 0;
  const deleteConfirmMessage =
    deleteConfirmFreshMods.length === 1
      ? `Удалить ${deleteConfirmFreshMods[0].displayName}? Файл будет удалён из папки mods.`
      : deleteConfirmFreshMods.length > 1
      ? `Удалить ${deleteConfirmFreshMods.length} модов? Файлы будут удалены из папки mods.`
      : '';

  const toolbar = (
    <AppToolbar
      canShowWorkspace={canShowWorkspace}
      query={query}
      searchSource={catalogSource}
      filter={filter}
      settingsOpen={settingsOpen}
      busy={busy || updateAllBusy}
      updatesLoading={updatesLoadingVisible}
      updatesStatus={updatesStatus}
      onQueryChange={setQuery}
      onClearQuery={clearCatalogQuery}
      onToggleSearchSource={handleToggleSearchSource}
      onFilterChange={setFilter}
      onOpenSettings={() => setSettingsOpen(true)}
    />
  );

  return (
    <main className="appShell">
      <TitleBar>{toolbar}</TitleBar>

      <div className="appBody">
      {canShowWorkspace ? (
        <>
          <StatsBar stats={stats} />

          <NoticeModal tone="bad" message={error} onClose={() => setError('')} />
          <NoticeModal tone="ok" message={info && !error && !bootstrapping && !syncing ? info : ''} onClose={() => setInfo('')} />
          <NoticeModal
            tone="bad"
            message={deleteConfirmMessage}
            onClose={() => setDeleteConfirmMods([])}
            confirm={
              deleteConfirmFreshMods.length
                ? {
                    busy: deleteBusy,
                    confirmLabel: deleteBusy ? 'Удаление...' : 'Удалить',
                    cancelLabel: 'Отмена',
                    onConfirm: confirmDeleteMods,
                    onCancel: () => setDeleteConfirmMods([])
                  }
                : null
            }
          />
          <NoticeModal
            tone="bad"
            message={updateConfirmMessage}
            onClose={() => setUpdateConfirm(null)}
            confirm={
              updateConfirm
                ? {
                    busy: updateConfirmBusy,
                    confirmLabel: updateConfirmBusy ? 'Обновление…' : 'Обновить',
                    cancelLabel: 'Отмена',
                    onConfirm: confirmPendingUpdate,
                    onCancel: () => setUpdateConfirm(null)
                  }
                : null
            }
          />

          <section
            className={`workspace${catalogMode && (!isUpdatesMode || !showUpdatesPanel) ? ' workspaceCatalog' : ''}${
              showUpdatesPanel ? ' workspaceUpdates' : ''
            }`}
          >
            {catalogMode ? (
              <div className="catalogSearchWrap">
                <CatalogSearchPanel
                  source={catalogSource}
                  target={catalogTarget}
                  results={catalogResults}
                  loading={catalogLoading}
                  loadingMore={catalogLoadingMore}
                  hasMore={catalogHasMore}
                  error={catalogError}
                  updatesBlocked={catalogUpdatesBlocked}
                  updatesCheckedAtMs={updatesInitBlocked || !updatesReady ? null : updatesCheckedAtMs}
                  updatesReady={updatesReady && !updatesInitBlocked}
                  updatesLoading={updatesLoadingVisible}
                  query={query}
                  installedProjectIds={catalogInstalledProjectIds}
                  installedMods={mods}
                  modForItem={catalogModForItem}
                  selectedKey={isUpdatesMode ? selected?.key : null}
                  selectedKeys={isUpdatesMode ? selectedKeys : null}
                  onSelect={(item, event) => {
                    if (catalogSource === 'updates') {
                      const mod = modsByKey.get(item.key ?? item.id);
                      if (mod) handleTableSelect(mod, event);
                      return;
                    }
                    selectCatalogCandidate(item);
                  }}
                  onSelectDrag={
                    isUpdatesMode
                      ? (item, select, options) => {
                          const mod = modsByKey.get(item.key ?? item.id);
                          if (mod) handleTableSelectDrag(mod, select, options);
                        }
                      : undefined
                  }
                  onContextMenu={
                    isUpdatesMode
                      ? (item, event) => {
                          const mod = modsByKey.get(item.key ?? item.id);
                          if (mod) handleModContextMenu(mod, event);
                        }
                      : undefined
                  }
                  onLoadMore={catalogSource === 'updates' ? undefined : loadMoreCatalogResults}
                />
              </div>
            ) : (
              <ModTable
                mods={visible}
                selected={selected}
                selectedKeys={selectedKeys}
                sort={sort}
                onSort={handleSort}
                onSelect={handleTableSelect}
                onSelectDrag={handleTableSelectDrag}
                onContextMenu={handleModContextMenu}
                onCoverClick={openRelationsForMod}
                onSourceClick={(mod) => setProviderKey(mod.key)}
                onVersionClick={(mod) => setVersionKey(mod.key)}
                onTagsClick={(mod) => setTagsKey(mod.key)}
                onDescriptionClick={(mod) => setDescriptionKey(mod.key)}
              />
            )}
            {!showAside ? null : (
            <aside>
              {showUpdatesPanel ? (
                <UpdatesVersionPanel
                  mod={updatesSelectedMod}
                  cacheScope={settings?.instanceRoot}
                  busy={busy || updateAllBusy || Boolean(updateInstallVersionId)}
                  updateCount={updatesCandidates.length}
                  updatingAll={updateAllBusy}
                  updatesLoading={updatesLoadingVisible}
                  updateAllError={updateAllError}
                  installingVersionId={updateInstallVersionId}
                  onUpdateAllRequest={() => {
                    if (updatesCandidates.length && !busy && !updateAllBusy) {
                      setUpdateConfirm({ scope: 'all' });
                    }
                  }}
                  onInstallRequest={(version) => {
                    if (!updatesSelectedMod || busy || updateAllBusy || updateInstallVersionId) return;
                    setUpdateConfirm({ scope: 'single', mod: updatesSelectedMod, version });
                  }}
                  onClearMod={() => {
                    if (busy || updateAllBusy || updateInstallVersionId) return;
                    setSelected(null);
                    setSelectedKeys(new Set());
                  }}
                />
              ) : selected && !isUpdatesMode ? (
                <ModEditor
                  mod={selected}
                  mods={mods}
                  busy={busy || updateAllBusy}
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
            )}
          </section>
        </>
      ) : (
        <section className="setupState">
          <h2>Выбери сборку</h2>
          <p>Укажи папку сборки с minecraft/mods — обложки и зависимости подтянутся один раз в фоне.</p>
          <button type="button" onClick={() => setSettingsOpen(true)} disabled={busy || bootstrapping || syncing}>
            Открыть настройки
          </button>
        </section>
      )}
      </div>

      <ModContextMenu
        menu={modContextMenu}
        count={contextMenuMods.length}
        label={contextMenuLabel}
        busy={busy || deleteBusy || updateConfirmBusy}
        onClose={closeModContextMenu}
        onCopy={handleContextCopyMods}
        onOpenPage={contextMenuPageUrl ? handleContextOpenPage : undefined}
        onDisable={
          modContextMenu?.mode === 'updates' || !contextMenuCanDisable
            ? undefined
            : handleContextDisableMods
        }
        onEnable={
          modContextMenu?.mode === 'updates' || !contextMenuCanEnable
            ? undefined
            : handleContextEnableMods
        }
        onUpdate={modContextMenu?.mode === 'updates' ? handleContextUpdateMods : undefined}
        onDelete={handleContextDeleteMods}
      />

      {workspaceFooter.show ? (
        <footer className="prefetchProgressWrap">
          {workspaceFooter.trackActive ? (
            <div className="prefetchProgressTrack" aria-hidden="true">
              {workspaceFooter.scopedProgress ? (
                <div
                  className="prefetchProgressScope"
                  style={{ width: `${workspaceFooter.percent}%` }}
                >
                  <div className="prefetchProgressBar indeterminate" />
                </div>
              ) : workspaceFooter.indeterminate ? (
                <div className="prefetchProgressBar indeterminate" />
              ) : null}
            </div>
          ) : null}
          <div className="prefetchProgressRow">
            <p className={`prefetchProgressLabel${workspaceFooter.labelWarn ? ' prefetchProgressLabel--warn' : ''}`}>
              {workspaceFooter.label}
            </p>
            {workspaceFooter.stats ? (
              <ServerSyncDoneStats
                uploadCount={workspaceFooter.stats.uploadCount}
                updateCount={workspaceFooter.stats.updateCount}
                deleteCount={workspaceFooter.stats.deleteCount}
                skipCount={workspaceFooter.stats.skipCount}
                uploadFiles={workspaceFooter.stats.uploadFiles}
                skipFiles={workspaceFooter.stats.skipFiles}
                deleteFiles={workspaceFooter.stats.deleteFiles}
                updatePairs={workspaceFooter.stats.updatePairs}
              />
            ) : null}
            {workspaceFooter.cancelable ? (
              <button
                type="button"
                className="prefetchProgressCancel"
                onClick={handleCancelProgress}
                aria-label="Отменить"
                title="Отменить"
              >
                <X size={14} strokeWidth={2} />
              </button>
            ) : null}
          </div>
        </footer>
      ) : null}

      {settingsOpen ? (
        <SettingsDialog
          settings={settings}
          busy={uiLocked}
          syncing={syncing}
          updater={updater}
          serverSync={serverSync}
          tab={settingsTab}
          onTabChange={setSettingsTab}
          onClose={() => setSettingsOpen(false)}
          onSave={handleSaveSettings}
          onSettingsSaved={setSettings}
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

      <CatalogInstallDialog
        candidate={catalogInstallSelection?.candidate}
        source={catalogInstallSelection?.source}
        busy={busy || updateAllBusy}
        cacheScope={settings?.instanceRoot}
        alreadyInstalled={catalogInstallAlreadyInstalled}
        installedStateKey={catalogInstallInstalledStateKey}
        onClose={() => !busy && closeCatalogInstall()}
        onInstalled={handleCatalogInstalled}
      />

      <ProviderDialog
        mod={providerMod}
        modNav={providerMod ? modModalNav(providerMod) : undefined}
        busy={busy || updateAllBusy}
        curseforgeApiKeySet={settings?.curseforgeApiKeySet}
        onClose={() => !busy && setProviderKey(null)}
        onApplied={handleProviderApplied}
      />

      <VersionDialog
        mod={versionMod}
        modNav={versionMod ? modModalNav(versionMod) : undefined}
        busy={busy || updateAllBusy}
        cacheScope={settings?.instanceRoot}
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
        busy={busy || updateAllBusy}
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
