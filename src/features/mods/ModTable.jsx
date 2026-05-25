import { useEffect, useRef, useState } from 'react';
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

function isCommandHeld(event) {
  return event.metaKey || event.ctrlKey;
}

function applyCmdDragMode(mod, mode, onSelectDrag) {
  onSelectDrag(mod, mode === 'select');
}

export function ModTable({
  mods,
  selected,
  selectedKeys,
  sort,
  onSort,
  onSelect,
  onSelectDrag,
  onCoverClick,
  onSourceClick,
  onVersionClick,
  onTagsClick,
  onDescriptionClick
}) {
  const wrapRef = useRef(null);
  const [cmdSelecting, setCmdSelecting] = useState(false);
  const cmdDragRef = useRef({
    active: false,
    dragged: false,
    mode: 'select',
    visited: new Set(),
    startMod: null
  });

  useEffect(() => {
    if (!selected?.filename || !wrapRef.current) return;
    const row = wrapRef.current.querySelector(`tr[data-filename="${CSS.escape(selected.filename)}"]`);
    row?.scrollIntoView({ block: 'nearest' });
  }, [selected?.filename]);

  useEffect(() => {
    function handleMouseUp() {
      cmdDragRef.current.active = false;
      setCmdSelecting(false);
    }

    window.addEventListener('mouseup', handleMouseUp);
    return () => window.removeEventListener('mouseup', handleMouseUp);
  }, []);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap || !cmdSelecting) return undefined;

    function handleWheel(event) {
      event.preventDefault();
    }

    wrap.addEventListener('wheel', handleWheel, { passive: false });
    return () => wrap.removeEventListener('wheel', handleWheel);
  }, [cmdSelecting]);

  function handleRowMouseDown(mod, event) {
    if (event.button !== 0 || !isCommandHeld(event)) return;

    setCmdSelecting(true);
    cmdDragRef.current = {
      active: true,
      dragged: false,
      mode: selectedKeys?.has(mod.key) ? 'deselect' : 'select',
      visited: new Set(),
      startMod: mod
    };
  }

  function handleRowMouseEnter(mod, event) {
    const state = cmdDragRef.current;
    if (!state.active || !(event.buttons & 1) || !isCommandHeld(event) || !onSelectDrag) return;

    if (!state.dragged) {
      state.dragged = true;
      if (state.startMod && !state.visited.has(state.startMod.key)) {
        state.visited.add(state.startMod.key);
        applyCmdDragMode(state.startMod, state.mode, onSelectDrag);
      }
    }

    if (state.visited.has(mod.key)) return;
    state.visited.add(mod.key);
    applyCmdDragMode(mod, state.mode, onSelectDrag);
  }

  function handleRowClick(mod, event) {
    if (cmdDragRef.current.dragged) {
      cmdDragRef.current.dragged = false;
      return;
    }
    onSelect(mod, event);
  }

  return (
    <div
      ref={wrapRef}
      className={`tableWrap scrollArea${cmdSelecting ? ' tableWrapCmdSelecting' : ''}`}
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
              >
                <td>
                  <button
                    type="button"
                    className="tagMarkButton"
                    onClick={(event) => {
                      event.stopPropagation();
                    onTagsClick?.(mod, event.currentTarget.getBoundingClientRect());
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
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
