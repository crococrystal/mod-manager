import { useMemo, useState } from 'react';
import { Plus, Trash2 } from 'lucide-react';
import { Button, IconButton } from '../../components/Button.jsx';
import { Modal } from '../../components/Modal.jsx';
import { TagMark } from '../mods/ModBadges.jsx';

export function ModRelations({ mod, mods, busy, onChange, onSelectMod }) {
  const [addOpen, setAddOpen] = useState(false);
  const [query, setQuery] = useState('');
  const byKey = useMemo(() => new Map(mods.map((item) => [item.key, item])), [mods]);
  const dependencies = mod.resolvedDependencies ?? mod.dependencies ?? [];
  const dependencyItems = dependencies.map((key) => byKey.get(key) ?? { key, displayName: key, missing: true });
  const usedByItems = (mod.usedBy ?? []).map((key) => byKey.get(key)).filter(Boolean);
  const needle = query.trim().toLowerCase();
  const options = mods
    .filter((item) => item.key !== mod.key && !dependencies.includes(item.key))
    .filter((item) => !needle || `${item.displayName} ${item.filename}`.toLowerCase().includes(needle))
    .sort((a, b) => a.displayName.localeCompare(b.displayName));

  function addDependency(key) {
    onChange([...new Set([...(mod.dependencies ?? []), key])]);
    setAddOpen(false);
    setQuery('');
  }

  function removeDependency(key) {
    onChange((mod.dependencies ?? []).filter((item) => item !== key));
  }

  return (
    <div className="relations">
      <section className="relationBlock">
        <header>
          <span>Зависимости</span>
          <Button icon={Plus} onClick={() => setAddOpen(true)} disabled={busy}>Добавить</Button>
        </header>
        {dependencyItems.length ? (
          <div className="relationList">
            {dependencyItems.map((item) => (
              <RelationRow
                key={item.key}
                mod={item}
                removable={!item.missing && (mod.dependencies ?? []).includes(item.key)}
                onSelect={() => !item.missing && onSelectMod(item)}
                onRemove={() => removeDependency(item.key)}
                busy={busy}
              />
            ))}
          </div>
        ) : (
          <p className="mutedText">Не указаны.</p>
        )}
      </section>

      <section className="relationBlock">
        <header>
          <span>Используется для</span>
          <strong>{usedByItems.length}</strong>
        </header>
        {usedByItems.length ? (
          <div className="relationList">
            {usedByItems.map((item) => (
              <RelationRow key={item.key} mod={item} onSelect={() => onSelectMod(item)} />
            ))}
          </div>
        ) : (
          <p className="mutedText">Пока ни один мод не ссылается на этот.</p>
        )}
      </section>

      {addOpen ? (
        <Modal title="Добавить зависимость" subtitle={mod.displayName} onClose={() => setAddOpen(false)}>
          <input
            className="searchInput"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Поиск мода"
            autoFocus
          />
          <div className="pickerList">
            {options.slice(0, 120).map((item) => (
              <button key={item.key} type="button" onClick={() => addDependency(item.key)}>
                <TagMark mod={item} />
                <span>{item.displayName}</span>
              </button>
            ))}
            {!options.length ? <p className="mutedText">Ничего не найдено.</p> : null}
          </div>
        </Modal>
      ) : null}
    </div>
  );
}

function RelationRow({ mod, removable = false, busy = false, onSelect, onRemove }) {
  return (
    <div className={`relationRow ${mod.missing ? 'missing' : ''}`}>
      {mod.missing ? <span className="missingMark">?</span> : <TagMark mod={mod} />}
      <button type="button" onClick={onSelect} disabled={mod.missing}>
        {mod.displayName}
      </button>
      {removable ? <IconButton icon={Trash2} label="Убрать" onClick={onRemove} disabled={busy} /> : null}
    </div>
  );
}
