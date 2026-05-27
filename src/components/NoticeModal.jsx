import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { getModalPortalRoot } from '../lib/modalPortal.js';
import { AlertTriangle, CheckCircle2 } from 'lucide-react';

const AUTO_CLOSE_MS = 2000;

export function NoticeModal({ tone = 'bad', message, onClose, confirm }) {
  const isConfirm = Boolean(confirm && message);

  useEffect(() => {
    if (!message || isConfirm) return undefined;
    const timer = window.setTimeout(onClose, AUTO_CLOSE_MS);
    return () => window.clearTimeout(timer);
  }, [message, onClose, isConfirm]);

  useEffect(() => {
    if (!isConfirm) return undefined;

    function handleEscape(event) {
      if (event.key === 'Escape' && !confirm?.busy) {
        (confirm.onCancel ?? onClose)();
      }
    }

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isConfirm, confirm, onClose]);

  if (!message) return null;

  const Icon = tone === 'ok' ? CheckCircle2 : AlertTriangle;

  function handleBackdrop(event) {
    if (event.target !== event.currentTarget || confirm?.busy) return;
    (confirm?.onCancel ?? onClose)();
  }

  return createPortal(
    <div
      className={`noticeToastLayer${isConfirm ? ' noticeToastLayerInteractive' : ''}`}
      onMouseDown={isConfirm ? handleBackdrop : undefined}
      aria-live={isConfirm ? undefined : 'polite'}
    >
      <div
        className={`noticeToast${isConfirm ? ' noticeToastConfirm' : ''}`}
        role={isConfirm ? 'alertdialog' : 'status'}
        aria-modal={isConfirm ? 'true' : undefined}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <Icon className="noticeToastIcon" size={28} strokeWidth={1.75} />
        <p>{message}</p>
        {isConfirm ? (
          <div className="noticeToastActions">
            <button
              type="button"
              className="button noticeToastBtn"
              onClick={confirm.onConfirm}
              disabled={confirm.busy}
            >
              {confirm.confirmLabel ?? 'Да'}
            </button>
            <button
              type="button"
              className="button button-ghost noticeToastBtn"
              onClick={() => (confirm.onCancel ?? onClose)()}
              disabled={confirm.busy}
            >
              {confirm.cancelLabel ?? 'Отмена'}
            </button>
          </div>
        ) : null}
      </div>
    </div>,
    getModalPortalRoot()
  );
}
