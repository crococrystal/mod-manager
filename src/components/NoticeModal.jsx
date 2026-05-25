import { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { AlertTriangle, CheckCircle2 } from 'lucide-react';

const AUTO_CLOSE_MS = 2000;

export function NoticeModal({ tone = 'bad', message, onClose }) {
  useEffect(() => {
    if (!message) return undefined;
    const timer = window.setTimeout(onClose, AUTO_CLOSE_MS);
    return () => window.clearTimeout(timer);
  }, [message, onClose]);

  if (!message) return null;

  const Icon = tone === 'ok' ? CheckCircle2 : AlertTriangle;

  return createPortal(
    <div className="noticeToastLayer" aria-live="polite">
      <div className="noticeToast" role="status">
        <Icon className="noticeToastIcon" size={28} strokeWidth={1.75} />
        <p>{message}</p>
      </div>
    </div>,
    document.body
  );
}
