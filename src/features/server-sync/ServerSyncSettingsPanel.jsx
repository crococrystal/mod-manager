import { useCallback, useEffect, useRef, useState } from 'react';
import { Server } from 'lucide-react';
import { saveSettings } from '../../api.js';
import { Button } from '../../components/Button.jsx';
import { ServerSyncPathField } from './ServerSyncPathField.jsx';
import {
  normalizeServerSyncDraft,
  normalizeSshHost,
  serverSyncFromSettings,
  withServerSync
} from './serverSyncSettings.js';
import {
  previewServerSyncLane,
  syncModsToServerLane,
  testServerSync
} from './serverSyncApi.js';
import {
  EMPTY_PREVIEW_OVERLAY,
  previewToOverlayUi
} from './serverSyncPreviewUi.js';

const SSH_HOST_HINT = 'Алиас из ~/.ssh/config (User, Port, ключ).';
const CONNECTION_STATUS_RESET_MS = 3000;
const CONNECTION_STATUS_RESET_LONG_MS = 5000;
const CONNECTION_STATUS_LONG_MESSAGE_CHARS = 72;

const EMPTY_LANE_PREVIEW = {
  checking: false,
  starting: false,
  ready: false,
  preview: null
};

function connectionStatusResetMs(message) {
  return message.length > CONNECTION_STATUS_LONG_MESSAGE_CHARS
    ? CONNECTION_STATUS_RESET_LONG_MS
    : CONNECTION_STATUS_RESET_MS;
}

