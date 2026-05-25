import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { BookOpen, Wrench } from 'lucide-react';
import { sideOptions } from '../../lib/modMeta.jsx';
import { ModCover } from './ModCover.jsx';

export function TagsDialog({ mod, busy, onClose, onSave }) {
  const [side, setSide] = useState(mod?.side ?? 'universal');
  const [library, setLibrary] = useState(Boolean(mod?.library));
  const [technical, setTechnical] = useState(Boolean(mod?.technical));

  useEffect(() => {
    setSide(mod?.side === 'unknown' ? 'universal' : mod?.side ?? 'universal');
    setLibrary(Boolean(mod?.library));
    setTechnical(Boolean(mod?.technical));
  }, [mod?.key, mod?.side, mod?.library, mod?.technical]);

  if (!mod) return null;

  async function save() {
    await onSave(mod.key, { side, library, technical });
    onClose();
  }

  return createPortal(
    <div className="dependencyModalBackdrop" onMouseDown={() => !busy && onClose()}>
      <div className="dependencyModalStack compactModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <div className="dependencyModalHead">
          <ModCover mod={mod} size="tile" />
          <div className="dependencyModalHeadText">
            <p className="dependencyModalSubtitle">{mod.displayName}</p>
            <h3 className="dependencyModalTitle">Метки</h3>
          </div>
        </div>
        <div className="compactModal" role="dialog" aria-modal="true" aria-label="Метки мода">
          <div className="tagDialogGroup">
            <span>Сторона</span>
            <div className="sideButtons">
              {sideOptions.map((item) => {
                const Icon = item.icon;
                return (
                  <button
                    key={item.id}
                    className={side === item.id ? 'active' : ''}
                    onClick={() => setSide(item.id)}
                    disabled={busy}
                    title={item.label}
                    type="button"
                  >
                    <Icon className={`tagIcon ${item.tone}`} size={20} />
                  </button>
                );
              })}
            </div>
          </div>

          <div className="tagDialogGroup">
            <span>Дополнительно</span>
            <div className="flagButtons">
              <button
                className={library ? 'active' : ''}
                onClick={() => setLibrary((value) => !value)}
                disabled={busy}
                title="Библиотека"
                type="button"
              >
                <BookOpen className="tagIcon library" size={18} />
              </button>
              <button
                className={technical ? 'active' : ''}
                onClick={() => setTechnical((value) => !value)}
                disabled={busy}
                title="Оптимизация"
                type="button"
              >
                <Wrench className="tagIcon technical" size={18} />
              </button>
            </div>
          </div>

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
