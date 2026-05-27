export function DependencyModalBackdrop({ children, uiLocked = false, onClose }) {
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
