import { useCallback, useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import {
  canCheckForUpdates,
  checkForAppUpdate,
  formatUpdateError,
  installAppUpdate
} from '../lib/updater.js';

export function useAppUpdater() {
  const [status, setStatus] = useState('idle');
  const [pendingUpdate, setPendingUpdate] = useState(null);
  const [progress, setProgress] = useState(null);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState(null);
  const [appVersion, setAppVersion] = useState('');
  const [modalDismissed, setModalDismissed] = useState(false);
  const updateRef = useRef(null);
  const noticeTimerRef = useRef(null);

  useEffect(() => {
    if (!canCheckForUpdates()) return;
    void getVersion()
      .then((version) => setAppVersion(version))
      .catch(() => {});
  }, []);

  useEffect(() => () => {
    if (noticeTimerRef.current) clearTimeout(noticeTimerRef.current);
  }, []);

  const clearNoticeTimer = useCallback(() => {
    if (!noticeTimerRef.current) return;
    clearTimeout(noticeTimerRef.current);
    noticeTimerRef.current = null;
  }, []);

  const showUpToDateNotice = useCallback(() => {
    clearNoticeTimer();
    setNotice({ tone: 'ok', text: 'У вас актуальная версия' });
    noticeTimerRef.current = window.setTimeout(() => {
      setNotice(null);
      noticeTimerRef.current = null;
    }, 3000);
  }, [clearNoticeTimer]);

  const rememberUpdate = useCallback((update) => {
    updateRef.current = update;
    setPendingUpdate({
      version: update.version,
      currentVersion: update.currentVersion
    });
    setModalDismissed(false);
    setStatus('available');
  }, []);

  const check = useCallback(async ({ silent = false } = {}) => {
    if (!canCheckForUpdates()) return { skipped: true };
    setStatus('checking');
    setError('');
    if (!silent) {
      clearNoticeTimer();
      setNotice(null);
      setProgress(null);
    }
    try {
      const update = await checkForAppUpdate();
      if (!update) {
        updateRef.current = null;
        setPendingUpdate(null);
        setStatus('idle');
        if (!silent) showUpToDateNotice();
        return { upToDate: true };
      }
      rememberUpdate(update);
      return { upToDate: false, update };
    } catch (err) {
      updateRef.current = null;
      setPendingUpdate(null);
      setStatus('idle');
      const message = formatUpdateError(err);
      if (!silent) setNotice({ tone: 'error', text: message });
      return { error: message };
    }
  }, [clearNoticeTimer, rememberUpdate, showUpToDateNotice]);

  const install = useCallback(async ({ suppressModal = false } = {}) => {
    const update = updateRef.current;
    if (!update) return;
    setStatus('installing');
    setError('');
    setProgress(null);
    if (!suppressModal) setModalDismissed(false);
    try {
      await installAppUpdate(update, setProgress);
    } catch (err) {
      setStatus('available');
      setProgress(null);
      setError(formatUpdateError(err));
    }
  }, []);

  const dismissModal = useCallback(() => {
    if (status === 'installing') return;
    setModalDismissed(true);
  }, [status]);

  const checkAndInstall = useCallback(
    async ({ silent = false, fromSettings = false } = {}) => {
      if (!updateRef.current) {
        const result = await check({ silent });
        if (fromSettings) setModalDismissed(true);
        if (result?.upToDate || result?.error || result?.skipped) return result;
      }
      if (fromSettings) setModalDismissed(true);
      if (!updateRef.current) return { skipped: true };
      await install({ suppressModal: fromSettings });
      return { upToDate: false };
    },
    [check, install]
  );

  const showUpdateModal =
    Boolean(pendingUpdate) &&
    !modalDismissed &&
    (status === 'available' || status === 'installing');

  const updateBusy = status === 'checking' || status === 'installing';

  return {
    status,
    pendingUpdate,
    progress,
    error,
    notice,
    appVersion,
    showUpdateModal,
    updateBusy,
    check,
    checkAndInstall,
    install,
    dismissModal,
    clearNotice: () => {
      clearNoticeTimer();
      setNotice(null);
    }
  };
}
