import { useEffect, useMemo, useRef, useState } from 'react';
import { Folder, FolderOpen, Info, RefreshCw, Trash2 } from 'lucide-react';
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

export function SettingsDialog({ settings, busy, onClose, onSave, onCleared }) {
  const [draft, setDraft] = useState(() => settings ?? {});
  const [message, setMessage] = useState('');
  const [clearConfirm, setClearConfirm] = useState(null);
  const [clearing, setClearing] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [updateStatus, setUpdateStatus] = useState('idle');
  const [updateNotice, setUpdateNotice] = useState(null);
  const updateNoticeTimerRef = useRef(null);

  const packLocked = busy || clearing;
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

  useEffect(() => () => {
    if (updateNoticeTimerRef.current) clearTimeout(updateNoticeTimerRef.current);
  }, []);

  function clearUpdateNoticeTimer() {
    if (!updateNoticeTimerRef.current) return;
    clearTimeout(updateNoticeTimerRef.current);
    updateNoticeTimerRef.current = null;
  }

  function showUpToDateNotice() {
    clearUpdateNoticeTimer();
    setUpdateNotice({ tone: 'ok', text: 'У вас актуальная версия' });
    updateNoticeTimerRef.current = setTimeout(() => {
      setUpdateNotice(null);
      updateNoticeTimerRef.current = null;
    }, 3000);
  }

  async function checkUpdates() {
    if (!canCheckForUpdates()) return;
    setUpdateStatus('checking');
    clearUpdateNoticeTimer();
    setUpdateNotice(null);
    try {
      const update = await checkForAppUpdate();
      if (!update) {
        setUpdateStatus('idle');
        showUpToDateNotice();
        return;
      }
      setUpdateStatus('installing');
      await installAppUpdate(update);
    } catch (err) {
      setUpdateStatus('idle');
      setUpdateNotice({ tone: 'error', text: String(err) });
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

  const uiBusy = packLocked || updateBusy;

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
          <div className="settingsUpdateBar">
            <span
              className={[
                'settingsUpdateVersion',
                updateNotice ? `settingsUpdateVersion--${updateNotice.tone}` : ''
              ].filter(Boolean).join(' ')}
            >
              {updateNotice ? updateNotice.text : (appVersion || '—')}
            </span>
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
