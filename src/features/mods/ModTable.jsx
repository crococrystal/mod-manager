import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react';
import { formatDate } from '../../lib/modMeta.jsx';
import { ModCover } from './ModCover.jsx';
import { SourceIcon, TagMark } from './ModBadges.jsx';

function SortHeader({ id, children, sort, onSort }) {
  const active = sort?.key === id;
  const Icon = active ? (sort.direction === 'asc' ? ArrowUp : ArrowDown) : ArrowUpDown;
  return (
    <button type="button" className={`sortHeader${active ? ' active' : ''}`} onClick={() => onSort(id)}>
      <span>{children}</span>
      <Icon size={12} />
    </button>
  );
}

function canChangeVersion(mod) {
  return Boolean(
    (mod.source === 'modrinth' && mod.modrinthId) ||
      (mod.source === 'curseforge' && mod.curseforgeId)
  );
}

function isInteractiveTableTarget(target) {
  return target instanceof HTMLElement
    && Boolean(target.closest('button, a, input, textarea, select, [role="button"]'));
}

function ModRowCells({
  mod,
  onCoverClick,
  onSourceClick,
  onVersionClick,
  onTagsClick,
  onDescriptionClick
}) {
  return (
    <>
      <td>
        <button
          type="button"
          className="tagMarkButton"
          onClick={(event) => {
            event.stopPropagation();
            onTagsClick?.(mod);
          }}
          title="Метки мода"
        >
          <TagMark mod={mod} />
        </button>
      </td>
      <td className="coverCell" onClick={(event) => event.stopPropagation()}>
        <ModCover
          mod={mod}
          onClick={onCoverClick ? () => onCoverClick(mod) : undefined}
          title={onCoverClick ? 'Связи мода' : undefined}
        />
      </td>
      <td>
        <strong>{mod.displayName}</strong>
      </td>
      <td
        className="descriptionCell"
        title={mod.description || undefined}
      >
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onDescriptionClick?.(mod);
          }}
        >
          {mod.description || '—'}
        </button>
      </td>
      <td className="versionCell">
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onVersionClick?.(mod);
          }}
          disabled={!canChangeVersion(mod)}
          title={canChangeVersion(mod) ? 'Версии мода' : 'Сначала выбери поставщика'}
        >
          {mod.installedVersion || '—'}
        </button>
      </td>
      <td>{formatDate(mod.modifiedAt)}</td>
      <td onClick={(event) => event.stopPropagation()}>
        <SourceIcon mod={mod} onClick={onSourceClick ? () => onSourceClick(mod) : undefined} />
      </td>
    </>
  );
}

function SelectedModDock({
  mod,
  dockRef,
  onSelect,
  onContextMenu,
  onCoverClick,
  onSourceClick,
  onVersionClick,
  onTagsClick,
  onDescriptionClick
}) {
  if (!mod) return null;

  return (
    <div
      ref={dockRef}
      className="selectedModDock"
      aria-hidden="true"
    >
      <table>
        <tbody>
          <tr
            className="selected"
            onClick={(event) => onSelect(mod, event)}
            onContextMenu={(event) => onContextMenu?.(mod, event)}
          >
            <ModRowCells
              mod={mod}
              onCoverClick={onCoverClick}
              onSourceClick={onSourceClick}
              onVersionClick={onVersionClick}
              onTagsClick={onTagsClick}
              onDescriptionClick={onDescriptionClick}
            />
          </tr>
        </tbody>
      </table>
    </div>
  );
}

