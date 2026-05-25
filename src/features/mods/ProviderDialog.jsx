import { createPortal } from 'react-dom';
import { sourceIcons } from '../../lib/modMeta.jsx';
import { ModCover } from './ModCover.jsx';

const providerOptions = [
  { id: 'modrinth', label: 'Modrinth' },
  { id: 'curseforge', label: 'CurseForge' }
];

export function ProviderDialog({ mod, busy, onClose, onSelect }) {
  if (!mod) return null;

  return createPortal(
    <div className="dependencyModalBackdrop" onMouseDown={onClose}>
      <div className="dependencyModalStack providerModalStack" onMouseDown={(event) => event.stopPropagation()}>
        <div className="dependencyModalHead">
          <ModCover mod={mod} size="tile" />
          <div className="dependencyModalHeadText">
            <p className="dependencyModalSubtitle">{mod.displayName}</p>
            <h3 className="dependencyModalTitle">Поставщик</h3>
          </div>
        </div>
        <div className="dependencyModal providerModal" role="dialog" aria-modal="true" aria-label="Поставщик мода">
          {providerOptions.map((item) => {
            const icon = sourceIcons[item.id]?.icon;
            const active = mod.source === item.id;
            return (
              <button
                key={item.id}
                type="button"
                className={`providerOption${active ? ' active' : ''}`}
                onClick={() => onSelect(item.id)}
                disabled={busy || active}
              >
                {icon ? <img src={icon} alt="" /> : null}
                <span>{item.label}</span>
                {active ? <strong>Выбран</strong> : null}
              </button>
            );
          })}
        </div>
      </div>
    </div>,
    document.body
  );
}
