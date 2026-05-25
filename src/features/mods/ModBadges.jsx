import { AlertTriangle, BookOpen, Wrench } from 'lucide-react';
import { sideOptions, sourceIcons } from '../../lib/modMeta.jsx';

export function TagMark({ mod }) {
  const side = sideOptions.find((item) => item.id === mod.side) ?? sideOptions[1];
  const SideIcon = side.icon;

  return (
    <span className="tagMark" title={side.label}>
      <SideIcon className={`tagIcon ${side.tone}`} size={17} />
      {mod.library ? <BookOpen className="tagIcon library" size={15} /> : null}
      {mod.technical ? <Wrench className="tagIcon technical" size={15} /> : null}
    </span>
  );
}

export function SourceIcon({ mod, linked = false, onClick }) {
  const item = sourceIcons[mod.source] ?? sourceIcons.manual;
  const needsAttention = mod.duplicate || !mod.hasTags;
  const label = needsAttention ? `${item.label}: проверить` : item.label;
  const content = (
    <>
      <img src={item.icon} alt="" />
      {needsAttention ? <AlertTriangle size={13} /> : null}
    </>
  );

  if (onClick) {
    return (
      <button
        type="button"
        className="sourceIcon sourceIconButton"
        onClick={onClick}
        title={`Поставщик: ${label}`}
        aria-label={`Поставщик: ${label}`}
      >
        {content}
      </button>
    );
  }

  if (linked && mod.sourceUrl) {
    return (
      <a
        className="sourceIcon"
        href={mod.sourceUrl}
        target="_blank"
        rel="noreferrer"
        title={`Открыть ${label}`}
        aria-label={`Открыть ${label}`}
      >
        {content}
      </a>
    );
  }

  return (
    <span className="sourceIcon" title={label} aria-label={label}>
      {content}
    </span>
  );
}

export function ModPageLink({ mod }) {
  const item = sourceIcons[mod.source] ?? sourceIcons.manual;

  if (!mod.sourceUrl) {
    return (
      <div className="modPageLink muted">
        <img src={item.icon} alt="" />
        <span>Сторонний мод</span>
      </div>
    );
  }

  return (
    <a className="modPageLink" href={mod.sourceUrl} target="_blank" rel="noreferrer">
      <img src={item.icon} alt="" />
      <span>Открыть страницу</span>
    </a>
  );
}
