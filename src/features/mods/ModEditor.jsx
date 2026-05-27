import { useEffect, useMemo, useRef, useState } from 'react';
import { LoaderCircle, RefreshCw, RotateCcw } from 'lucide-react';
import { mergeDependencyKeys } from '../../lib/usedBy.js';
import { ModRelations } from '../relations/ModRelations.jsx';
import { ModCover } from './ModCover.jsx';

const COVER_MIME_TYPES = new Set(['image/png', 'image/jpeg', 'image/webp', 'image/gif']);

function isCoverFile(file) {
  return Boolean(file && COVER_MIME_TYPES.has(file.type));
}

export function ModEditor({
  mod,
  mods,
  busy,
  onPatch,
  onUploadCover,
  onDeleteCover,
  onRefreshAssets,
  assetsRefreshing = false,
  coverSaving = false,
  onSelectMod,
  onOpenRelations,
  onCloseRelations,
  relationsOpenKey,
  relationsModNav
}) {
  const coverInputRef = useRef(null);
  const coverDragDepthRef = useRef(0);
  const [coverPending, setCoverPending] = useState(false);
  const [coverDragOver, setCoverDragOver] = useState(false);
  const [dependencies, setDependencies] = useState(mod.dependencies ?? []);
  const modRef = useRef(mod);
  const freshMod = useMemo(() => mods.find((item) => item.key === mod.key) ?? mod, [mods, mod.key]);
  const resolvedDependencies = useMemo(
    () => freshMod.resolvedDependencies ?? mergeDependencyKeys(dependencies, freshMod.jarDependencies),
    [freshMod, dependencies]
  );
  const usedBy = useMemo(() => freshMod.usedBy ?? mod.usedBy ?? [], [freshMod.usedBy, mod.usedBy]);
  const coverBusy = coverPending || coverSaving || assetsRefreshing;

  modRef.current = mod;

  useEffect(() => {
    setDependencies(mod.dependencies ?? []);
  }, [mod.key, mod.dependencies]);

  useEffect(() => {
    coverDragDepthRef.current = 0;
    setCoverDragOver(false);
  }, [mod.key]);

  function handleDependenciesChange(next) {
    setDependencies(next);
    onPatch(modRef.current.key, { dependencies: next });
  }

  function uploadCoverFile(file) {
    if (!file || !onUploadCover || !isCoverFile(file) || busy || coverBusy) return;

    setCoverPending(true);
    const reader = new FileReader();
    reader.addEventListener('load', () => {
      if (typeof reader.result !== 'string') {
        setCoverPending(false);
        return;
      }
      void Promise.resolve(onUploadCover(modRef.current.key, reader.result)).finally(() => {
        setCoverPending(false);
      });
    });
    reader.addEventListener('error', () => {
      setCoverPending(false);
    });
    reader.readAsDataURL(file);
  }

  function uploadCover(event) {
    const file = event.target.files?.[0];
    event.target.value = '';
    uploadCoverFile(file);
  }

  function resetCoverDragState() {
    coverDragDepthRef.current = 0;
    setCoverDragOver(false);
  }

  function handleCoverDragEnter(event) {
    if (busy || coverBusy) return;
    event.preventDefault();
    event.stopPropagation();
    coverDragDepthRef.current += 1;
    if ([...event.dataTransfer.types].includes('Files')) {
      setCoverDragOver(true);
    }
  }

  function handleCoverDragOver(event) {
    if (busy || coverBusy) return;
    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = 'copy';
  }

  function handleCoverDragLeave(event) {
    event.preventDefault();
    event.stopPropagation();
    coverDragDepthRef.current = Math.max(0, coverDragDepthRef.current - 1);
    if (coverDragDepthRef.current === 0) {
      setCoverDragOver(false);
    }
  }

  function handleCoverDrop(event) {
    event.preventDefault();
    event.stopPropagation();
    resetCoverDragState();
    if (busy || coverBusy) return;
    uploadCoverFile(event.dataTransfer.files?.[0]);
  }

  return (
    <div className="editor scrollArea">
      <div className="editorHero">
        <div
          className={`editorCoverFrame editorCoverDropArea${coverDragOver ? ' editorCoverDropArea--active' : ''}`}
          onDragEnter={handleCoverDragEnter}
          onDragOver={handleCoverDragOver}
          onDragLeave={handleCoverDragLeave}
          onDrop={handleCoverDrop}
        >
          <ModCover
            key={mod.key}
            mod={mod}
            size="hero"
            title="Нажми или перетащи обложку"
            onClick={() => !busy && !coverBusy && coverInputRef.current?.click()}
          />
          {coverDragOver && !coverBusy ? (
            <div className="coverDropOverlay">Перетащи обложку</div>
          ) : null}
          {coverBusy ? (
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
                disabled={busy || coverBusy}
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
                disabled={busy || coverBusy}
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
        modNav={relationsModNav}
      />
    </div>
  );
}
