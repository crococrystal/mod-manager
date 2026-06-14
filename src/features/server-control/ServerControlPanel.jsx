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
import { RconPanel } from './RconPanel.jsx';
import { serverControlActionUi, serverControlToOverlayUi } from './serverControlUi.js';
import {
  bootElapsedSeconds,
  isServerControlStatusFresh,
  isServerBootInProgress,
  readServerControlSession,
  resetServerControlSession,
  serverControlScopeKey,
  writeServerControlSession
} from './serverControlSessionStore.js';

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

const JAVA_POLL_ATTEMPTS = 12;
const READY_POLL_ATTEMPTS = 120;
const POLL_INTERVAL_MS = 5000;
const BOOT_POLL_INTERVAL_MS = 5000;
const BOOT_TIMER_TICK_MS = 1000;

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
  const [ready, setReady] = useState(false);
  const [statusMessage, setStatusMessage] = useState('');
  const [checking, setChecking] = useState(false);
  const [starting, setStarting] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState('');
  const [editorOpen, setEditorOpen] = useState(false);
  const [awaitingLaunch, setAwaitingLaunch] = useState(false);
  const [bootTracking, setBootTracking] = useState(false);
  const [bootElapsedSec, setBootElapsedSec] = useState(0);
  const launchPollRef = useRef(0);
  const bootTimerStartRef = useRef(null);
  const scopeKey = serverControlScopeKey(sshHost, serverRootPath);

  const persistStatus = useCallback(
    (patch) => {
      writeServerControlSession(scopeKey, patch);
    },
    [scopeKey]
  );

  const beginBootTracking = useCallback(
    (startedAt = Date.now()) => {
      const session = readServerControlSession(scopeKey);
      const nextStartedAt = session.bootStartedAt || startedAt;
      bootTimerStartRef.current = nextStartedAt;
      setBootTracking(true);
      setAwaitingLaunch(true);
      setBootElapsedSec(bootElapsedSeconds(nextStartedAt));
      persistStatus({
        bootTracking: true,
        bootStartedAt: nextStartedAt
      });
    },
    [persistStatus, scopeKey]
  );

  const endBootTracking = useCallback(() => {
    setBootTracking(false);
    setAwaitingLaunch(false);
    bootTimerStartRef.current = null;
    setBootElapsedSec(0);
    persistStatus({
      bootTracking: false,
      bootStartedAt: 0
    });
  }, [persistStatus]);

  useEffect(() => {
    const session = readServerControlSession(scopeKey);
    const tracking = isServerBootInProgress(session);
    setChecked(session.checked);
    setRunning(session.running);
    setReady(session.ready);
    setStatusMessage(session.statusMessage);
    setError(session.error);
    setBootTracking(tracking);
    setAwaitingLaunch(tracking);
    setChecking(false);
    setStarting(false);
    setStopping(false);
    if (tracking && session.bootStartedAt) {
      bootTimerStartRef.current = session.bootStartedAt;
      setBootElapsedSec(bootElapsedSeconds(session.bootStartedAt));
    } else {
      bootTimerStartRef.current = null;
      setBootElapsedSec(0);
    }
  }, [scopeKey]);

  useEffect(() => {
    persistStatus({ error });
  }, [error, persistStatus]);

  const booting = checked && running && !ready;
  const bootWaiting = bootTracking && !running;
  const showBootTimer = starting || awaitingLaunch || booting || bootTracking;

  useEffect(
    () => () => {
      launchPollRef.current += 1;
    },
    []
  );

  useEffect(() => {
    if (!showBootTimer) {
      return undefined;
    }

    if (bootTimerStartRef.current === null) {
      bootTimerStartRef.current = Date.now();
      persistStatus({ bootStartedAt: bootTimerStartRef.current, bootTracking: true });
    }

    const tick = () => {
      const startedAt = bootTimerStartRef.current;
      if (!startedAt) {
        return;
      }
      setBootElapsedSec(
        Math.max(1, Math.ceil((Date.now() - startedAt) / BOOT_TIMER_TICK_MS))
      );
    };

    tick();
    const intervalId = window.setInterval(tick, BOOT_TIMER_TICK_MS);
    return () => {
      window.clearInterval(intervalId);
    };
  }, [persistStatus, showBootTimer]);

  const hasSsh = Boolean(sshHost?.trim());
  const hasServerRoot = Boolean(serverRootPath?.trim());
  const hasScript = Boolean(serverStartScript?.trim());
  const rowBusy = checking || starting || stopping || awaitingLaunch;
  const controlDisabled =
    disabled || rowBusy || actionBusy || !hasSsh || !hasServerRoot;
  const editorDisabled =
    disabled || actionBusy || !hasSsh || !hasServerRoot || !hasScript;

  const resetStatus = useCallback(() => {
    endBootTracking();
    const session = resetServerControlSession(scopeKey);
    setChecked(session.checked);
    setRunning(session.running);
    setReady(session.ready);
    setStatusMessage(session.statusMessage);
    setError(session.error);
  }, [endBootTracking, scopeKey]);

  const applyResult = useCallback(
    (result) => {
      const checked = true;
      const running = Boolean(result?.running);
      const ready = Boolean(result?.ready);
      const statusMessage = result?.message ?? '';
      setChecked(checked);
      setRunning(running);
      setReady(ready);
      setStatusMessage(statusMessage);
      const bootPatch = ready
        ? { bootTracking: false, bootStartedAt: 0 }
        : running
          ? {
              bootTracking: true,
              bootStartedAt:
                readServerControlSession(scopeKey).bootStartedAt || Date.now()
            }
          : readServerControlSession(scopeKey).bootTracking
            ? { bootTracking: true }
            : {};
      persistStatus({
        checked,
        running,
        ready,
        statusMessage,
        cachedAt: Date.now(),
        ...bootPatch
      });
      if (ready) {
        setBootTracking(false);
        setAwaitingLaunch(false);
        bootTimerStartRef.current = null;
        setBootElapsedSec(0);
      } else if (running) {
        const startedAt =
          readServerControlSession(scopeKey).bootStartedAt || Date.now();
        bootTimerStartRef.current = startedAt;
        setBootTracking(true);
        setBootElapsedSec(bootElapsedSeconds(startedAt));
      }
    },
    [persistStatus, scopeKey]
  );

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
      return result;
    } catch (err) {
      setChecked(false);
      setRunning(false);
      setReady(false);
      setStatusMessage('');
      persistStatus({
        checked: false,
        running: false,
        ready: false,
        statusMessage: '',
        cachedAt: 0,
        bootTracking: false,
        bootStartedAt: 0
      });
      endBootTracking();
      setError(String(err));
      throw err;
    } finally {
      setChecking(false);
    }
  }, [applyResult, endBootTracking, serverRootPath, sshHost]);

  const ensureServerRunning = useCallback(async () => {
    const session = readServerControlSession(scopeKey);
    if (isServerBootInProgress(session)) {
      return {
        ok: true,
        running: session.running,
        ready: false,
        message: session.statusMessage
      };
    }
    if (isServerControlStatusFresh(session)) {
      setChecked(session.checked);
      setRunning(session.running);
      setReady(session.ready);
      setStatusMessage(session.statusMessage);
      return {
        ok: true,
        running: session.running,
        ready: session.ready,
        message: session.statusMessage
      };
    }
    return checkStatus();
  }, [checkStatus, scopeKey]);

  const pollLaunchStatus = useCallback(async () => {
    const pollId = launchPollRef.current + 1;
    launchPollRef.current = pollId;
    beginBootTracking();
    setError('');

    let hasJava = false;

    for (let attempt = 1; attempt <= JAVA_POLL_ATTEMPTS; attempt += 1) {
      await sleep(POLL_INTERVAL_MS);
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
          hasJava = true;
          applyResult(result);
          if (result?.ready) {
            endBootTracking();
            return;
          }
          break;
        }
        setChecked(true);
        setRunning(false);
        setReady(false);
      } catch (err) {
        if (launchPollRef.current !== pollId) {
          return;
        }
        endBootTracking();
        setError(String(err));
        return;
      }
    }

    if (launchPollRef.current !== pollId) {
      return;
    }

    if (!hasJava) {
      endBootTracking();
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
      return;
    }

    for (let attempt = 1; attempt <= READY_POLL_ATTEMPTS; attempt += 1) {
      await sleep(POLL_INTERVAL_MS);
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
        if (!result?.running) {
          applyResult(result);
          endBootTracking();
          setError('Сервер остановился во время загрузки.');
          return;
        }
        applyResult(result);
        if (result?.ready) {
          endBootTracking();
          return;
        }
      } catch (err) {
        if (launchPollRef.current !== pollId) {
          return;
        }
        endBootTracking();
        setError(String(err));
        return;
      }
    }

    if (launchPollRef.current !== pollId) {
      return;
    }
    endBootTracking();
  }, [applyResult, beginBootTracking, endBootTracking, serverRootPath, sshHost]);

  useEffect(() => {
    const session = readServerControlSession(scopeKey);
    if (!isServerBootInProgress(session)) {
      return undefined;
    }
    if (!sshHost?.trim() || !serverRootPath?.trim()) {
      return undefined;
    }
    void pollLaunchStatus();
    return undefined;
    // Возобновляем опрос только при смене scope (повторное открытие настроек).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey]);

  useEffect(() => {
    if (!booting || rowBusy || !hasSsh || !hasServerRoot) {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      void checkStatus();
    }, BOOT_POLL_INTERVAL_MS);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [booting, checkStatus, hasServerRoot, hasSsh, rowBusy]);

  const startServer = useCallback(async () => {
    launchPollRef.current += 1;
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
      if (!result?.running || !result?.ready) {
        beginBootTracking();
        void pollLaunchStatus();
      }
    } catch (err) {
      setChecked(false);
      setRunning(false);
      setReady(false);
      setStatusMessage('');
      persistStatus({
        checked: false,
        running: false,
        ready: false,
        statusMessage: '',
        cachedAt: 0,
        bootTracking: false,
        bootStartedAt: 0
      });
      endBootTracking();
      setError(String(err));
    } finally {
      setStarting(false);
    }
  }, [applyResult, beginBootTracking, endBootTracking, persistStatus, pollLaunchStatus, serverRootPath, sshHost]);

  const stopServer = useCallback(async () => {
    launchPollRef.current += 1;
    endBootTracking();
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
      setReady(false);
      setStatusMessage('');
      persistStatus({
        checked: false,
        running: false,
        ready: false,
        statusMessage: '',
        cachedAt: 0,
        bootTracking: false,
        bootStartedAt: 0
      });
      endBootTracking();
      setError(String(err));
    } finally {
      setStopping(false);
    }
  }, [applyResult, endBootTracking, persistStatus, serverRootPath, sshHost]);

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
    checking: checking || bootWaiting,
    starting: starting || bootWaiting || booting,
    stopping,
    awaitingLaunch: awaitingLaunch || bootTracking,
    checked,
    running,
    ready,
    error,
    message: statusMessage,
    showBootTimer,
    bootElapsedSec
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

      <RconPanel
        sshHost={sshHost}
        serverRootPath={serverRootPath}
        disabled={disabled}
        actionBusy={actionBusy || rowBusy}
        inputDisabled={inputDisabled}
        serverRunning={running}
        ensureServerRunning={ensureServerRunning}
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
