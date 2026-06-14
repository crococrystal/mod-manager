import { useCallback, useEffect, useState } from 'react';
import { ServerSyncPathField } from '../server-sync/ServerSyncPathField.jsx';
import { checkServerRcon, sendServerRconCommand } from './rconApi.js';
import { rconActionUi, rconToOverlayUi } from './rconUi.js';

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

const IDLE_HINT =
  'Сначала проверяется статус сервера выше, затем server.properties и RCON.';

export function RconPanel({
  sshHost,
  serverRootPath,
  disabled,
  actionBusy,
  inputDisabled,
  serverRunning,
  ensureServerRunning
}) {
  const [connected, setConnected] = useState(false);
  const [checking, setChecking] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');
  const [command, setCommand] = useState('');
  const [hint, setHint] = useState(IDLE_HINT);
  const [checkingHint, setCheckingHint] = useState('');

  const hasSsh = Boolean(sshHost?.trim());
  const hasServerRoot = Boolean(serverRootPath?.trim());
  const hasCommand = Boolean(command.trim());
  const rowBusy = checking || sending;
  const controlDisabled =
    disabled || rowBusy || actionBusy || !hasSsh || !hasServerRoot;

  const reset = useCallback(() => {
    setConnected(false);
    setError('');
    setCommand('');
    setHint(IDLE_HINT);
    setCheckingHint('');
  }, []);

  useEffect(() => {
    reset();
  }, [reset, serverRootPath, sshHost]);

  useEffect(() => {
    if (!serverRunning) {
      setConnected(false);
    }
  }, [serverRunning]);

  const requireRunningServer = useCallback(async () => {
    setCheckingHint('Проверка статуса сервера…');
    const status = await ensureServerRunning();
    if (!status?.running) {
      throw new Error('Сервер выключен. Запустите сервер и повторите проверку RCON.');
    }
    return status;
  }, [ensureServerRunning]);

  const checkAccess = useCallback(async () => {
    setChecking(true);
    setError('');
    setHint(IDLE_HINT);
    try {
      await requireRunningServer();
      setCheckingHint('Читаем server.properties…');
      setHint('Читаем server.properties по SSH…');
      const result = await checkServerRcon({
        sshHost: sshHost?.trim() || undefined,
        serverRootPath: serverRootPath?.trim() || undefined
      });
      if (result?.ok) {
        setConnected(true);
        const parts = [
          result.detail,
          result.message,
          'Введите команду: list, say, stop…'
        ].filter(Boolean);
        setHint(parts.join(' '));
      } else {
        setConnected(false);
        setError(result?.detail || result?.message || 'RCON недоступен.');
      }
    } catch (err) {
      setConnected(false);
      setError(String(err));
    } finally {
      setChecking(false);
      setCheckingHint('');
    }
  }, [requireRunningServer, serverRootPath, sshHost]);

  const sendCommand = useCallback(async () => {
    const trimmed = command.trim();
    if (!trimmed) {
      return;
    }
    setSending(true);
    setError('');
    try {
      await requireRunningServer();
      setCheckingHint('Отправка команды…');
      const result = await sendServerRconCommand({
        sshHost: sshHost?.trim() || undefined,
        serverRootPath: serverRootPath?.trim() || undefined,
        command: trimmed
      });
      const output = result?.output?.trim() || result?.message?.trim();
      setHint(output || 'Команда выполнена.');
      setCommand('');
    } catch (err) {
      setError(String(err));
      setConnected(false);
    } finally {
      setSending(false);
      setCheckingHint('');
    }
  }, [command, requireRunningServer, serverRootPath, sshHost]);

  const handleAction = useCallback(() => {
    if (connected) {
      void sendCommand();
      return;
    }
    void checkAccess();
  }, [checkAccess, connected, sendCommand]);

  const handleKeyDown = useCallback(
    (event) => {
      if (event.key !== 'Enter' || !connected || rowBusy || !hasCommand) {
        return;
      }
      event.preventDefault();
      void sendCommand();
    },
    [connected, hasCommand, rowBusy, sendCommand]
  );

  const previewUi = rconToOverlayUi({
    checking: checking || sending,
    sending: false,
    error,
    checkingHint
  });

  const action = rconActionUi({
    connected,
    checking: checking || sending,
    sending,
    hasCommand
  });

  return (
    <div className="rconSection rconSection--nested">
      <ServerSyncPathField
        id="serverRconCommand"
        label="RCON"
        hint={hint}
        value={command}
        placeholder={
          connected ? 'list, say Hello, stop…' : 'Проверьте доступ к RCON'
        }
        inputDisabled={inputDisabled || !connected}
        syncDisabled={controlDisabled || (connected && !hasCommand)}
        laneUi={EMPTY_SYNC_LANE}
        previewUi={previewUi}
        showResult={false}
        actionUi={action}
        onChange={(event) => setCommand(event.target.value)}
        onKeyDown={handleKeyDown}
        onAction={handleAction}
        onDismissPreview={() => setError('')}
        onEditStart={() => setError('')}
      />
    </div>
  );
}
