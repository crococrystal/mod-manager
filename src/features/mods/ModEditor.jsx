import { useEffect, useMemo, useState } from 'react';
import { BookOpen, Wrench } from 'lucide-react';
import { sideOptions } from '../../lib/modMeta.jsx';
import { ModPageLink } from './ModBadges.jsx';
import { ModRelations } from '../relations/ModRelations.jsx';

export function ModEditor({ mod, mods, busy, onPatch, onSelectMod }) {
  const [description, setDescription] = useState(mod.description ?? '');
  const currentSide = mod.side === 'unknown' ? 'universal' : mod.side;
  const currentMod = useMemo(() => mods.find((item) => item.key === mod.key) ?? mod, [mods, mod]);

  useEffect(() => {
    setDescription(mod.description ?? '');
  }, [mod.key, mod.description]);

  useEffect(() => {
    const saved = mod.description ?? '';
    if (description === saved) return undefined;
    const timer = window.setTimeout(() => {
      onPatch(mod.key, { description });
    }, 420);
    return () => window.clearTimeout(timer);
  }, [description, mod.key, mod.description, onPatch]);

  function patch(next) {
    onPatch(mod.key, next);
  }

  return (
    <aside className="editor scrollArea">
      <header className="editorHead">
        <h2>{mod.displayName}</h2>
        <p>{mod.filename}</p>
      </header>

      <ModPageLink mod={mod} />

      <section className="controlGroup">
        <label>Сторона</label>
        <div className="iconGrid three">
          {sideOptions.map((side) => {
            const Icon = side.icon;
            return (
              <button
                key={side.id}
                className={currentSide === side.id ? 'active' : ''}
                onClick={() => patch({ side: side.id })}
                disabled={busy}
                title={side.label}
                type="button"
              >
                <Icon className={`tagIcon ${side.tone}`} size={21} />
              </button>
            );
          })}
        </div>
      </section>

      <section className="controlGroup">
        <label>Свойства</label>
        <div className="iconGrid two">
          <button
            className={mod.library ? 'active' : ''}
            onClick={() => patch({ library: !mod.library })}
            disabled={busy}
            title="Библиотека"
            type="button"
          >
            <BookOpen className="tagIcon library" size={20} />
          </button>
          <button
            className={mod.technical ? 'active' : ''}
            onClick={() => patch({ technical: !mod.technical })}
            disabled={busy}
            title="Оптимизация"
            type="button"
          >
            <Wrench className="tagIcon technical" size={20} />
          </button>
        </div>
      </section>

      <section className="controlGroup">
        <label>Описание</label>
        <textarea
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          placeholder="Что это за мод и зачем он в сборке"
        />
      </section>

      <ModRelations
        mod={currentMod}
        mods={mods}
        busy={busy}
        onChange={(dependencies) => patch({ dependencies })}
        onSelectMod={onSelectMod}
      />
    </aside>
  );
}
