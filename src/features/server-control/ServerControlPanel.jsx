import { useCallback, useEffect, useRef, useState } from 'react';
import { Pencil } from 'lucide-react';
import { IconButton } from '../../components/Button.jsx';
import { ServerSyncPathField } from '../server-sync/ServerSyncPathField.jsx';
import { LaunchScriptEditorModal } from './LaunchScriptEditorModal.jsx';
import {
  checkServerControlStatus,
  startServerControl,
  stopServerControl
} from './serverControlApi.js';
import { serverControlActionUi, serverControlToOverlayUi } from './serverControlUi.js';

const EMPTY_SYNC_LANE = {
  syncing: false,
  main: '',
  ok: false,
  showResult: false,
  doneParts: null,
  phase: null,
  side: '',
  current: 0,
  total: 0,
  filename: ''
};

const LAUNCH_POLL_ATTEMPTS = 12;
const LAUNCH_POLL_INTERVAL_MS = 5000;

function sleep(ms) {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

export function ServerControlPanel({
  sshHost,
  serverRootPath,
  serverStartScript,
  disabled,
  actionBusy,
  inputDisabled,
  onRootPathChange,
  onStartSettingChange,
  onBlur
}) {
  const [checked, setChecked] = useState(false);
  const [running, setRunning] = useState(false);
  const [statusMessage, setStatusMessage] = useState('');
  const [checking, setChecking] = useState(false);
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState('');
  const [editorOpen, setEditorOpen] = useState(false);
  const [awaitingLaunch, setAwaitingLaunch] = useState(false);
  const launchPollRef = useRef(0);

  useEffect(
    () => () => {
      launchPollRef.current += 1;
    },
    []
  );

  const hasSsh = Boolean(sshHost?.trim());
  const hasServerRoot = Boolean(serverRootPath?.trim());
  const hasScript = Boolean(serverStartScript?.trim());
  const rowBusy = checking || starting || stopping || awaitingLaunch;
  const controlDisabled =
    disabled || rowBusy || actionBusy || !hasSsh || !hasServerRoot;
  const editorDisabled =
    disabled || actionBusy || !hasSsh || !hasServerRoot || !hasScript;

  const resetStatus = useCallback(() => {
    setChecked(false);
    setRunning(false);
    setStatusMessage('');
    setError('');
  }, []);

  const applyResult = useCallback((result) => {
    setChecked(true);
    setRunning(Boolean(result?.running));
    setStatusMessage(result?.message ?? '');
  }, []);

  const checkStatus = useCallback(async () => {
    setChecking(true);
    setError('');
    try {
      const result = await checkServerControlStatus({
        sshHost: sshHost?.trim() || undefined,
        serverRootPath: serverRootPath?.trim() || undefined
      });
      applyResult(result);
      if (result?.message && result?.ok === false) {
        setError(result.message);
      }
    } catch (err) {
      setChecked(false);
      setRunning(false);
      setStatusMessage('');
      setError(String(err));
    } finally {
      setChecking(false);
    }
  }, [applyResult, serverRootPath, sshHost]);

  const pollLaunchStatus = useCallback(async () => {
    const pollId = launchPollRef.current + 1;
    launchPollRef.current = pollId;
    setAwaitingLaunch(true);
    setError('');

    for (let attempt = 1; attempt <= LAUNCH_POLL_ATTEMPTS; attempt += 1) {
      await sleep(LAUNCH_POLL_INTERVAL_MS);
      if (launchPollRef.current !== pollId) {
        return;
      }
      try {
        const result = await checkServerControlStatus({
          sshHost: sshHost?.trim() || undefined,
          serverRootPath: serverRootPath?.trim() || undefined
        });
        if (launchPollRef.current !== pollId) {
          return;
        }
        if (result?.running) {
          applyResult(result);
          setAwaitingLaunch(false);
          return;
        }
        setChecked(true);
        setRunning(false);
        setStatusMessage(`Запуск… ${attempt * 5} с, java пока нет`);
      } catch (err) {
        if (launchPollRef.current !== pollId) {
          return;
        }
        setAwaitingLaunch(false);
        setError(String(err));
        return;
      }
    }

    if (launchPollRef.current !== pollId) {
      return;
    }
    setAwaitingLaunch(false);
    try {
      const result = await checkServerControlStatus({
        sshHost: sshHost?.trim() || undefined,
        serverRootPath: serverRootPath?.trim() || undefined
      });
      applyResult(result);
      setError(
        'Java не появился. Проверьте скрипт (карандаш): java в PATH, корень сервера, для run.bat — nogui (добавляется автоматически).'
      );
    } catch (err) {
      setError(String(err));
    }
  }, [applyResult, serverRootPath, sshHost]);

  const startServer = useCallback(async () => {
    launchPollRef.current += 1;
    setAwaitingLaunch(false);
    setStarting(true);
    setError('');
    try {
      const result = await startServerControl({
        sshHost: sshHost?.trim() || undefined,
        serverRootPath: serverRootPath?.trim() || undefined
      });
      applyResult(result);
      if (result?.ok === false && result?.message) {
        setError(result.message);
        return;
      }
      if (!result?.running) {
        void pollLaunchStatus();
      }
    } catch (err) {
      setChecked(false);
      setRunning(false);
      setStatusMessage('');
      setError(String(err));
    } finally {
      setStarting(false);
    }
  }, [applyResult, pollLaunchStatus, serverRootPath, sshHost]);

  const stopServer = useCallback(async () => {
    setStopping(true);
    setError('');
    try {
      const result = await stopServerControl({
        sshHost: sshHost?.trim() || undefined,
        serverRootPath: serverRootPath?.trim() || undefined
      });
      applyResult(result);
      if (result?.ok === false && result?.message) {
        setError(result.message);
      }
    } catch (err) {
      setChecked(false);
      setRunning(false);
      setStatusMessage('');
      setError(String(err));
    } finally {
      setStopping(false);
    }
  }, [applyResult, serverRootPath, sshHost]);

  const handleAction = useCallback(() => {
    if (!checked) {
      void checkStatus();
      return;
    }
    if (running) {
      void stopServer();
      return;
    }
    void startServer();
  }, [checkStatus, checked, running, startServer, stopServer]);

  const action = serverControlActionUi({
    checked,
    running,
    checking: checking || awaitingLaunch,
    starting,
    stopping
  });

  const previewUi = serverControlToOverlayUi({
    checking: checking || awaitingLaunch,
    starting,
    stopping,
    awaitingLaunch,
    checked,
    running,
    error,
    message: statusMessage
  });

  return (
    <div className="serverControlSection">
      <ServerSyncPathField
        id="serverControlRootPath"
        label="Путь к корню сервера"
        value={serverRootPath}
        placeholder="C:/Users/Admin/server или /home/user/server"
        inputDisabled={inputDisabled}
        syncDisabled={controlDisabled}
        laneUi={EMPTY_SYNC_LANE}
        previewUi={previewUi}
        showResult={false}
        actionUi={action}
        onChange={onRootPathChange}
        onBlur={onBlur}
        onAction={handleAction}
        onDismissPreview={resetStatus}
        onEditStart={resetStatus}
      />

      <div className="field serverControlStartSettings">
        <div className="field serverControlStartField">
          <div className="fieldHeader">
            <label htmlFor="serverStartScript">Скрипт запуска в корне сервера</label>
          </div>
          <div className="serverControlScriptRow">
            <input
              id="serverStartScript"
              value={serverStartScript}
              disabled={inputDisabled || actionBusy}
              placeholder="start.sh / start.bat"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              onChange={(event) =>
                onStartSettingChange({ serverStartScript: event.target.value })
              }
              onBlur={onBlur}
            />
            <IconButton
              icon={Pencil}
              label="Редактировать скрипт"
              disabled={editorDisabled}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => setEditorOpen(true)}
            />
          </div>
        </div>
      </div>

      <LaunchScriptEditorModal
        open={editorOpen}
        sshHost={sshHost}
        serverRootPath={serverRootPath}
        scriptName={serverStartScript}
        onClose={() => setEditorOpen(false)}
      />
    </div>
  );
}
