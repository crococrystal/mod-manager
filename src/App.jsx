import { useCallback, useEffect, useMemo, useState } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Search, Settings, SlidersHorizontal } from 'lucide-react';
import {
  bootstrapInstance,
  getSettings,
  saveSettings,
  scanMods,
  updateModTags,
  uploadCover
} from './api.js';
import { IconButton } from './components/Button.jsx';
import { TitleBar } from './components/TitleBar.jsx';
import { NoticeModal } from './components/NoticeModal.jsx';
import { ModEditor } from './features/mods/ModEditor.jsx';
import { ModTable } from './features/mods/ModTable.jsx';
import { SettingsDialog } from './features/settings/SettingsDialog.jsx';
import { filters } from './lib/modMeta.jsx';
import { normalizeModsGraph } from './lib/usedBy.js';
import './styles/index.css';

function needsBootstrap(cacheStatus, { force = false } = {}) {
  if (force) return true;
  if (!cacheStatus) return true;
  return cacheStatus.needsCovers || cacheStatus.needsDependencies;
}

function withLocalCovers(next) {
  const mods = (next.mods ?? []).map((mod) => ({
    ...mod,
    coverUrl: mod.coverPath ? convertFileSrc(mod.coverPath) : mod.coverUrl ?? null
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
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [busy, setBusy] = useState(false);
  const [bootstrapping, setBootstrapping] = useState(false);
  const [error, setError] = useState('');
  const [info, setInfo] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [progress, setProgress] = useState(null);

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
        if (!bootstrapping) setInfo('');
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
    [applyPayload, bootstrapping]
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
  }, [loadSettings, reload, runBootstrap]);

  useEffect(() => {
    if (!visible.length) {
      setSelected(null);
      return;
    }
    if (!selected || !visible.some((mod) => mod.filename === selected.filename)) {
      setSelected(visible[0]);
    }
  }, [visible, selected]);

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
      setSelected(visible[nextIndex]);
    },
    [visible, selected]
  );

  useEffect(() => {
    function handleKeyDown(event) {
      if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
      const target = event.target;
      if (target instanceof HTMLElement && target.closest('input, textarea, select, [contenteditable="true"]')) {
        return;
      }
      event.preventDefault();
      moveSelection(event.key === 'ArrowUp' ? -1 : 1);
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [moveSelection]);

  useEffect(() => {
    let unlistenProgress;
    let unlistenCover;
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
        const { key, coverPath } = event.payload ?? {};
        if (!key || !coverPath) return;
        const coverUrl = convertFileSrc(coverPath);
        setPayload((current) => {
          if (!current) return current;
          let touched = false;
          const mods = current.mods.map((mod) => {
            if (mod.key !== key) return mod;
            touched = true;
            return { ...mod, coverPath, coverUrl };
          });
          return touched ? { ...current, mods } : current;
        });
        setSelected((current) => {
          if (!current || current.key !== key) return current;
          return { ...current, coverPath, coverUrl };
        });
      });
    })();
    return () => {
      unlistenProgress?.();
      unlistenCover?.();
    };
  }, []);

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

  const handleRefreshSettings = useCallback(
    async (nextSettings) => {
      setBusy(true);
      setError('');
      try {
        const saved = await saveSettings(nextSettings);
        setSettings(saved);
        await reload();
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [reload]
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

  const canShowWorkspace = Boolean(settings?.instanceRoot);
  const progressPercent =
    progress?.total > 0 ? Math.min(100, Math.round((progress.index / progress.total) * 100)) : 0;
  const uiLocked = busy || bootstrapping;

  const progressLabel =
    bootstrapping && progress
      ? `${progress.kind === 'covers' ? 'Обложки' : 'Зависимости'} · ${progress.index}/${progress.total}${
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
              disabled={uiLocked}
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
          disabled={uiLocked}
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
        disabled={uiLocked}
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
            <ModTable mods={visible} selected={selected} onSelect={setSelected} />
            <aside>
              {selected ? (
                <ModEditor
                  mod={selected}
                  mods={mods}
                  busy={uiLocked}
                  onPatch={patchMod}
                  onUploadCover={handleUploadCover}
                  onSelectMod={(mod) => setSelected(mods.find((item) => item.filename === mod.filename) ?? mod)}
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
          <button type="button" onClick={() => setSettingsOpen(true)}>
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
          onRefresh={handleRefreshSettings}
          onCleared={async () => {
            const next = await getSettings();
            setSettings(next);
            if (next.instanceRoot) await reload();
          }}
        />
      ) : null}
    </main>
  );
}

export default App;
