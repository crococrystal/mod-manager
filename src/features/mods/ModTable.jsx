import { useCallback, useRef } from 'react';
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react';
import { formatDate } from '../../lib/modMeta.jsx';
import { useSelectedListDock } from '../../hooks/useSelectedListDock.js';
import { useTableDragSelect } from '../../hooks/useTableDragSelect.js';
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
  return !mod.disabled && Boolean(
    (mod.source === 'modrinth' && mod.modrinthId) ||
      (mod.source === 'curseforge' && mod.curseforgeId)
  );
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
      className="selectedModDock selectedListDock"
      aria-hidden="true"
    >
      <table>
        <tbody>
          <tr
            className={['selected', mod.disabled ? 'modRowDisabled' : ''].filter(Boolean).join(' ')}
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

  const { dragSelecting, handleRowMouseDown, handleRowMouseEnter, handleRowClick } = useTableDragSelect({
    wrapRef,
    onSelectDrag
  });

  const rowSelector = useCallback(
    (wrap) => {
      if (!selected?.filename) return null;
      return wrap.querySelector(`tr[data-filename="${CSS.escape(selected.filename)}"]`);
    },
    [selected?.filename]
  );

  const topLimitSelector = useCallback((wrap) => wrap.querySelector('th'), []);

  useSelectedListDock({
    active: Boolean(selected?.filename),
    wrapRef,
    dockRef: selectedDockRef,
    rowSelector,
    topLimitSelector,
    fallbackTopInset: 36,
    scrollIntoViewKey: selected?.filename ?? null,
    deps: [mods, selected?.filename]
  });

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
              const rowClass = [active ? 'selected' : '', mod.disabled ? 'modRowDisabled' : '']
                .filter(Boolean)
                .join(' ');
              return (
                <tr
                  key={mod.filename}
                  data-filename={mod.filename}
                  className={rowClass}
                  onMouseDown={(event) => handleRowMouseDown(mod, event)}
                  onMouseEnter={(event) => handleRowMouseEnter(mod, event)}
                  onClick={(event) => handleRowClick(mod, event, onSelect)}
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
