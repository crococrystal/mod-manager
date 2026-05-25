import { useEffect, useMemo, useState } from 'react';
import { Folder, FolderOpen, Info, RefreshCw, Trash2 } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { Button } from '../../components/Button.jsx';
import { Modal } from '../../components/Modal.jsx';
import { clearAppData } from '../../api.js';

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

export function SettingsDialog({ settings, busy, onClose, onSave, onRefresh, onCleared }) {
  const [draft, setDraft] = useState(() => settings ?? {});
  const [message, setMessage] = useState('');
  const [confirmingClear, setConfirmingClear] = useState(false);
  const [clearing, setClearing] = useState(false);

  const hasPack = Boolean(draft.instanceRoot);
  const cacheStatus = settings?.cacheStatus;
  const recent = useMemo(() => {
    const list = settings?.recentInstances ?? [];
    return list.filter((item) => item && item !== draft.instanceRoot);
  }, [settings, draft.instanceRoot]);

  useEffect(() => {
    setDraft(settings ?? {});
  }, [settings]);

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

  async function refreshNow() {
    if (!hasPack) {
      setMessage('Выбери папку сборки.');
      return;
    }
    setMessage('');
    await onRefresh(toSettings(draft));
  }

  async function openCurseForgeHelp() {
    await openUrl('https://console.curseforge.com/?#/api-keys');
  }

  async function confirmClear() {
    setClearing(true);
    setMessage('');
    try {
      const result = await clearAppData();
      const parts = [];
      if (result?.removedCatalogFiles) parts.push(`файлов: ${result.removedCatalogFiles}`);
      if (result?.clearedInstances) parts.push(`сборок: ${result.clearedInstances}`);
      setMessage(parts.length ? `Очищено — ${parts.join(', ')}.` : 'Очищено.');
      setConfirmingClear(false);
      await onCleared?.();
    } catch (err) {
      setMessage(`Не удалось очистить: ${err}`);
    } finally {
      setClearing(false);
    }
  }

  const uiBusy = busy || clearing;

  return (
    <Modal
      title="Настройки"
      onClose={onClose}
      footer={(
        <div className="settingsFooter">
          <Button
            icon={Trash2}
            onClick={() => setConfirmingClear(true)}
            disabled={uiBusy}
            tone="danger-ghost"
          >
            Очистить данные
          </Button>
          <Button icon={RefreshCw} onClick={refreshNow} disabled={uiBusy || !hasPack}>
            Обновить
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

        {confirmingClear ? (
          <div className="dangerConfirm">
            <p>Удалить все скачанные обложки и зависимости?</p>
            <div className="dangerActions">
              <Button onClick={() => setConfirmingClear(false)} disabled={clearing} tone="ghost">
                Отмена
              </Button>
              <Button onClick={confirmClear} disabled={clearing} tone="danger">
                {clearing ? 'Чистим…' : 'Да, очистить'}
              </Button>
            </div>
          </div>
        ) : null}

        {message ? <p className="settingsMessage">{message}</p> : null}
      </div>
    </Modal>
  );
}
