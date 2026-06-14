import { useEffect } from 'react';

export function DependencyModalBackdrop({ children, uiLocked = false, onClose }) {
  useEffect(() => {
    if (!onClose) return undefined;

    function handleEscape(event) {
      if (event.key === 'Escape' && !uiLocked) onClose();
    }

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [uiLocked, onClose]);

  function handleBackdropMouseDown(event) {
    if (event.target !== event.currentTarget) return;
    if (!uiLocked && onClose) onClose();
  }

  return (
    <div className="dependencyModalBackdrop" onMouseDown={handleBackdropMouseDown}>
      {children}
    </div>
  );
}
