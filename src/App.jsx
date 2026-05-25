import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Search, Settings, SlidersHorizontal } from 'lucide-react';
import {
  bootstrapInstance,
  copyModFiles,
  deleteCustomCover,
  getSettings,
  saveSettings,
  scanMods,
  switchModSource,
  updateModTags,
  uploadCover
} from './api.js';
import { IconButton } from './components/Button.jsx';
import { TitleBar } from './components/TitleBar.jsx';
import { NoticeModal } from './components/NoticeModal.jsx';
import { ModEditor } from './features/mods/ModEditor.jsx';
import { ModTable } from './features/mods/ModTable.jsx';
import { ProviderDialog } from './features/mods/ProviderDialog.jsx';
import { SettingsDialog } from './features/settings/SettingsDialog.jsx';
import { filters } from './lib/modMeta.jsx';
import { normalizeModsGraph } from './lib/usedBy.js';
import './styles/index.css';

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

function withLocalCovers(next) {
  const mods = (next.mods ?? []).map((mod) => ({
    ...mod,
    coverUrl: coverUrlFor(mod)
  }));
  normalizeModsGraph(mods);
  return { ...next, mods };
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
  const [error, setError] = useState('');
  const [info, setInfo] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [progress, setProgress] = useState(null);
  const [providerKey, setProviderKey] = useState(null);
  const watcherReloadingRef = useRef(false);
  const watcherReloadPendingRef = useRef(false);

  const mods = payload?.mods ?? [];
  const stats = payload?.stats ?? {};

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return mods.filter((mod) => {
      const matchesQuery =
        !needle ||
        `${mod.displayName} ${mod.filename} ${mod.description ?? ''}`.toLowerCase().includes(needle);
      const matchesFilter =
        filter === 'all' ||
        (filter === 'library' && mod.library) ||
        (filter === 'technical' && mod.technical) ||
        mod.side === filter;
      return matchesQuery && matchesFilter;
    });
  }, [mods, query, filter]);

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
      return { ...current, mods };
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

      setBootstrapping(true);
      setError('');
      try {
        const result = await bootstrapInstance(force);
        if (!result?.skipped) {
          await reload({ silent: true });
        }
      } catch (err) {
        setError(String(err));
      } finally {
        setBootstrapping(false);
        setProgress(null);
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
    if (!visible.length) {
      setSelected(null);
      setSelectedKeys(new Set());
      return;
    }
    if (!selected || !visible.some((mod) => mod.filename === selected.filename)) {
      setSelected(visible[0]);
      setSelectedKeys(new Set([visible[0].key]));
    }
  }, [visible, selected]);

  useEffect(() => {
    const visibleKeys = new Set(visible.map((mod) => mod.key));
    setSelectedKeys((current) => {
      const next = new Set([...current].filter((key) => visibleKeys.has(key)));
      if (!next.size && selected?.key && visibleKeys.has(selected.key)) {
        next.add(selected.key);
      }
      const same = next.size === current.size && [...next].every((key) => current.has(key));
      return same ? current : next;
    });
  }, [selected?.key, visible]);

  const moveSelection = useCallback(
    (delta) => {
      if (!visible.length) return;
      const index = visible.findIndex((mod) => mod.filename === selected?.filename);
      const nextIndex =
        index < 0
          ? delta > 0
            ? 0
            : visible.length - 1
          : (index + delta + visible.length) % visible.length;
      const next = visible[nextIndex];
      setSelected(next);
      setSelectedKeys(new Set([next.key]));
      setRelationsKey((current) => (current ? next.key : current));
    },
    [visible, selected]
  );

  const copySelectedFiles = useCallback(async () => {
    const keys = [...selectedKeys];
    if (!keys.length) return;
    setError('');
    try {
      const count = await copyModFiles(keys);
      setInfo(`Скопировано файлов: ${count}.`);
    } catch (err) {
      setError(String(err));
    }
  }, [selectedKeys]);

  useEffect(() => {
    function handleKeyDown(event) {
      const target = event.target;
      if (target instanceof HTMLElement && target.closest('input, textarea, select, [contenteditable="true"]')) {
        return;
      }

      const command = event.metaKey || event.ctrlKey;
      if (command && event.key.toLowerCase() === 'a' && canShowWorkspace) {
        event.preventDefault();
        const keys = visible.map((mod) => mod.key);
        setSelectedKeys(new Set(keys));
        if (visible.length && !selected) {
          setSelected(visible[0]);
        }
        return;
      }

      if (command && event.key.toLowerCase() === 'c' && canShowWorkspace) {
        if (selectedKeys.size) {
          event.preventDefault();
          void copySelectedFiles();
        }
        return;
      }

      if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
      event.preventDefault();
      moveSelection(event.key === 'ArrowUp' ? -1 : 1);
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [canShowWorkspace, copySelectedFiles, moveSelection, selected, selectedKeys, visible]);

  useEffect(() => {
    let unlistenMods;
    (async () => {
      unlistenMods = await listen('mods-folder-changed', async () => {
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
    (async () => {
      unlistenProgress = await listen('prefetch-progress', (event) => {
        const payload = event.payload;
        if (payload?.status === 'done') {
          setProgress(null);
          return;
        }
        setProgress(payload);
      });
      unlistenCover = await listen('cover-ready', (event) => {
        const { key, coverPath, coverModifiedAt } = event.payload ?? {};
        if (!key || !coverPath) return;
        const base = convertFileSrc(coverPath);
        const coverUrl = coverModifiedAt ? `${base}?v=${coverModifiedAt}` : base;
        updateModInPayload(key, { coverPath, coverUrl, coverModifiedAt, coverManual: false });
      });
      unlistenDependencies = await listen('dependencies-ready', (event) => {
        const { key, dependencies } = event.payload ?? {};
        if (!key || !Array.isArray(dependencies)) return;
        updateModInPayload(key, { dependencies });
      });
    })();
    return () => {
      unlistenProgress?.();
      unlistenCover?.();
      unlistenDependencies?.();
    };
  }, [updateModInPayload]);

  const handleSaveSettings = useCallback(
    async (nextSettings, options = {}) => {
      setBusy(true);
      setError('');
      try {
        const saved = await saveSettings(nextSettings);
        setSettings(saved);
        if (options.scan !== false && saved.instanceRoot) {
          const next = await scanMods();
          applyPayload(next);
          if (options.bootstrap && saved.instanceRoot) {
            void runBootstrap(saved.cacheStatus, { force: options.forceBootstrap });
          } else if (needsBootstrap(saved.cacheStatus)) {
            void runBootstrap(saved.cacheStatus);
          }
        }
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyPayload, runBootstrap]
  );

  const patchMod = useCallback(
    async (key, patch) => {
      setBusy(true);
      setError('');
      try {
        const next = await updateModTags({ key, ...patch });
        applyPayload(next);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyPayload]
  );

  const handleUploadCover = useCallback(
    async (key, dataUrl) => {
      setBusy(true);
      setError('');
      try {
        const next = await uploadCover({ key, dataUrl });
        applyPayload(next);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyPayload]
  );

  const handleDeleteCover = useCallback(
    async (key) => {
      setBusy(true);
      setError('');
      try {
        const next = await deleteCustomCover(key);
        applyPayload(next);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyPayload]
  );

  const handleSwitchSource = useCallback(
    async (source) => {
      if (!providerKey) return;
      setBusy(true);
      setError('');
      try {
        const next = await switchModSource({ key: providerKey, source });
        applyPayload(next);
        setProviderKey(null);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [applyPayload, providerKey]
  );

  const [relationsKey, setRelationsKey] = useState(null);
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
  const handleSelectMod = useCallback(
    (mod) => {
      if (!mod) return;
      const fresh = mods.find((item) => item.filename === mod.filename) ?? mod;
      setSelected(fresh);
      setRelationsKey((current) => (current ? fresh.key : current));
    },
    [mods]
  );
  const handleTableSelect = useCallback((mod) => {
    setSelected(mod);
    setSelectedKeys(new Set([mod.key]));
    setRelationsKey((current) => (current ? mod.key : current));
  }, []);

  const canShowWorkspace = Boolean(settings?.instanceRoot);
  const progressPercent =
    progress?.total > 0 ? Math.min(100, Math.round((progress.index / progress.total) * 100)) : 0;
  const uiLocked = busy;

  const progressLabel =
    bootstrapping && progress
      ? `${progress.kind === 'covers' ? 'Обложки' : progress.kind === 'dependencies' ? 'Зависимости' : 'Моды'} · ${progress.index}/${progress.total}${
          progress.name ? ` · ${progress.name}` : ''
        }`
      : '';

  const toolbar = canShowWorkspace ? (
    <div className="topToolbar" data-tauri-drag-region>
      <label className="search" data-tauri-drag-region="false">
        <Search size={14} />
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Поиск по названию или файлу"
          data-tauri-drag-region="false"
        />
      </label>
      <div className="segments" data-tauri-drag-region>
        {filters.map((item) => {
          const Icon = item.icon ?? SlidersHorizontal;
          return (
            <button
              key={item.id}
              className={filter === item.id ? 'active' : ''}
              onClick={() => setFilter(item.id)}
              type="button"
              disabled={busy}
              data-tauri-drag-region="false"
            >
              {item.icon ? <Icon className={`tagIcon ${item.tone}`} size={13} /> : null}
              <span>{item.label}</span>
            </button>
          );
        })}
      </div>
      <div className="topActions">
        <IconButton
          icon={Settings}
          label="Настройки"
          onClick={() => setSettingsOpen(true)}
          disabled={busy || bootstrapping}
        />
      </div>
    </div>
  ) : (
    <div className="topToolbar topToolbarEmpty" data-tauri-drag-region>
      <span className="titleBarName" data-tauri-drag-region>Mod Manager</span>
      <IconButton
        icon={Settings}
        label="Настройки"
        onClick={() => setSettingsOpen(true)}
        disabled={busy || bootstrapping}
      />
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
          <NoticeModal tone="ok" message={info && !error && !bootstrapping ? info : ''} onClose={() => setInfo('')} />

          <section className="workspace">
            <ModTable
              mods={visible}
              selected={selected}
              selectedKeys={selectedKeys}
              onSelect={handleTableSelect}
              onCoverClick={openRelationsForMod}
              onSourceClick={(mod) => setProviderKey(mod.key)}
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
                  relationsOpenKey={relationsKey}
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
          <button type="button" onClick={() => setSettingsOpen(true)} disabled={busy || bootstrapping}>
            Открыть настройки
          </button>
        </section>
      )}
      </div>

      {bootstrapping && progress ? (
        <footer className="prefetchProgressWrap">
          <div className="prefetchProgressTrack" aria-hidden="true">
            <div className="prefetchProgressBar" style={{ width: `${progressPercent}%` }} />
          </div>
          <p className="prefetchProgressLabel">{progressLabel}</p>
        </footer>
      ) : null}

      {settingsOpen ? (
        <SettingsDialog
          settings={settings}
          busy={uiLocked}
          onClose={() => setSettingsOpen(false)}
          onSave={handleSaveSettings}
          onCleared={async () => {
            const next = await getSettings();
            setSettings(next);
            if (next.instanceRoot) {
              await reload();
              void runBootstrap(next.cacheStatus, { force: true });
            }
          }}
        />
      ) : null}

      <ProviderDialog
        mod={providerMod}
        busy={busy}
        onClose={() => !busy && setProviderKey(null)}
        onSelect={handleSwitchSource}
      />
    </main>
  );
}

export default App;
