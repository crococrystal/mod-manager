import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { LoaderCircle, Plus, Trash2 } from 'lucide-react';
import { ModCover } from '../mods/ModCover.jsx';
import { ModModalHead } from '../mods/ModModalHead.jsx';
import { modModalSubtitle } from '../../lib/modMeta.jsx';
import { DependencyModalBackdrop } from '../../components/DependencyModalBackdrop.jsx';
import { ModalModNavRail } from '../../components/ModalModNavRail.jsx';
import { getModalPortalRoot } from '../../lib/modalPortal.js';

function relationsViewFor(hasDeps, hasUsed) {
  if (hasDeps && hasUsed) return 'dual';
  if (hasUsed) return 'used';
  if (hasDeps) return 'manage';
  return null;
}

function resolveActiveView(relationsView, lastView, hasDeps, hasUsed) {
  if (relationsView) return relationsView;
  if (lastView === 'dual') {
    if (hasDeps && hasUsed) return 'dual';
    if (hasUsed) return 'used';
    return 'manage';
  }
  if (lastView === 'used') return hasUsed ? 'used' : 'manage';
  return 'manage';
}

export function ModRelations({
  currentMod,
  resolvedDependencies,
  manualDependencies,
  jarDependencies = [],
  usedBy,
  mods,
  busy,
  assetsRefreshing = false,
  onChange,
  onSelectMod,
  onOpenRelations,
  onCloseRelations,
  relationsOpenKey,
  modNav
}) {
  const relationsOpen = relationsOpenKey === currentMod.key;
  const [addOpen, setAddOpen] = useState(false);
  const [query, setQuery] = useState('');
  const modal = addOpen ? 'add' : relationsOpen ? 'relations' : null;
  const setModal = (next) => {
    if (next === 'add') {
      setAddOpen(true);
    } else if (next === 'relations') {
      setAddOpen(false);
      onOpenRelations?.(currentMod);
    } else {
      setAddOpen(false);
      onCloseRelations?.();
    }
  };
  const jarKeys = useMemo(() => new Set(jarDependencies), [jarDependencies]);

  const usedItems = useMemo(
    () =>
      (usedBy ?? [])
        .map((key) => mods.find((candidate) => candidate.key === key))
        .filter(Boolean),
    [usedBy, mods]
  );

  const selectedItems = useMemo(
    () =>
      resolvedDependencies
        .map((key) => mods.find((candidate) => candidate.key === key))
        .filter(Boolean),
    [resolvedDependencies, mods]
  );

  const hasUsed = usedItems.length > 0;
  const hasDeps = selectedItems.length > 0;
  const isOptional = !hasDeps && !hasUsed;
  const showAddList = query.trim().length > 0;
  const relationsView = relationsViewFor(hasDeps, hasUsed);
  const lastViewRef = useRef('manage');

  if (relationsView) lastViewRef.current = relationsView;
  const activeView = resolveActiveView(relationsView, lastViewRef.current, hasDeps, hasUsed);

  function closeModal() {
    setAddOpen(false);
    onCloseRelations?.();
    setQuery('');
  }

  function openRelations() {
    setAddOpen(false);
    onOpenRelations?.(currentMod);
  }

  function addDependency(key) {
    if (!key || resolvedDependencies.includes(key)) return;
    onChange([...manualDependencies, key]);
    setAddOpen(false);
    setQuery('');
  }

  function removeDependency(key) {
    if (jarKeys.has(key)) return;
    onChange(manualDependencies.filter((item) => item !== key));
  }

  function pickMod(event, mod) {
    if (!event || !mod || !onSelectMod) return;
    event.stopPropagation();
    event.preventDefault();
    onSelectMod(mod);
  }

  const dependencyOptions = mods
    .filter((item) => {
      if (item.key === currentMod.key || resolvedDependencies.includes(item.key)) return false;
      const needle = query.trim().toLowerCase();
      if (!needle) return true;
      return `${item.displayName} ${item.filename}`.toLowerCase().includes(needle);
    })
    .sort((a, b) => a.displayName.localeCompare(b.displayName));

  useEffect(() => {
    setAddOpen(false);
    setQuery('');
  }, [currentMod.key]);

  useEffect(() => {
    if (!modal) return undefined;

    function handleEscape(event) {
      if (event.key === 'Escape') closeModal();
    }

    if (currentMod.coverUrl) {
      const cover = new Image();
      cover.src = currentMod.coverUrl;
    }
    for (const item of [...selectedItems, ...usedItems]) {
      if (!item.coverUrl) continue;
      const img = new Image();
      img.src = item.coverUrl;
    }

    document.addEventListener('keydown', handleEscape);
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    return () => {
      document.removeEventListener('keydown', handleEscape);
      document.body.style.overflow = prevOverflow;
    };
  }, [modal, selectedItems, usedItems, currentMod.coverUrl]);

  function renderDepsRows() {
    return selectedItems.map((item) => (
      <div key={item.key} className="dependencyManageRow">
        <ModCover
          mod={item}
          size="tile"
          title="Выбрать мод"
          onClick={(event) => pickMod(event, item)}
        />
        <span>{item.displayName}</span>
        {jarKeys.has(item.key) ? null : (
          <button
            type="button"
            className="dependencyRemove"
            onClick={() => removeDependency(item.key)}
            disabled={busy}
            title="Убрать зависимость"
          >
            <Trash2 size={18} />
          </button>
        )}
      </div>
    ));
  }

  function renderUsedRows() {
    return usedItems.map((item) => (
      <div key={item.key} className="dependencyViewRow">
        <ModCover
          mod={item}
          size="tile"
          title="Выбрать мод"
          onClick={(event) => pickMod(event, item)}
        />
        <span>{item.displayName}</span>
      </div>
    ));
  }

  const relationsModal =
    modal === 'relations' && activeView === 'dual' ? (
      <div className="dependencyModalDual" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead mod={currentMod} subtitle={modModalSubtitle(currentMod, { section: 'Связи' })} />
        <ModalModNavRail modNav={modNav} uiLocked={busy}>
          <div className="dependencyModalDualGrid" role="dialog" aria-modal="true" aria-label="Связи мода">
          <section className="dependencyModalPane">
            <header className="dependencyModalPaneHead">
              <span>Зависимости</span>
              {selectedItems.length > 0 ? (
                <span className="dependencyModalPaneCount">{selectedItems.length}</span>
              ) : null}
            </header>
            <div className="dependencyModalPaneBody scrollArea">{renderDepsRows()}</div>
            <button
              type="button"
              className="dependencyModalPaneAdd"
              onClick={() => setModal('add')}
              disabled={busy}
            >
              <Plus size={16} strokeWidth={2} />
              Добавить
            </button>
          </section>
          <section className="dependencyModalPane dependencyModalPaneMuted">
            <header className="dependencyModalPaneHead">
              <span>Используется для</span>
              {usedItems.length > 0 ? (
                <span className="dependencyModalPaneCount">{usedItems.length}</span>
              ) : null}
            </header>
            <div className="dependencyModalPaneBody scrollArea">{renderUsedRows()}</div>
          </section>
          </div>
        </ModalModNavRail>
      </div>
    ) : modal === 'relations' && activeView === 'manage' ? (
      <div className="dependencyModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead mod={currentMod} subtitle={modModalSubtitle(currentMod, { section: 'Зависимости' })} />
        <ModalModNavRail modNav={modNav} uiLocked={busy}>
          <div
            className="dependencyModal dependencyModalManage dependencyModalWithAdd"
            role="dialog"
            aria-modal="true"
            aria-label="Зависимости мода"
          >
            {isOptional ? (
              <p className="dependencyModalNote">
                Мод не требуется для других модов и не является обязательным
              </p>
            ) : (
              <div className="dependencyOptions scrollArea">{renderDepsRows()}</div>
            )}
            <button
              type="button"
              className="dependencyModalPaneAdd"
              onClick={() => setModal('add')}
              disabled={busy}
            >
              <Plus size={16} strokeWidth={2} />
              Добавить
            </button>
          </div>
        </ModalModNavRail>
      </div>
    ) : modal === 'relations' && activeView === 'used' ? (
      <div className="dependencyModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead
          mod={currentMod}
          subtitle={modModalSubtitle(currentMod, { section: 'Используется для' })}
          titleFirst
        />
        <ModalModNavRail modNav={modNav} uiLocked={busy}>
          <div
            className="dependencyModal dependencyModalManage"
            role="dialog"
            aria-modal="true"
            aria-label="Используется для"
          >
            {hasUsed ? <div className="dependencyOptions scrollArea">{renderUsedRows()}</div> : null}
          </div>
        </ModalModNavRail>
      </div>
    ) : null;

  const modalContent =
    modal === 'add' ? (
      <div className="dependencyModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <ModModalHead mod={currentMod} subtitle={modModalSubtitle(currentMod, { section: 'Зависимости' })} />
        <ModalModNavRail modNav={modNav} uiLocked={busy}>
          <div
            className="dependencyModal dependencyModalAdd"
            role="dialog"
            aria-modal="true"
            aria-label="Добавить зависимость"
          >
            {isOptional && !showAddList ? (
              <p className="dependencyModalNote">
                Мод не требуется для других модов и не является обязательным
              </p>
            ) : null}
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Поиск мода"
            />
            {showAddList ? (
              <div className="dependencyOptions scrollArea">
                {dependencyOptions.length ? (
                  dependencyOptions.slice(0, 80).map((item) => (
                    <div
                      key={item.key}
                      className="dependencyPickRow"
                      role="button"
                      tabIndex={0}
                      onClick={() => addDependency(item.key)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault();
                          addDependency(item.key);
                        }
                      }}
                    >
                      <ModCover
                        mod={item}
                        size="tile"
                        title="Выбрать мод"
                        onClick={(event) => pickMod(event, item)}
                      />
                      <span>{item.displayName}</span>
                    </div>
                  ))
                ) : (
                  <span>Ничего не найдено</span>
                )}
              </div>
            ) : null}
          </div>
        </ModalModNavRail>
      </div>
    ) : (
      relationsModal
    );

  const portal =
    modal && modalContent ? (
      <DependencyModalBackdrop uiLocked={busy} onClose={closeModal}>
        {modalContent}
      </DependencyModalBackdrop>
    ) : null;

  return (
    <>
      <div className="controlGroup">
        <label>Зависимости</label>
        <div className="dependencyTiles">
          {selectedItems.map((item) => (
            <button
              key={item.key}
              type="button"
              className="dependencyTile"
              onClick={openRelations}
              disabled={busy || assetsRefreshing}
              title={item.displayName}
            >
              <ModCover mod={item} size="tile" />
              {assetsRefreshing ? (
                <span className="relationTileSpinner" aria-hidden="true">
                  <LoaderCircle size={18} className="spin" />
                </span>
              ) : null}
            </button>
          ))}
          <button
            type="button"
            className="dependencyTile dependencyTileAdd"
            onClick={() => setModal('relations')}
            disabled={busy}
            title="Связи мода"
          >
            <Plus size={22} strokeWidth={1.75} />
          </button>
        </div>
      </div>

      {hasUsed ? (
        <div className="controlGroup usedByGroup">
          <label>Используется для</label>
          <div className="dependencyTiles">
            {usedItems.map((item) => (
              <button
                key={item.key}
                type="button"
                className="dependencyTile"
                onClick={openRelations}
                disabled={busy || assetsRefreshing}
                title={item.displayName}
              >
                <ModCover mod={item} size="tile" />
                {assetsRefreshing ? (
                  <span className="relationTileSpinner" aria-hidden="true">
                    <LoaderCircle size={18} className="spin" />
                  </span>
                ) : null}
              </button>
            ))}
          </div>
        </div>
      ) : null}

      {portal ? createPortal(portal, getModalPortalRoot()) : null}
    </>
  );
}
