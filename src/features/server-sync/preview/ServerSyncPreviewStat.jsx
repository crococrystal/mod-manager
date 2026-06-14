import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { getModalPortalRoot } from '../../../lib/modalPortal.js';

const HIDE_DELAY_MS = 80;

export function ServerSyncPreviewStat({ icon: Icon, count, variant, label, title, children }) {
  const triggerRef = useRef(null);
  const hideTimerRef = useRef(null);
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState(null);

  const reposition = useCallback(() => {
    const node = triggerRef.current;
    if (!node) return;
    const rect = node.getBoundingClientRect();
    const margin = 12;
    const maxWidth = Math.min(680, window.innerWidth - margin * 2);
    const estHeight = 280;
    let top = rect.bottom + 8;
    let left = Math.max(margin, rect.left);
    let transform = 'translate(0, 0)';

    if (top + estHeight > window.innerHeight - margin) {
      top = rect.top - 8;
      transform = 'translate(0, -100%)';
    }

    if (left + maxWidth > window.innerWidth - margin) {
      left = Math.max(margin, window.innerWidth - margin - maxWidth);
    }

    setCoords({ top, left, transform });
  }, []);

  const cancelHide = useCallback(() => {
    if (hideTimerRef.current) {
      clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
  }, []);

  const show = useCallback(() => {
    cancelHide();
    reposition();
    setOpen(true);
  }, [cancelHide, reposition]);

  const scheduleHide = useCallback(() => {
    cancelHide();
    hideTimerRef.current = window.setTimeout(() => {
      hideTimerRef.current = null;
      setOpen(false);
    }, HIDE_DELAY_MS);
  }, [cancelHide]);

  useEffect(() => {
    if (!open) return undefined;

    const handleReflow = () => reposition();
    window.addEventListener('scroll', handleReflow, true);
    window.addEventListener('resize', handleReflow);

    return () => {
      window.removeEventListener('scroll', handleReflow, true);
      window.removeEventListener('resize', handleReflow);
    };
  }, [open, reposition]);

  useEffect(() => () => cancelHide(), [cancelHide]);

  function stopOverlayDismiss(event) {
    event.stopPropagation();
  }

  const tooltip =
    open && coords
      ? createPortal(
          <div
            className="serverSyncPreviewStatTooltip serverSyncPreviewStatTooltip--portal"
            style={{
              top: `${coords.top}px`,
              left: `${coords.left}px`,
              transform: coords.transform
            }}
            role="tooltip"
            onMouseEnter={show}
            onMouseLeave={scheduleHide}
            onClick={stopOverlayDismiss}
            onMouseDown={stopOverlayDismiss}
          >
            <div className={`serverSyncPreviewStatTooltipHead serverSyncPreviewStatTooltipHead--${variant}`}>
              <span className="serverSyncPreviewStatTooltipBadge" aria-hidden="true">
                <Icon size={14} strokeWidth={2.2} />
                <span className="serverSyncPreviewStatCount">{count}</span>
              </span>
              <span className="serverSyncPreviewStatTooltipTitle">{title}</span>
            </div>
            <div className="serverSyncPreviewStatTooltipBody">{children}</div>
          </div>,
          getModalPortalRoot()
        )
      : null;

  return (
    <>
      <span className={`serverSyncPreviewStat serverSyncPreviewStat--${variant}`}>
        <span
          ref={triggerRef}
          className="serverSyncPreviewStatTrigger"
          tabIndex={0}
          aria-label={label}
          onMouseEnter={show}
          onMouseLeave={scheduleHide}
          onFocus={show}
          onBlur={scheduleHide}
          onClick={stopOverlayDismiss}
          onMouseDown={stopOverlayDismiss}
        >
          <Icon size={12} strokeWidth={2.2} aria-hidden="true" />
          <span className="serverSyncPreviewStatCount">{count}</span>
        </span>
      </span>
      {tooltip}
    </>
  );
}
