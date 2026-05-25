import { useEffect, useMemo, useState } from 'react';
import { Folder, FolderOpen, Info, RefreshCw } from 'lucide-react';
import { getVersion } from '@tauri-apps/api/app';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button } from '../../components/Button.jsx';
import { Modal } from '../../components/Modal.jsx';
import { NoticeModal } from '../../components/NoticeModal.jsx';
import { clearAppData } from '../../api.js';
import { canCheckForUpdates, checkForAppUpdate, installAppUpdate } from '../../lib/updater.js';

function toSettings(draft) {
  return {
    instanceRoot: draft.instanceRoot || null,
    curseforgeApiKey: draft.curseforgeApiKey ?? '',
    autoPrefetchCovers: true,
    autoPrefetchDependencies: true,
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

function formatStatus(status) {
  if (!status) return 'не подготовлена';
  if (status.ready) return 'кэш готов';
  const missing = [];
  if (status.needsCovers) missing.push('обложки');
  if (status.needsDependencies) missing.push('зависимости');
  return missing.length ? `нужно: ${missing.join(' и ')}` : 'готово к проверке';
}

export function SettingsDialog({ settings, busy, onClose, onSave, onCleared }) {
  const [draft, setDraft] = useState(() => settings ?? {});
  const [message, setMessage] = useState('');
  const [clearConfirm, setClearConfirm] = useState(null);
  const [clearing, setClearing] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [updateStatus, setUpdateStatus] = useState('idle');

  const hasPack = Boolean(draft.instanceRoot);
  const cacheStatus = settings?.cacheStatus;
  const recent = useMemo(() => {
    const list = settings?.recentInstances ?? [];
    return list.filter((item) => item && item !== draft.instanceRoot);
  }, [settings, draft.instanceRoot]);

  useEffect(() => {
    setDraft(settings ?? {});
  }, [settings]);

  useEffect(() => {
    if (!canCheckForUpdates()) return;
    void getVersion()
      .then((version) => setAppVersion(version))
      .catch(() => {});
  }, []);

  async function checkUpdates() {
    if (!canCheckForUpdates()) return;
    setUpdateStatus('checking');
    setMessage('');
    try {
      const update = await checkForAppUpdate();
      if (!update) {
        setUpdateStatus('idle');
        return;
      }
      setUpdateStatus('installing');
      await installAppUpdate(update);
    } catch (err) {
      setUpdateStatus('idle');
      setMessage(String(err));
    }
  }

  const updateBusy = updateStatus === 'checking' || updateStatus === 'installing';
  const updateLabel =
    updateStatus === 'checking' ? '…' : updateStatus === 'installing' ? 'Загрузка' : 'Обновить';

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

  async function openCurseForgeHelp() {
    await openUrl('https://console.curseforge.com/?#/api-keys');
  }

  async function confirmClear() {
    setClearing(true);
    setMessage('');
    try {
      await clearAppData();
      setClearConfirm(null);
      await onCleared?.();
      onClose?.();
    } catch (err) {
      setMessage(`Не удалось обновить данные: ${err}`);
    } finally {
      setClearing(false);
    }
  }

  const uiBusy = busy || clearing || updateBusy;

  return (
    <Modal
      title="Настройки"
      onClose={onClose}
      footer={(
        <div className="settingsFooter">
          <Button
            icon={RefreshCw}
            onClick={() => setClearConfirm('Заново загрузить обложки и\u00a0зависимости?')}
            disabled={uiBusy}
            tone="ghost"
            className="settingsFooterBtn"
          >
            Обновить данные
          </Button>
        </div>
      )}
    >
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
            <Button icon={FolderOpen} onClick={pickFolder} disabled={uiBusy}>
              Выбрать
            </Button>
          </div>
          {hasPack ? (
            <p className="cacheHint">Статус: {formatStatus(cacheStatus)}.</p>
          ) : null}
        </label>

        {recent.length ? (
          <div className="field">
            <span className="fieldLabel">Недавние сборки</span>
            <ul className="recentList">
              {recent.map((path) => (
                <li key={path}>
                  <button
                    type="button"
                    className="recentItem"
                    onClick={() => useRecent(path)}
                    disabled={uiBusy}
                    title={path}
                  >
                    <Folder size={15} />
                    <span className="recentName">{instanceNameFromPath(path)}</span>
                  </button>
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        {canCheckForUpdates() ? (
          <div className="settingsUpdateBar">
            <span className="settingsUpdateVersion">{appVersion || '—'}</span>
            <button
              type="button"
              className="settingsUpdateAction"
              onClick={checkUpdates}
              disabled={uiBusy}
            >
              {updateLabel}
            </button>
          </div>
        ) : null}

        {message ? <p className="settingsMessage">{message}</p> : null}
      </div>

      {clearConfirm ? (
        <NoticeModal
          tone="bad"
          message={clearConfirm}
          onClose={() => !clearing && setClearConfirm(null)}
          confirm={{
            confirmLabel: clearing ? 'Обновляем…' : 'Обновить',
            cancelLabel: 'Отмена',
            busy: clearing,
            onConfirm: confirmClear,
            onCancel: () => setClearConfirm(null)
          }}
        />
      ) : null}
    </Modal>
  );
}
