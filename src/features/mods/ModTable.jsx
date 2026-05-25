import { useEffect, useRef } from 'react';
import { formatDate } from '../../lib/modMeta.jsx';
import { SourceIcon, TagMark } from './ModBadges.jsx';

export function ModTable({ mods, selected, onSelect }) {
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
            <th>Название</th>
            <th>Описание</th>
            <th>Дата</th>
            <th>Источник</th>
          </tr>
        </thead>
        <tbody>
          {mods.map((mod) => (
            <tr
              key={mod.key}
              data-filename={mod.filename}
              className={selected?.key === mod.key ? 'selected' : ''}
              onClick={() => onSelect(mod)}
            >
              <td><TagMark mod={mod} /></td>
              <td><strong>{mod.displayName}</strong></td>
              <td className="descriptionCell" title={mod.description || undefined}>
                {mod.description || '-'}
              </td>
              <td>{formatDate(mod.modifiedAt)}</td>
              <td><SourceIcon mod={mod} /></td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
