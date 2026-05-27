import { useEffect, useMemo, useRef, useState } from 'react';
import { LoaderCircle, RefreshCw, RotateCcw } from 'lucide-react';
import { mergeDependencyKeys } from '../../lib/usedBy.js';
import { ModRelations } from '../relations/ModRelations.jsx';
import { ModCover } from './ModCover.jsx';

export function ModEditor({
  mod,
  mods,
  busy,
  onPatch,
  onUploadCover,
  onDeleteCover,
  onRefreshAssets,
  assetsRefreshing = false,
  onSelectMod,
  onOpenRelations,
  onCloseRelations,
  relationsOpenKey
}) {
  const coverInputRef = useRef(null);
  const [dependencies, setDependencies] = useState(mod.dependencies ?? []);
  const modRef = useRef(mod);
  const freshMod = useMemo(() => mods.find((item) => item.key === mod.key) ?? mod, [mods, mod.key]);
  const resolvedDependencies = useMemo(
    () => freshMod.resolvedDependencies ?? mergeDependencyKeys(dependencies, freshMod.jarDependencies),
    [freshMod, dependencies]
  );
  const usedBy = useMemo(() => freshMod.usedBy ?? mod.usedBy ?? [], [freshMod.usedBy, mod.usedBy]);

  modRef.current = mod;

  useEffect(() => {
    setDependencies(mod.dependencies ?? []);
  }, [mod.key, mod.dependencies]);

  function handleDependenciesChange(next) {
    setDependencies(next);
    onPatch(modRef.current.key, { dependencies: next });
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
        <div className="editorCoverFrame">
          <ModCover
            key={mod.key}
            mod={mod}
            size="hero"
            title="Нажми, чтобы загрузить обложку"
            onClick={() => !busy && !assetsRefreshing && coverInputRef.current?.click()}
          />
          {assetsRefreshing ? (
            <div className="assetLoadingOverlay" aria-hidden="true">
              <LoaderCircle size={32} className="spin" />
            </div>
          ) : null}
          <div className="coverActionStack">
            {onRefreshAssets ? (
              <button
                type="button"
                className="coverActionIcon"
                onClick={(event) => {
                  event.stopPropagation();
                  onRefreshAssets(mod.key);
                }}
                disabled={busy || assetsRefreshing}
                title="Обновить обложку и зависимости"
                aria-label="Обновить обложку и зависимости"
              >
                <RefreshCw size={14} className={assetsRefreshing ? 'spin' : ''} />
              </button>
            ) : null}
            {mod.coverManual && onDeleteCover ? (
              <button
                type="button"
                className="coverActionIcon"
                onClick={(event) => {
                  event.stopPropagation();
                  onDeleteCover(mod.key);
                }}
                disabled={busy || assetsRefreshing}
                title="Сбросить кастомную обложку"
                aria-label="Сбросить кастомную обложку"
              >
                <RotateCcw size={14} />
              </button>
            ) : null}
          </div>
        </div>
        <h2>{mod.displayName}</h2>
        <input ref={coverInputRef} type="file" accept="image/png,image/jpeg,image/webp,image/gif" onChange={uploadCover} hidden />
      </div>

      <ModRelations
        currentMod={mod}
        resolvedDependencies={resolvedDependencies}
        manualDependencies={dependencies}
        jarDependencies={freshMod.jarDependencies ?? []}
        usedBy={usedBy}
        mods={mods}
        busy={busy}
        assetsRefreshing={assetsRefreshing}
        onChange={handleDependenciesChange}
        onSelectMod={onSelectMod}
        onOpenRelations={onOpenRelations}
        onCloseRelations={onCloseRelations}
        relationsOpenKey={relationsOpenKey}
      />
    </div>
  );
}
