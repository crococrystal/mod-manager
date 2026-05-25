import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { modModalSubtitle } from '../../lib/modMeta.jsx';
import { ModModalHead } from './ModModalHead.jsx';

export function DescriptionDialog({ mod, busy, onClose, onSave }) {
  const [description, setDescription] = useState(mod?.description ?? '');
  const textareaRef = useRef(null);

  useEffect(() => {
    setDescription(mod?.description ?? '');
    window.setTimeout(() => textareaRef.current?.focus(), 0);
  }, [mod?.key, mod?.description]);

  if (!mod) return null;

  async function save() {
    await onSave(mod.key, { description });
    onClose();
  }

  return createPortal(
    <div className="dependencyModalBackdrop" onMouseDown={() => !busy && onClose()}>
      <div className="dependencyModalStack compactModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead mod={mod} subtitle={modModalSubtitle(mod, { section: 'Описание' })} />
        <div className="compactModal" role="dialog" aria-modal="true" aria-label="Описание мода">
          <textarea
            ref={textareaRef}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder="Что это за мод, зачем он в сборке"
          />
          <div className="compactModalActions">
            <button type="button" className="button button-ghost" onClick={onClose} disabled={busy}>
              Отмена
            </button>
            <button type="button" className="button" onClick={save} disabled={busy}>
              Сохранить
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body
  );
}
