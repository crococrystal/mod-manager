import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { getModalPortalRoot } from '../lib/modalPortal.js';
import { ArrowUpCircle, Download } from 'lucide-react';

function progressLabel(progress) {
  if (progress?.phase === 'install') return 'Установка…';
  if (progress?.percent != null) return `Загрузка ${progress.percent}%`;
  return 'Загрузка…';
}

export function UpdateModal({
  currentVersion,
  version,
  status,
  progress,
  error,
  onInstall,
  onDismiss
}) {
  useEffect(() => {
    function handleEscape(event) {
      if (event.key === 'Escape' && status !== 'installing') onDismiss?.();
    }
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [status, onDismiss]);

  if (!version) return null;

  const installing = status === 'installing';
  const Icon = installing ? Download : ArrowUpCircle;

  function handleBackdrop(event) {
    if (event.target !== event.currentTarget || installing) return;
    onDismiss?.();
  }

  return createPortal(
    <div
      className="noticeToastLayer noticeToastLayerInteractive"
      onMouseDown={handleBackdrop}
      aria-live="polite"
    >
      <div
        className="noticeToast noticeToastConfirm noticeToastUpdate"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="updateModalTitle"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <Icon className="noticeToastIcon" size={28} strokeWidth={1.75} />
        <p id="updateModalTitle" className="noticeToastTitle">
          {installing ? progressLabel(progress) : `Доступна версия ${version}`}
        </p>
        {!installing ? (
          <p className="noticeToastHint">Сейчас установлена {currentVersion || '—'}</p>
        ) : null}
        {installing ? (
          <div className="noticeToastProgressTrack" aria-hidden="true">
            <div
              className={`noticeToastProgressBar${progress?.percent == null ? ' indeterminate' : ''}`}
              style={progress?.percent != null ? { width: `${progress.percent}%` } : undefined}
            />
          </div>
        ) : null}
        {error ? <p className="noticeToastError">{error}</p> : null}
        {!installing ? (
          <div className="noticeToastActions">
            <button type="button" className="button noticeToastBtn" onClick={onInstall}>
              Обновить
            </button>
            <button type="button" className="button button-ghost noticeToastBtn" onClick={onDismiss}>
              Позже
            </button>
          </div>
        ) : null}
      </div>
    </div>,
    getModalPortalRoot()
  );
}
