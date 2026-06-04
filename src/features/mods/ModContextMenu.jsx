import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Copy, ExternalLink, Trash2 } from 'lucide-react';
import { getModalPortalRoot } from '../../lib/modalPortal.js';

const EDGE_PADDING = 8;

export function ModContextMenu({
  menu,
  count = 0,
  label,
  busy = false,
  onClose,
  onCopy,
  onOpenPage,
  onDelete
}) {
  const menuRef = useRef(null);
  const [position, setPosition] = useState(() => ({ x: menu?.x ?? 0, y: menu?.y ?? 0 }));

  useLayoutEffect(() => {
    if (!menu) return;
    setPosition({ x: menu.x, y: menu.y });
  }, [menu]);

  useLayoutEffect(() => {
    const node = menuRef.current;
    if (!menu || !node) return;

    const rect = node.getBoundingClientRect();
    setPosition({
      x: Math.max(EDGE_PADDING, Math.min(menu.x, window.innerWidth - rect.width - EDGE_PADDING)),
      y: Math.max(EDGE_PADDING, Math.min(menu.y, window.innerHeight - rect.height - EDGE_PADDING))
    });
  }, [menu, count]);

  useEffect(() => {
    if (!menu) return undefined;

    function closeOnEscape(event) {
      if (event.key === 'Escape') onClose?.();
    }

    function closeOnPointer(event) {
      if (menuRef.current?.contains(event.target)) return;
      onClose?.();
    }

    window.addEventListener('keydown', closeOnEscape);
    window.addEventListener('mousedown', closeOnPointer);
    window.addEventListener('scroll', onClose, true);
    return () => {
      window.removeEventListener('keydown', closeOnEscape);
      window.removeEventListener('mousedown', closeOnPointer);
      window.removeEventListener('scroll', onClose, true);
    };
  }, [menu, onClose]);

  if (!menu || count <= 0) return null;

  const selectedText = count > 1 ? `Выбрано: ${count}` : label || 'Мод';

  return createPortal(
    <div className="modContextMenuLayer" onContextMenu={(event) => event.preventDefault()}>
      <div
        ref={menuRef}
        className="modContextMenu"
        style={{ transform: `translate3d(${position.x}px, ${position.y}px, 0)` }}
        role="menu"
      >
        <div className="modContextMenuCaption" title={selectedText}>{selectedText}</div>
        <button type="button" role="menuitem" onClick={onCopy} disabled={busy}>
          <Copy size={15} />
          {count > 1 ? 'Скопировать файлы' : 'Скопировать файл'}
        </button>
        {onOpenPage ? (
          <button type="button" role="menuitem" onClick={onOpenPage} disabled={busy}>
            <ExternalLink size={15} />
            Открыть страницу
          </button>
        ) : null}
        <button
          type="button"
          role="menuitem"
          className="danger"
          onClick={onDelete}
          disabled={busy}
        >
          <Trash2 size={15} />
          {count > 1 ? 'Удалить выбранные' : 'Удалить мод'}
        </button>
      </div>
    </div>,
    getModalPortalRoot()
  );
}
