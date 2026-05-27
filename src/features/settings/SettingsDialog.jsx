import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Folder, FolderOpen, HardDrive, Info, RefreshCw, Settings2, Trash2 } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button } from '../../components/Button.jsx';
import { Modal } from '../../components/Modal.jsx';
import { canCheckForUpdates } from '../../lib/updater.js';
import { getDataUsage } from '../../api.js';

function toSettings(draft) {
  return {
    instanceRoot: draft.instanceRoot || null,
    curseforgeApiKey: draft.curseforgeApiKey ?? '',
    autoPrefetchCovers: true,
    autoPrefetchDependencies: true,
    autoCheckUpdates: draft.autoCheckUpdates ?? true,
    recentInstances: draft.recentInstances ?? []
  };
}

function instanceNameFromPath(path) {
  if (!path) return '';
  const parts = path.split(/[\\/]/).filter(Boolean);
  while (parts.length) {
    const tail = parts[parts.length - 1].toLowerCase();
    if (tail === 'mods' || tail === 'minecraft' || tail === '.minecraft') {
      parts.pop();
      continue;
    }
    break;
  }
  return parts[parts.length - 1] || path;
}

function updateProgressLabel(progress) {
  if (progress?.phase === 'install') return 'Установка…';
  if (progress?.percent != null) return `Загрузка ${progress.percent}%`;
  return 'Загрузка…';
}

function formatBytes(bytes) {
  if (!bytes && bytes !== 0) return '—';
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) {
    return kb < 10 ? `${kb.toFixed(1)} KB` : `${Math.round(kb)} KB`;
  }
  const mb = kb / 1024;
  if (mb < 1024) {
    return mb < 10 ? `${mb.toFixed(2)} MB` : `${mb.toFixed(1)} MB`;
  }
  const gb = mb / 1024;
  return `${gb.toFixed(2)} GB`;
}

