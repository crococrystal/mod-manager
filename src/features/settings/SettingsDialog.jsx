import { useEffect, useState } from 'react';
import { FolderOpen, RefreshCw, Save } from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { openPath } from '@tauri-apps/plugin-opener';
import { Button } from '../../components/Button.jsx';
import { Modal } from '../../components/Modal.jsx';

export function SettingsDialog({ settings, busy, onClose, onSave, onRescan }) {
  const [draft, setDraft] = useState(() => settings ?? {});

  useEffect(() => {
    setDraft(settings ?? {});
  }, [settings]);

  async function pickFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Выбери папку Prism-инстанса или папку minecraft/mods'
    });
    if (typeof selected === 'string') {
      setDraft((current) => ({ ...current, instanceRoot: selected }));
    }
  }

  async function openMods() {
    if (!settings?.modsDir) return;
    await openPath(settings.modsDir);
  }

  async function save() {
    await onSave({
      instanceRoot: draft.instanceRoot || null,
      curseforgeApiKey: draft.curseforgeApiKey ?? '',
      autoPrefetchCovers: draft.autoPrefetchCovers !== false,
      autoPrefetchDependencies: draft.autoPrefetchDependencies !== false
    });
  }

  return (
    <Modal
      title="Настройки"
      subtitle="Локальное приложение, данные сборки остаются на твоем диске"
      onClose={onClose}
      size="wide"
      footer={(
        <>
          <Button onClick={onClose}>Закрыть</Button>
          <Button tone="primary" icon={Save} onClick={save} disabled={busy}>Сохранить</Button>
        </>
      )}
    >
      <div className="settingsGrid">
        <section className="settingsSection">
          <h3>Сборка</h3>
          <label className="field">
            <span>Папка инстанса</span>
            <div className="pathField">
              <input
                value={draft.instanceRoot ?? ''}
                onChange={(event) => setDraft((current) => ({ ...current, instanceRoot: event.target.value }))}
                placeholder="/Users/.../PrismLauncher/instances/Pack"
              />
              <Button icon={FolderOpen} onClick={pickFolder}>Выбрать</Button>
            </div>
          </label>
          <dl className="pathMeta">
            <div>
              <dt>mods</dt>
              <dd>{settings?.modsDir ?? '-'}</dd>
            </div>
            <div>
              <dt>данные mod-manager</dt>
              <dd>{settings?.dataRoot ?? '-'}</dd>
            </div>
          </dl>
        </section>

        <section className="settingsSection">
          <h3>Автоматизация</h3>
          <label className="field">
            <span>CurseForge API key</span>
            <input
              value={draft.curseforgeApiKey ?? ''}
              onChange={(event) => setDraft((current) => ({ ...current, curseforgeApiKey: event.target.value }))}
              placeholder="Оставь пустым, если не нужен"
              type="password"
            />
          </label>
          <label className="checkRow">
            <input
              type="checkbox"
              checked={draft.autoPrefetchCovers !== false}
              onChange={(event) => setDraft((current) => ({ ...current, autoPrefetchCovers: event.target.checked }))}
            />
            <span>Подтягивать обложки при проверке сборки</span>
          </label>
          <label className="checkRow">
            <input
              type="checkbox"
              checked={draft.autoPrefetchDependencies !== false}
              onChange={(event) => setDraft((current) => ({ ...current, autoPrefetchDependencies: event.target.checked }))}
            />
            <span>Подтягивать зависимости при проверке сборки</span>
          </label>
        </section>

        <section className="settingsSection settingsSectionFull">
          <h3>Обслуживание</h3>
          <div className="maintenanceActions">
            <Button icon={FolderOpen} onClick={openMods} disabled={!settings?.modsDir}>Открыть mods</Button>
            <Button icon={RefreshCw} onClick={onRescan} disabled={busy}>Проверить сборку</Button>
          </div>
          <p className="mutedText">
            Кнопки обложек и зависимостей больше не торчат в верхней панели. Их логика будет жить здесь и запускаться как часть проверки сборки.
          </p>
        </section>
      </div>
    </Modal>
  );
}
