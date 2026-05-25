import { useLayoutEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { BookOpen, Wrench } from 'lucide-react';
import { sideOptions, modModalSubtitle } from '../../lib/modMeta.jsx';
import { ModModalHead } from './ModModalHead.jsx';

const POPOVER_WIDTH = 300;
const POPOVER_HEIGHT = 250;
const VIEWPORT_PAD = 12;

function currentTags(mod) {
  return {
    side: mod.side === 'unknown' ? 'universal' : mod.side ?? 'universal',
    library: Boolean(mod.library),
    technical: Boolean(mod.technical)
  };
}

function popoverPosition(anchor) {
  if (!anchor) {
    return { top: VIEWPORT_PAD, left: VIEWPORT_PAD };
  }
  const gap = 6;
  let top = anchor.bottom + gap;
  let left = anchor.left;
  if (left + POPOVER_WIDTH > window.innerWidth - VIEWPORT_PAD) {
    left = window.innerWidth - POPOVER_WIDTH - VIEWPORT_PAD;
  }
  if (left < VIEWPORT_PAD) left = VIEWPORT_PAD;
  if (top + POPOVER_HEIGHT > window.innerHeight - VIEWPORT_PAD) {
    top = anchor.top - POPOVER_HEIGHT - gap;
  }
  if (top < VIEWPORT_PAD) top = VIEWPORT_PAD;
  return { top, left };
}

export function TagsDialog({ mod, anchor, savingKey, onClose, onSave }) {
  const [position, setPosition] = useState(() => popoverPosition(anchor));

  useLayoutEffect(() => {
    setPosition(popoverPosition(anchor));
  }, [anchor, mod?.key]);

  if (!mod) return null;

  const tags = currentTags(mod);
  const saving = savingKey === mod.key;

  async function apply(patch) {
    if (saving) return;
    const next = { ...tags, ...patch };
    if (
      next.side === tags.side &&
      next.library === tags.library &&
      next.technical === tags.technical
    ) {
      return;
    }
    await onSave(mod.key, next);
  }

  return createPortal(
    <div className="tagsPopoverBackdrop" onMouseDown={() => !saving && onClose()}>
      <div
        className="tagsPopover"
        style={{ top: position.top, left: position.left }}
        onMouseDown={(event) => event.stopPropagation()}
        role="dialog"
        aria-label="Метки мода"
      >
        <ModModalHead mod={mod} subtitle={modModalSubtitle(mod, { section: 'Метки' })} />

        <div className="tagDialogGroup">
          <span>Сторона</span>
          <div className="sideButtons">
            {sideOptions.map((item) => {
              const Icon = item.icon;
              return (
                <button
                  key={item.id}
                  className={tags.side === item.id ? 'active' : ''}
                  onClick={() => apply({ side: item.id })}
                  disabled={saving}
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
              className={tags.library ? 'active' : ''}
              onClick={() => apply({ library: !tags.library })}
              disabled={saving}
              title="Библиотека"
              type="button"
            >
              <BookOpen className="tagIcon library" size={18} />
            </button>
            <button
              className={tags.technical ? 'active' : ''}
              onClick={() => apply({ technical: !tags.technical })}
              disabled={saving}
              title="Оптимизация"
              type="button"
            >
              <Wrench className="tagIcon technical" size={18} />
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body
  );
}
