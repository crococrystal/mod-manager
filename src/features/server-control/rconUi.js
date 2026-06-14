import { ArrowRight, RefreshCw } from 'lucide-react';
import { EMPTY_PREVIEW_OVERLAY } from '../server-sync/serverSyncPreviewUi.js';

export function rconToOverlayUi({ checking, sending, error, checkingHint }) {
  if (checking || sending) {
    return {
      ...EMPTY_PREVIEW_OVERLAY,
      checking: true,
      starting: Boolean(sending),
      main:
        checkingHint ||
        (sending ? 'Отправка…' : 'Проверка RCON…')
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

  return EMPTY_PREVIEW_OVERLAY;
}

export function rconActionUi({ connected, checking, sending, hasCommand }) {
  const busy = checking || sending;
  if (busy) {
    return {
      icon: RefreshCw,
      label: checking ? 'Проверка…' : 'Отправка…',
      className: 'serverSyncPathSyncBtn--spinning',
      disabled: true
    };
  }

  if (connected) {
    return {
      icon: ArrowRight,
      label: 'Отправить',
      className: hasCommand ? 'serverSyncPathSyncBtn--confirmUpload' : '',
      disabled: !hasCommand
    };
  }

  return {
    icon: RefreshCw,
    label: 'Проверить',
    className: '',
    disabled: false
  };
}
