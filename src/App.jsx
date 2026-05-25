import { useCallback, useEffect, useMemo, useState } from 'react';
import { Search, Settings, SlidersHorizontal } from 'lucide-react';
import { getSettings, saveSettings, scanMods, updateModTags } from './api.js';
import { IconButton } from './components/Button.jsx';
import { ModEditor } from './features/mods/ModEditor.jsx';
import { ModTable } from './features/mods/ModTable.jsx';
import { SettingsDialog } from './features/settings/SettingsDialog.jsx';
import { filters } from './lib/modMeta.jsx';
import './styles/index.css';

function App() {
  const [payload, setPayload] = useState(null);
  const [settings, setSettings] = useState(null);
  const [selected, setSelected] = useState(null);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [settingsOpen, setSettingsOpen] = useState(false);

  const mods = payload?.mods ?? [];

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return mods.filter((mod) => {
      const matchesQuery = !needle || `${mod.displayName} ${mod.filename} ${mod.description ?? ''}`.toLowerCase().includes(needle);
      const matchesFilter =
        filter === 'all' ||
        (filter === 'library' && mod.library) ||
        (filter === 'technical' && mod.technical) ||
        mod.side === filter;
      return matchesQuery && matchesFilter;
    });
  }, [mods, query, filter]);

  const applyPayload = useCallback((next) => {
    setPayload(next);
    setSettings(next.settings);
    setSelected((current) => {
      if (!next.mods.length) return null;
      return next.mods.find((mod) => mod.key === current?.key) ?? next.mods[0];
    });
  }, []);

  const loadSettings = useCallback(async () => {
    const next = await getSettings();
    setSettings(next);
    return next;
  }, []);

  const reload = useCallback(async () => {
    setBusy(true);
    setError('');
    try {
      const next = await scanMods();
      applyPayload(next);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, [applyPayload]);

  useEffect(() => {
    loadSettings()
      .then((next) => {
        if (next.instanceRoot) reload();
        else setSettingsOpen(true);
      })
      .catch((err) => setError(String(err)));
  }, [loadSettings, reload]);

  useEffect(() => {
    if (!visible.length) {
      setSelected(null);
      return;
    }
    if (!selected || !visible.some((mod) => mod.key === selected.key)) {
      setSelected(visible[0]);
    }
  }, [visible, selected]);

  async function handleSaveSettings(nextSettings) {
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
  }

  const patchMod = useCallback(async (key, patch) => {
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
  }, [applyPayload]);

  const stats = payload?.stats ?? {};

  return (
    <main className="appShell">
      <header className="topbar">
        <div>
          <h1>mod-manager</h1>
          <p>
            {stats.total ?? 0} jar · {stats.tagged ?? 0} в mod-tags.json · {stats.noIndex ?? 0} сторонних
          </p>
        </div>
        <IconButton icon={Settings} label="Настройки" onClick={() => setSettingsOpen(true)} />
      </header>

      <section className="toolbar">
        <label className="search">
          <Search size={17} />
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Поиск по названию, файлу или описанию" />
        </label>
        <div className="segments">
          {filters.map((item) => {
            const Icon = item.icon ?? SlidersHorizontal;
            return (
              <button key={item.id} className={filter === item.id ? 'active' : ''} onClick={() => setFilter(item.id)} type="button">
                {item.icon ? <Icon className={`tagIcon ${item.tone}`} size={16} /> : null}
                <span>{item.label}</span>
              </button>
            );
          })}
        </div>
      </section>

      {error ? <div className="errorBar">{error}</div> : null}

      {settings?.instanceRoot ? (
        <section className="workspace">
          <ModTable mods={visible} selected={selected} onSelect={setSelected} />
          {selected ? (
            <ModEditor mod={selected} mods={mods} busy={busy} onPatch={patchMod} onSelectMod={setSelected} />
          ) : (
            <aside className="emptyState">Выбери мод в списке</aside>
          )}
        </section>
      ) : (
        <section className="setupState">
          <h2>Выбери сборку</h2>
          <p>mod-manager хранит метки в `.mod-manager/mod-tags.json` внутри выбранного инстанса.</p>
          <button type="button" onClick={() => setSettingsOpen(true)}>Открыть настройки</button>
        </section>
      )}

      {settingsOpen ? (
        <SettingsDialog
          settings={settings}
          busy={busy}
          onClose={() => setSettingsOpen(false)}
          onSave={handleSaveSettings}
          onRescan={reload}
        />
      ) : null}
    </main>
  );
}

export default App;