export function ModTable({
  mods,
  selected,
  selectedKeys,
  sort,
  onSort,
  onSelect,
  onSelectDrag,
  onContextMenu,
  onCoverClick,
  onSourceClick,
  onVersionClick,
  onTagsClick,
  onDescriptionClick
}) {
  const wrapRef = useRef(null);
  const selectedDockRef = useRef(null);
  const [dragSelecting, setDragSelecting] = useState(false);
  const dockFrameRef = useRef(0);
  const dragSelectRef = useRef({
    active: false,
    dragged: false,
    visited: new Set(),
    startMod: null
  });

  useLayoutEffect(() => {
    const wrap = wrapRef.current;
    if (!selected?.filename || !wrap) return;

    const row = wrap.querySelector(`tr[data-filename="${CSS.escape(selected.filename)}"]`);
    if (!row) return;

    const headCell = wrap.querySelector('th');
    const wrapRect = wrap.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    const topLimit = (headCell?.getBoundingClientRect().bottom ?? wrapRect.top + 36) + 1;
    const bottomLimit = wrapRect.bottom;

    if (rowRect.top < topLimit) {
      wrap.scrollTop -= topLimit - rowRect.top;
    } else if (rowRect.bottom > bottomLimit) {
      wrap.scrollTop += rowRect.bottom - bottomLimit;
    }
  }, [selected?.filename]);

  useLayoutEffect(() => {
    const wrap = wrapRef.current;
    const dock = selectedDockRef.current;
    if (!wrap || !dock || !selected?.filename) {
      if (dock) dock.classList.remove('selectedModDockVisible', 'selectedModDockTop', 'selectedModDockBottom');
      return undefined;
    }

    let currentPlacement = null;

    function setDockPlacement(placement) {
      if (currentPlacement === placement) return;
      currentPlacement = placement;
      dock.classList.toggle('selectedModDockVisible', Boolean(placement));
      dock.classList.toggle('selectedModDockTop', placement === 'top');
      dock.classList.toggle('selectedModDockBottom', placement === 'bottom');
    }

    function updateSelectedDock() {
      dockFrameRef.current = 0;

      const row = wrap.querySelector(`tr[data-filename="${CSS.escape(selected.filename)}"]`);
      const headCell = wrap.querySelector('th');
      if (!row) {
        setDockPlacement(null);
        return;
      }

      const wrapRect = wrap.getBoundingClientRect();
      const rowRect = row.getBoundingClientRect();
      const topLimit = headCell?.getBoundingClientRect().bottom ?? wrapRect.top + 36;
      const bottomLimit = wrapRect.bottom;
      let placement = null;

      if (rowRect.top < topLimit) {
        placement = 'top';
      } else if (rowRect.bottom > bottomLimit) {
        placement = 'bottom';
      }

      if (!placement) {
        setDockPlacement(null);
        return;
      }

      const rowHeight = Math.max(1, Math.round(rowRect.height));
      const x = Math.round(wrapRect.left);
      const y = placement === 'top'
        ? Math.round(topLimit - 1)
        : Math.round(bottomLimit - rowHeight);

      dock.style.width = `${Math.round(wrap.clientWidth)}px`;
      dock.style.transform = `translate3d(${x}px, ${y}px, 0)`;
      setDockPlacement(placement);
    }

    function scheduleSelectedDockUpdate() {
      if (dockFrameRef.current) return;
      dockFrameRef.current = window.requestAnimationFrame(updateSelectedDock);
    }

    updateSelectedDock();
    wrap.addEventListener('scroll', scheduleSelectedDockUpdate, { passive: true });
    window.addEventListener('resize', scheduleSelectedDockUpdate);

    const resizeObserver = new ResizeObserver(scheduleSelectedDockUpdate);
    resizeObserver.observe(wrap);

    return () => {
      wrap.removeEventListener('scroll', scheduleSelectedDockUpdate);
      window.removeEventListener('resize', scheduleSelectedDockUpdate);
      resizeObserver.disconnect();
      setDockPlacement(null);
      if (dockFrameRef.current) {
        window.cancelAnimationFrame(dockFrameRef.current);
        dockFrameRef.current = 0;
      }
    };
  }, [mods, selected?.filename]);

  useEffect(() => {
    function handleMouseUp() {
      dragSelectRef.current.active = false;
      setDragSelecting(false);
    }

    window.addEventListener('mouseup', handleMouseUp);
    return () => window.removeEventListener('mouseup', handleMouseUp);
  }, []);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap || !dragSelecting) return undefined;

    function handleWheel(event) {
      event.preventDefault();
    }

    wrap.addEventListener('wheel', handleWheel, { passive: false });
    return () => wrap.removeEventListener('wheel', handleWheel);
  }, [dragSelecting]);

  function handleRowMouseDown(mod, event) {
    if (event.button !== 0 || isInteractiveTableTarget(event.target)) return;

    setDragSelecting(true);
    dragSelectRef.current = {
      active: true,
      dragged: false,
      visited: new Set(),
      startMod: mod
    };
  }

  function handleRowMouseEnter(mod, event) {
    const state = dragSelectRef.current;
    if (!state.active || !(event.buttons & 1) || !onSelectDrag) return;

    if (!state.dragged) {
      state.dragged = true;
      if (state.startMod && !state.visited.has(state.startMod.key)) {
        state.visited.add(state.startMod.key);
        onSelectDrag(state.startMod, true, { reset: true });
      }
    }

    if (state.visited.has(mod.key)) return;
    state.visited.add(mod.key);
    onSelectDrag(mod, true);
  }

  function handleRowClick(mod, event) {
    if (dragSelectRef.current.dragged) {
      dragSelectRef.current.dragged = false;
      return;
    }
    onSelect(mod, event);
  }

  return (
    <>
      <div
        ref={wrapRef}
        className={`tableWrap scrollArea${dragSelecting ? ' tableWrapDragSelecting' : ''}`}
      >
        <table>
          <thead>
            <tr>
              <th><SortHeader id="tag" sort={sort} onSort={onSort}>Метка</SortHeader></th>
              <th aria-hidden="true" />
              <th><SortHeader id="name" sort={sort} onSort={onSort}>Название</SortHeader></th>
              <th><SortHeader id="description" sort={sort} onSort={onSort}>Описание</SortHeader></th>
              <th><SortHeader id="version" sort={sort} onSort={onSort}>Версия</SortHeader></th>
              <th><SortHeader id="date" sort={sort} onSort={onSort}>Дата</SortHeader></th>
              <th><SortHeader id="source" sort={sort} onSort={onSort}>Источник</SortHeader></th>
            </tr>
          </thead>
          <tbody>
            {mods.map((mod) => {
              const active = selectedKeys?.has(mod.key);
              return (
                <tr
                  key={mod.filename}
                  data-filename={mod.filename}
                  className={active ? 'selected' : ''}
                  onMouseDown={(event) => handleRowMouseDown(mod, event)}
                  onMouseEnter={(event) => handleRowMouseEnter(mod, event)}
                  onClick={(event) => handleRowClick(mod, event)}
                  onContextMenu={(event) => onContextMenu?.(mod, event)}
                >
                  <ModRowCells
                    mod={mod}
                    onCoverClick={onCoverClick}
                    onSourceClick={onSourceClick}
                    onVersionClick={onVersionClick}
                    onTagsClick={onTagsClick}
                    onDescriptionClick={onDescriptionClick}
                  />
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <SelectedModDock
        mod={selected}
        dockRef={selectedDockRef}
        onSelect={onSelect}
        onContextMenu={onContextMenu}
        onCoverClick={onCoverClick}
        onSourceClick={onSourceClick}
        onVersionClick={onVersionClick}
        onTagsClick={onTagsClick}
        onDescriptionClick={onDescriptionClick}
      />
    </>
  );
}
