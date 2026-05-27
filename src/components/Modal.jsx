import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { getModalPortalRoot } from '../lib/modalPortal.js';
import { IconButton } from './Button.jsx';

export function Modal({
  title,
  subtitle,
  children,
  footer,
  headerExtra,
  onClose,
  size = 'default',
  className = '',
  showClose = true,
  ariaLabel
}) {
  const dialogLabel = ariaLabel ?? title ?? 'Диалог';
  const showHeader = Boolean(title || headerExtra || showClose);
  const modal = (
    <div className="modalBackdrop" onMouseDown={onClose}>
      <section
        className={`modal modal-${size}${className ? ` ${className}` : ''}`}
        role="dialog"
        aria-modal="true"
        aria-label={dialogLabel}
        onMouseDown={(event) => event.stopPropagation()}
      >
        {showHeader ? (
          <header className="modalHeader">
            <div className={`modalHeaderMain${title ? '' : ' modalHeaderMain--tabsOnly'}`}>
              {title ? <h2>{title}</h2> : null}
              {headerExtra}
            </div>
            {showClose ? (
              <IconButton icon={X} label="Закрыть" className="modalCloseButton" onClick={onClose} />
            ) : null}
          </header>
        ) : null}
        <div className="modalBody scrollArea">{children}</div>
        {footer ? <footer className="modalFooter">{footer}</footer> : null}
      </section>
    </div>
  );

  return createPortal(modal, getModalPortalRoot());
}
