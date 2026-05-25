import { useEffect, useRef } from 'react';
import { formatDate } from '../../lib/modMeta.jsx';
import { ModCover } from './ModCover.jsx';
import { SourceIcon, TagMark } from './ModBadges.jsx';

export function ModTable({ mods, selected, selectedKeys, onSelect, onCoverClick, onSourceClick }) {
  const wrapRef = useRef(null);

  useEffect(() => {
    if (!selected?.filename || !wrapRef.current) return;
    const row = wrapRef.current.querySelector(`tr[data-filename="${CSS.escape(selected.filename)}"]`);
    row?.scrollIntoView({ block: 'nearest' });
  }, [selected?.filename]);

  return (
    <div ref={wrapRef} className="tableWrap scrollArea">
      <table>
        <thead>
          <tr>
            <th>Метка</th>
            <th aria-hidden="true" />
            <th>Название</th>
            <th>Описание</th>
            <th>Дата</th>
            <th>Источник</th>
          </tr>
        </thead>
        <tbody>
          {mods.map((mod) => {
            const active = selected?.filename === mod.filename || selectedKeys?.has(mod.key);
            return (
              <tr
                key={mod.filename}
                data-filename={mod.filename}
                className={active ? 'selected' : ''}
                onClick={(event) => onSelect(mod, event)}
              >
                <td><TagMark mod={mod} /></td>
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
                <td className="descriptionCell" title={mod.description || undefined}>
                  {mod.description || '—'}
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
