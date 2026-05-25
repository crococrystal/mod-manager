import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { IconButton } from './Button.jsx';

export function Modal({ title, subtitle, children, footer, onClose, size = 'default' }) {
  const modal = (
    <div className="modalBackdrop" onMouseDown={onClose}>
      <section
        className={`modal modal-${size}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="modalHeader">
          <div>
            <h2>{title}</h2>
          </div>
          <IconButton icon={X} label="Закрыть" onClick={onClose} />
        </header>
        <div className="modalBody scrollArea">{children}</div>
        {footer ? <footer className="modalFooter">{footer}</footer> : null}
      </section>
    </div>
  );

  return createPortal(modal, document.body);
}