export function ServerSyncSettingsPanel({
  settings,
  disabled,
  onSettingsSaved,
  serverSync
}) {
  const [draft, setDraft] = useState(() => serverSyncFromSettings(settings));
  const [testMessage, setTestMessage] = useState('');
  const [testMessageOk, setTestMessageOk] = useState(false);
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [lanePreviews, setLanePreviews] = useState({
    server: { ...EMPTY_LANE_PREVIEW },
    distribution: { ...EMPTY_LANE_PREVIEW }
  });
  const dirtyRef = useRef(false);
  const connectionResetTimerRef = useRef(null);
  const {
    server: serverLane,
    distribution: distributionLane,
    visibleResult,
    syncing,
    refresh,
    cancel,
    dismissLaneResult
  } = serverSync;

  useEffect(() => {
    if (dirtyRef.current) return;
    setDraft(serverSyncFromSettings(settings));
  }, [settings]);

  const clearConnectionStatus = useCallback(() => {
    if (connectionResetTimerRef.current) {
      clearTimeout(connectionResetTimerRef.current);
      connectionResetTimerRef.current = null;
    }
    setTestMessage('');
    setTestMessageOk(false);
    setTesting(false);
  }, []);

  useEffect(() => {
    if (testing || !testMessage) return undefined;

    connectionResetTimerRef.current = setTimeout(() => {
      connectionResetTimerRef.current = null;
      setTestMessage('');
      setTestMessageOk(false);
    }, connectionStatusResetMs(testMessage));

    return () => {
      if (connectionResetTimerRef.current) {
        clearTimeout(connectionResetTimerRef.current);
        connectionResetTimerRef.current = null;
      }
    };
  }, [testing, testMessage]);

  const resetLanePreview = useCallback((lane) => {
    setLanePreviews((current) => ({
      ...current,
      [lane]: { ...EMPTY_LANE_PREVIEW }
    }));
  }, []);

  const resetAllLanePreviews = useCallback(() => {
    setLanePreviews({
      server: { ...EMPTY_LANE_PREVIEW },
      distribution: { ...EMPTY_LANE_PREVIEW }
    });
  }, []);

  useEffect(() => {
    if (serverLane.syncing || serverLane.showResult) {
      resetLanePreview('server');
    }
    if (distributionLane.syncing || distributionLane.showResult) {
      resetLanePreview('distribution');
    }
  }, [
    distributionLane.showResult,
    distributionLane.syncing,
    resetLanePreview,
    serverLane.showResult,
    serverLane.syncing
  ]);

  useEffect(() => () => resetAllLanePreviews(), [resetAllLanePreviews]);

  const persist = useCallback(
    async (next, { blockUi = true } = {}) => {
      if (blockUi) {
        setSaving(true);
      }
      try {
        const saved = await saveSettings(withServerSync(settings, next));
        dirtyRef.current = false;
        setDraft(serverSyncFromSettings(saved));
        onSettingsSaved?.(saved);
      } finally {
        if (blockUi) {
          setSaving(false);
        }
      }
    },
    [onSettingsSaved, settings]
  );

  function updateDraft(patch) {
    dirtyRef.current = true;
    clearConnectionStatus();
    if ('serverModsPath' in patch) {
      resetLanePreview('server');
    }
    if ('distributionModsPath' in patch) {
      resetLanePreview('distribution');
    }
    if ('sshHost' in patch) {
      resetAllLanePreviews();
    }
    setDraft((current) => ({ ...current, ...patch }));
  }

  async function handleToggleDeleteExtra(enabled) {
    resetAllLanePreviews();
    const next = { ...draft, deleteExtraRemoteJars: enabled };
    setDraft(next);
    await persist(next, { blockUi: false });
  }

  async function handleToggleEnabled(enabled) {
    const next = { ...draft, enabled };
    setDraft(next);
    await persist(next, { blockUi: false });
  }

  async function handleBlurSave() {
    const next = normalizeServerSyncDraft({
      ...draft,
      sshHost: draft.sshHost
    });
    setDraft(next);
    await persist(next);
  }

  async function handleTestConnection() {
    const sshHost = normalizeSshHost(draft.sshHost);
    if (sshHost !== draft.sshHost) {
      setDraft((current) => ({ ...current, sshHost }));
    }

    setTesting(true);
    setTestMessage('Проверка подключения…');
    setTestMessageOk(false);
    try {
      const result = await testServerSync({ sshHost });
      const text =
        result?.message ||
        (result?.ok ? `«${sshHost}» подключён.` : 'SSH недоступен.');
      setTestMessage(text);
      setTestMessageOk(Boolean(result?.ok));
    } catch (err) {
      setTestMessage(String(err));
      setTestMessageOk(false);
    } finally {
      setTesting(false);
    }
  }

  async function startSync(lane) {
    clearConnectionStatus();
    dismissLaneResult(lane);
    setLanePreviews((current) => ({
      ...current,
      [lane]: { checking: false, starting: true, ready: false, preview: null }
    }));
    try {
      await persist(
        normalizeServerSyncDraft({
          ...draft,
          sshHost: draft.sshHost
        })
      );
      const result = await syncModsToServerLane(lane);
      if (result?.alreadyRunning) {
        await refresh();
      }
    } catch (err) {
      resetLanePreview(lane);
      console.error(`sync lane ${lane} failed`, err);
    }
  }

  async function checkLane(lane) {
    clearConnectionStatus();
    dismissLaneResult(lane);
    setLanePreviews((current) => ({
      ...current,
      [lane]: { checking: true, ready: false, preview: null }
    }));
    try {
      await persist(
        normalizeServerSyncDraft({
          ...draft,
          sshHost: draft.sshHost
        })
      );
      const preview = await previewServerSyncLane(lane);
      setLanePreviews((current) => ({
        ...current,
        [lane]: { checking: false, ready: true, preview }
      }));
    } catch (err) {
      setLanePreviews((current) => ({
        ...current,
        [lane]: {
          checking: false,
          ready: true,
          preview: { ok: false, errors: [String(err)] }
        }
      }));
    }
  }

  async function handleLaneAction(lane) {
    const laneUi = lane === 'server' ? serverLane : distributionLane;
    if (visibleResult[lane] && laneUi.ok && laneUi.doneParts) {
      dismissLaneResult(lane);
      return;
    }

    const state = lanePreviews[lane];
    if (state.ready && state.preview?.ok) {
      const toUpload = state.preview.toUpload ?? 0;
      const toDelete = state.preview.toDelete ?? 0;
      const toUpdate = state.preview.toUpdate ?? 0;
      if (toUpload === 0 && toDelete === 0 && toUpdate === 0) {
        dismissLanePreview(lane);
        return;
      }
      await startSync(lane);
      return;
    }
    await checkLane(lane);
  }

  function lanePreviewUi(lane) {
    const state = lanePreviews[lane];
    if (!state.checking && !state.starting && !state.ready) {
      return EMPTY_PREVIEW_OVERLAY;
    }
    return previewToOverlayUi(state.preview, {
      checking: state.checking,
      starting: state.starting
    });
  }

  function dismissLanePreview(lane) {
    resetLanePreview(lane);
  }

  function preventBlur(event) {
    event.preventDefault();
  }

  const panelBusy = disabled || saving;
  const actionBusy =
    testing ||
    syncing ||
    lanePreviews.server.checking ||
    lanePreviews.server.starting ||
    lanePreviews.distribution.checking ||
    lanePreviews.distribution.starting;
  const showConnectionStatus = Boolean(testing || testMessage);
  const sshHostHintClassName = [
    'serverSyncHostHint',
    showConnectionStatus && testing ? 'serverSyncHostHint--pending' : '',
    showConnectionStatus && !testing && testMessageOk ? 'serverSyncHostHint--ok' : '',
    showConnectionStatus && !testing && testMessage && !testMessageOk
      ? 'serverSyncHostHint--error'
      : ''
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className="settingsMinimal serverSyncPanel">
      <div className="field">
        <div className="fieldHeader">
          <label htmlFor="serverSyncSshHost">SSH host</label>
        </div>
        <div className="pathField serverSyncHostField">
          <input
            id="serverSyncSshHost"
            value={draft.sshHost}
            disabled={panelBusy || actionBusy}
            placeholder="win-test"
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            onChange={(event) => updateDraft({ sshHost: event.target.value })}
            onBlur={handleBlurSave}
          />
          <Button
            icon={Server}
            onMouseDown={preventBlur}
            onClick={handleTestConnection}
            disabled={panelBusy || actionBusy || !draft.sshHost.trim()}
          >
            {testing ? 'Проверка…' : 'Проверка'}
          </Button>
        </div>
        <p className={sshHostHintClassName} role="status" aria-live="polite">
          {showConnectionStatus ? testMessage : SSH_HOST_HINT}
        </p>
      </div>

      <div className="serverSyncToggleGroup">
        <label className="serverSyncToggleRow">
          <input
            type="checkbox"
            checked={draft.enabled}
            disabled={disabled || actionBusy}
            onChange={(event) => handleToggleEnabled(event.target.checked)}
          />
          <span>Авто-синхронизация</span>
        </label>
        <label className="serverSyncToggleRow">
          <input
            type="checkbox"
            checked={draft.deleteExtraRemoteJars ?? true}
            disabled={disabled || actionBusy}
            onChange={(event) => handleToggleDeleteExtra(event.target.checked)}
          />
          <span>Удалять лишние jar</span>
        </label>
      </div>

      <ServerSyncPathField
        id="serverSyncServerModsPath"
        label="Путь для серверных модов"
        hint="Моды для сервера. Клиентские моды сюда не попадают."
        value={draft.serverModsPath}
        placeholder=".../server/mods"
        inputDisabled={panelBusy}
        syncDisabled={panelBusy || !draft.enabled || !draft.serverModsPath.trim()}
        laneUi={serverLane}
        previewUi={lanePreviewUi('server')}
        showResult={visibleResult.server}
        onChange={(event) => updateDraft({ serverModsPath: event.target.value })}
        onBlur={handleBlurSave}
        onAction={() => void handleLaneAction('server')}
        onCancel={() => void cancel('server')}
        onDismissPreview={() => dismissLanePreview('server')}
        onDismissResult={() => dismissLaneResult('server')}
        onEditStart={() => dismissLanePreview('server')}
      />

      <ServerSyncPathField
        id="serverSyncDistributionModsPath"
        label="Путь для всех модов"
        hint="Папка для раздачи всех модов, включая клиентские."
        value={draft.distributionModsPath}
        placeholder=".../server/automodpack/.../mods"
        inputDisabled={panelBusy}
        syncDisabled={panelBusy || !draft.enabled || !draft.distributionModsPath.trim()}
        laneUi={distributionLane}
        previewUi={lanePreviewUi('distribution')}
        showResult={visibleResult.distribution}
        onChange={(event) => updateDraft({ distributionModsPath: event.target.value })}
        onBlur={handleBlurSave}
        onAction={() => void handleLaneAction('distribution')}
        onCancel={() => void cancel('distribution')}
        onDismissPreview={() => dismissLanePreview('distribution')}
        onDismissResult={() => dismissLaneResult('distribution')}
        onEditStart={() => dismissLanePreview('distribution')}
      />
    </div>
  );
}
