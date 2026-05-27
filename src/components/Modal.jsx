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
  ariaLabel
}) {
  const dialogLabel = ariaLabel ?? title ?? 'Диалог';
  const modal = (
    <div className="modalBackdrop" onMouseDown={onClose}>
      <section
        className={`modal modal-${size}`}
        role="dialog"
        aria-modal="true"
        aria-label={dialogLabel}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="modalHeader">
          <div className={`modalHeaderMain${title ? '' : ' modalHeaderMain--tabsOnly'}`}>
            {title ? <h2>{title}</h2> : null}
            {headerExtra}
          </div>
          <IconButton icon={X} label="Закрыть" className="modalCloseButton" onClick={onClose} />
        </header>
        <div className="modalBody scrollArea">{children}</div>
        {footer ? <footer className="modalFooter">{footer}</footer> : null}
      </section>
    </div>
  );

  return createPortal(modal, getModalPortalRoot());
}
