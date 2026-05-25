import { useEffect, useMemo, useRef, useState } from 'react';
import { BookOpen, Wrench } from 'lucide-react';
import { sideOptions } from '../../lib/modMeta.jsx';
import { mergeDependencyKeys } from '../../lib/usedBy.js';
import { ModRelations } from '../relations/ModRelations.jsx';
import { ModCover } from './ModCover.jsx';
import { ModPageLink } from './ModBadges.jsx';

export function ModEditor({ mod, mods, busy, onPatch, onUploadCover, onSelectMod }) {
  const coverInputRef = useRef(null);
  const currentSide = mod.side === 'unknown' ? 'universal' : mod.side;
  const [description, setDescription] = useState(mod.description ?? '');
  const [dependencies, setDependencies] = useState(mod.dependencies ?? []);
  const modRef = useRef(mod);
  const notesRef = useRef({ description, dependencies });
  const freshMod = useMemo(() => mods.find((item) => item.key === mod.key) ?? mod, [mods, mod.key]);
  const resolvedDependencies = useMemo(
    () => freshMod.resolvedDependencies ?? mergeDependencyKeys(dependencies, freshMod.jarDependencies),
    [freshMod, dependencies]
  );
  const usedBy = useMemo(() => freshMod.usedBy ?? mod.usedBy ?? [], [freshMod.usedBy, mod.usedBy]);

  modRef.current = mod;
  notesRef.current = { description, dependencies };

  useEffect(() => {
    setDescription(mod.description ?? '');
    setDependencies(mod.dependencies ?? []);
  }, [mod.key, mod.description, mod.dependencies]);

  useEffect(() => {
    const saved = mod.description ?? '';
    if (description === saved) return undefined;

    const timer = window.setTimeout(() => {
      onPatch(modRef.current.key, notesRef.current);
    }, 450);

    return () => window.clearTimeout(timer);
  }, [description, mod.key, mod.description, onPatch]);

  function handleDependenciesChange(next) {
    setDependencies(next);
    notesRef.current = { description, dependencies: next };
    onPatch(modRef.current.key, notesRef.current);
  }

  function applyWithNotes(patch) {
    onPatch(modRef.current.key, { description, dependencies, ...patch });
  }

  function uploadCover(event) {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (!file || !onUploadCover) return;

    const reader = new FileReader();
    reader.addEventListener('load', () => {
      if (typeof reader.result === 'string') {
        onUploadCover(modRef.current.key, reader.result);
      }
    });
    reader.readAsDataURL(file);
  }

  return (
    <div className="editor scrollArea">
      <div className="editorHero">
        <ModCover
          key={mod.key}
          mod={mod}
          size="hero"
          title="Нажми, чтобы загрузить обложку"
          onClick={() => !busy && coverInputRef.current?.click()}
        />
        <h2>{mod.displayName}</h2>
        <input ref={coverInputRef} type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={uploadCover} hidden />
      </div>

      <ModPageLink mod={mod} />

      <div className="controlGroup">
        <label>Сторона</label>
        <div className="sideButtons">
          {sideOptions.map((side) => {
            const Icon = side.icon;
            return (
              <button
                key={side.id}
                className={currentSide === side.id ? 'active' : ''}
                onClick={() => applyWithNotes({ side: side.id })}
                disabled={busy}
                title={side.label}
                type="button"
              >
                <Icon className={`tagIcon ${side.tone}`} size={20} />
              </button>
            );
          })}
        </div>
      </div>

      <div className="controlGroup">
        <label>Дополнительно</label>
        <div className="flagButtons">
          <button className={mod.library ? 'active' : ''} onClick={() => applyWithNotes({ library: !mod.library })} disabled={busy} type="button">
            <BookOpen className="tagIcon library" size={18} />
          </button>
          <button className={mod.technical ? 'active' : ''} onClick={() => applyWithNotes({ technical: !mod.technical })} disabled={busy} type="button">
            <Wrench className="tagIcon technical" size={18} />
          </button>
        </div>
      </div>

      <div className="controlGroup">
        <label>Описание</label>
        <textarea
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          placeholder="Что это за мод, зачем он в сборке"
        />
      </div>

      <ModRelations
        currentMod={mod}
        resolvedDependencies={resolvedDependencies}
        manualDependencies={dependencies}
        jarDependencies={freshMod.jarDependencies ?? []}
        usedBy={usedBy}
        mods={mods}
        busy={busy}
        onChange={handleDependenciesChange}
        onSelectMod={onSelectMod}
      />
    </div>
  );
}
