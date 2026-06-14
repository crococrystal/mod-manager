import { Play, RefreshCw, Square } from 'lucide-react';
import { EMPTY_PREVIEW_OVERLAY } from '../server-sync/serverSyncPreviewUi.js';

export function serverControlToOverlayUi({
  checking,
  starting,
  stopping,
  awaitingLaunch,
  checked,
  running,
  ready,
  error,
  message
}) {
  if (checking || starting || stopping) {
    const pollMessage = awaitingLaunch && message?.trim();
    return {
      ...EMPTY_PREVIEW_OVERLAY,
      checking,
      starting: starting || stopping,
      main: pollMessage || (starting ? 'Запуск…' : stopping ? 'Остановка…' : 'Проверка…')
    };
  }

  if (error?.trim()) {
    return {
      ...EMPTY_PREVIEW_OVERLAY,
      ready: true,
      error: true,
      main: error.trim()
    };
  }

  if (checked) {
    const booting = running && !ready;
    const main =
      message?.trim() ||
      (booting ? 'Сервер запускается' : running ? 'Сервер запущен' : 'Сервер выключен');
    return {
      ...EMPTY_PREVIEW_OVERLAY,
      ready: true,
      ok: running && ready,
      warning: booting,
      error: false,
      main
    };
  }

  return EMPTY_PREVIEW_OVERLAY;
}

export function serverControlActionUi({ checked, running, checking, starting, stopping }) {
  const busy = checking || starting || stopping;
  if (busy) {
    return {
      icon: RefreshCw,
      label: checking ? 'Проверка…' : starting ? 'Запуск…' : 'Остановка…',
      className: 'serverSyncPathSyncBtn--spinning',
      disabled: true
    };
  }
  if (!checked) {
    return {
      icon: RefreshCw,
      label: 'Проверить',
      className: '',
      disabled: false
    };
  }
  if (running) {
    return {
      icon: Square,
      label: 'Остановить',
      className: 'serverSyncPathSyncBtn--confirmUpload',
      disabled: false
    };
  }
  return {
    icon: Play,
    label: 'Запустить',
    className: 'serverSyncPathSyncBtn--confirmUpload',
    disabled: false
  };
}