export function SettingsDialog({ settings, busy, syncing = false, onClose, onSave, onRunSync, updater }) {
  const [tab, setTab] = useState('general');
  const [draft, setDraft] = useState(() => settings ?? {});
  const [message, setMessage] = useState('');
  const wasSyncingRef = useRef(false);

  const [syncOptions, setSyncOptions] = useState({
    identify: true,
    labels: true,
    assets: true
  });

  const [usage, setUsage] = useState(null);

  const packLocked = busy;
  const recent = useMemo(() => {
    const list = settings?.recentInstances ?? [];
    return list.filter((item) => item && item !== draft.instanceRoot);
  }, [settings, draft.instanceRoot]);

  const updateStatus = updater?.status ?? 'idle';
  const updateProgress = updater?.progress ?? null;
  const updateNotice = updater?.notice ?? null;
  const appVersion = updater?.appVersion ?? '';
  const updateBusy = updater?.updateBusy ?? false;

  useEffect(() => {
    setDraft(settings ?? {});
  }, [settings]);

  const refreshUsage = useCallback(async () => {
    try {
      const data = await getDataUsage();
      setUsage(data);
    } catch (err) {
      console.error('get_data_usage failed', err);
    }
  }, []);

  useEffect(() => {
    if (tab !== 'data') return;
    void refreshUsage();
  }, [tab, refreshUsage]);

  useEffect(() => {
    if (wasSyncingRef.current && !syncing && tab === 'data') {
      void refreshUsage();
    }
    wasSyncingRef.current = syncing;
  }, [syncing, tab, refreshUsage]);

  async function handleManualUpdate() {
    if (!updater) return;
    await updater.checkAndInstall({ silent: false, fromSettings: true });
  }

  const updateLabel =
    updateStatus === 'checking' || updateStatus === 'installing' ? '…' : 'Обновить';

  async function pickFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Выбери папку сборки'
    });
    if (typeof selected !== 'string') return;
    const next = { ...draft, instanceRoot: selected };
    setDraft(next);
    setMessage('');
    await onSave(toSettings(next), { bootstrap: true });
    onClose?.();
  }

  async function useRecent(path) {
    const next = { ...draft, instanceRoot: path };
    setDraft(next);
    setMessage('');
    await onSave(toSettings(next), { bootstrap: true });
    onClose?.();
  }

  async function removeRecent(path, event) {
    event.stopPropagation();
    const nextRecent = (settings?.recentInstances ?? []).filter((item) => item !== path);
    const next = { ...draft, recentInstances: nextRecent };
    setDraft(next);
    setMessage('');
    await onSave(toSettings(next), { bootstrap: false, scan: false });
  }

  async function openCurseForgeHelp() {
    await openUrl('https://console.curseforge.com/?#/api-keys');
  }

  async function saveAutoCheckUpdates(enabled) {
    const next = { ...draft, autoCheckUpdates: enabled };
    setDraft(next);
    setMessage('');
    await onSave(toSettings(next), { bootstrap: false, scan: false });
  }

  function toggleSyncOption(key) {
    setSyncOptions((current) => ({ ...current, [key]: !current[key] }));
  }

  const anySyncSelected = syncOptions.identify || syncOptions.labels || syncOptions.assets;

  async function runSync() {
    if (!anySyncSelected || syncing || !onRunSync) return;
    try {
      await onRunSync(syncOptions);
    } catch (_err) {
      /* ошибка показывается в App */
    }
  }

  const uiBusy = packLocked || updateBusy;
  const syncBusy = syncing || packLocked;

  return (
    <Modal
      ariaLabel="Настройки"
      onClose={onClose}
      headerExtra={
        <div className="settingsTabs" role="tablist" aria-label="Разделы настроек">
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'general'}
            className={`settingsTab${tab === 'general' ? ' settingsTab--active' : ''}`}
            onClick={() => setTab('general')}
          >
            <Settings2 size={16} className="settingsTabIcon" aria-hidden="true" />
            <span className="settingsTabLabel">Основные настройки</span>
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'data'}
            className={`settingsTab${tab === 'data' ? ' settingsTab--active' : ''}`}
            onClick={() => setTab('data')}
          >
            <HardDrive size={16} className="settingsTabIcon" aria-hidden="true" />
            <span className="settingsTabLabel">Данные</span>
          </button>
        </div>
      }
    >
      {tab === 'general' ? (
        <div className="settingsMinimal">
          <div className="field">
            <div className="fieldHeader">
              <label htmlFor="curseforgeApiKey">CurseForge API key</label>
              <button
                className="infoTinyButton"
                type="button"
                aria-label="Открыть страницу API keys"
                onClick={openCurseForgeHelp}
              >
                <Info size={15} />
              </button>
            </div>
            <input
              id="curseforgeApiKey"
              value={draft.curseforgeApiKey ?? ''}
              onChange={(event) => {
                setMessage('');
                setDraft((current) => ({ ...current, curseforgeApiKey: event.target.value }));
              }}
              onBlur={() => onSave(toSettings(draft), { bootstrap: false, scan: false })}
              placeholder="Для CurseForge-модов"
              type="password"
            />
          </div>

          <label className="field">
            <span>Папка сборки</span>
            <div className="pathField">
              <input
                value={draft.instanceRoot ?? ''}
                readOnly
                placeholder="/Users/.../PrismLauncher/instances/Pack"
              />
              <Button icon={FolderOpen} onClick={pickFolder} disabled={packLocked}>
                Выбрать
              </Button>
            </div>
          </label>

          {recent.length ? (
            <div className="field">
              <span className="fieldLabel">Недавние сборки</span>
              <ul className="recentList">
                {recent.map((path) => (
                  <li key={path} className="recentRow">
                    <button
                      type="button"
                      className="recentItem"
                      onClick={() => useRecent(path)}
                      disabled={packLocked}
                      title={path}
                    >
                      <Folder size={15} />
                      <span className="recentName">{instanceNameFromPath(path)}</span>
                    </button>
                    <button
                      type="button"
                      className="recentRemove"
                      onClick={(event) => removeRecent(path, event)}
                      disabled={packLocked}
                      aria-label="Удалить из недавних"
                      title="Удалить"
                    >
                      <Trash2 size={15} />
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}

          {canCheckForUpdates() ? (
            <>
              <label className="settingsToggleRow">
                <input
                  type="checkbox"
                  checked={draft.autoCheckUpdates ?? true}
                  disabled={packLocked}
                  onChange={(event) => saveAutoCheckUpdates(event.target.checked)}
                />
                <span>Автообновление приложения</span>
              </label>

              <div
                className={`settingsUpdateBar${updateStatus === 'installing' ? ' settingsUpdateBar--busy' : ''}`}
              >
                <div className="settingsUpdateMain">
                  {updateStatus === 'installing' ? (
                    <>
                      <span className="settingsUpdateVersion settingsUpdateVersion--progress">
                        {updateProgressLabel(updateProgress)}
                      </span>
                      <div className="settingsUpdateProgressTrack" aria-hidden="true">
                        <div
                          className={`settingsUpdateProgressBar${updateProgress?.percent == null ? ' indeterminate' : ''}`}
                          style={
                            updateProgress?.percent != null
                              ? { width: `${updateProgress.percent}%` }
                              : undefined
                          }
                        />
                      </div>
                    </>
                  ) : (
                    <span
                      className={[
                        'settingsUpdateVersion',
                        updateNotice ? `settingsUpdateVersion--${updateNotice.tone}` : ''
                      ]
                        .filter(Boolean)
                        .join(' ')}
                    >
                      {updateNotice ? updateNotice.text : appVersion || '—'}
                    </span>
                  )}
                </div>
                <button
                  type="button"
                  className="settingsUpdateAction"
                  onClick={handleManualUpdate}
                  disabled={uiBusy}
                >
                  {updateLabel}
                </button>
              </div>
            </>
          ) : null}

          {message ? <p className="settingsMessage">{message}</p> : null}
        </div>
      ) : (
        <div className="settingsData">
          <section className="settingsBlock">
            <header className="settingsBlockHeader settingsBlockHeader--usage">
              <div className="settingsBlockHeaderMain">
                <h3 className="settingsBlockTitle">Память</h3>
                <p className="settingsBlockHint">Данные приложения на диске.</p>
              </div>
              <span className="settingsUsageTotalValue">{formatBytes(usage?.total ?? 0)}</span>
            </header>

            <ul className="settingsUsageList">
              <li className="settingsUsageRow">
                <span className="settingsUsageName">Скачанные обложки</span>
                <span className="settingsUsageValue">{formatBytes(usage?.coversCache ?? 0)}</span>
              </li>
              <li className="settingsUsageRow">
                <span className="settingsUsageName">Кастомные обложки</span>
                <span className="settingsUsageValue">{formatBytes(usage?.coversManual ?? 0)}</span>
              </li>
              <li className="settingsUsageRow">
                <span className="settingsUsageName">Метаданные</span>
                <span className="settingsUsageValue">{formatBytes(usage?.tagsFile ?? 0)}</span>
              </li>
              {usage?.otherCache ? (
                <li className="settingsUsageRow">
                  <span className="settingsUsageName">Кеш</span>
                  <span className="settingsUsageValue">{formatBytes(usage.otherCache)}</span>
                </li>
              ) : null}
            </ul>
          </section>

          <section className="settingsBlock">
            <header className="settingsBlockHeader settingsBlockHeader--sync">
              <div className="settingsBlockHeaderMain">
                <h3 className="settingsBlockTitle">Синхронизация</h3>
                <p className="settingsBlockHint">
                  Перезаписывает данные у поставщиков и обновляет связи.
                </p>
              </div>
              <button
                type="button"
                className="settingsSyncIconBtn"
                onClick={runSync}
                disabled={!anySyncSelected || syncBusy}
                title={syncing ? 'Синхронизация…' : 'Синхронизировать'}
                aria-label={syncing ? 'Синхронизация…' : 'Синхронизировать'}
              >
                <RefreshCw size={18} className={syncing ? 'spin' : ''} />
              </button>
            </header>

            <ul className="settingsCheckList">
              <li>
                <label className="settingsCheckRow">
                  <input
                    type="checkbox"
                    checked={syncOptions.identify}
                    disabled={syncBusy}
                    onChange={() => toggleSyncOption('identify')}
                  />
                  <span className="settingsCheckMain">
                    <span className="settingsCheckTitle">Привязать ручные моды</span>
                    <span className="settingsCheckHint">
                      Ищет поставщика для модов без источника.
                    </span>
                  </span>
                </label>
              </li>
              <li>
                <label className="settingsCheckRow">
                  <input
                    type="checkbox"
                    checked={syncOptions.labels}
                    disabled={syncBusy}
                    onChange={() => toggleSyncOption('labels')}
                  />
                  <span className="settingsCheckMain">
                    <span className="settingsCheckTitle">Загрузить теги</span>
                    <span className="settingsCheckHint">
                      Удаляет теги поставщиков и запрашивает их заново.
                    </span>
                  </span>
                </label>
              </li>
              <li>
                <label className="settingsCheckRow">
                  <input
                    type="checkbox"
                    checked={syncOptions.assets}
                    disabled={syncBusy}
                    onChange={() => toggleSyncOption('assets')}
                  />
                  <span className="settingsCheckMain">
                    <span className="settingsCheckTitle">Загрузить обложки и зависимости</span>
                    <span className="settingsCheckHint">
                      Перезапрашивает обложки и связи между модами.
                    </span>
                  </span>
                </label>
              </li>
            </ul>
          </section>
        </div>
      )}
    </Modal>
  );
}
