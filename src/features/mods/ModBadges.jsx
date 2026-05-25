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

export function SourceIcon({ mod }) {
  const source = sourceIcons[mod.source] ?? sourceIcons.manual;
  const needsAttention = mod.duplicate || !mod.hasTags;

  return (
    <span className="sourceIcon" title={source.label} aria-label={source.label}>
      <img src={source.icon} alt="" />
      {needsAttention ? <AlertTriangle size={13} /> : null}
    </span>
  );
}

export function ModPageLink({ mod }) {
  const source = sourceIcons[mod.source] ?? sourceIcons.manual;

  if (!mod.sourceUrl) {
    return (
      <div className="modPageLink muted">
        <img src={source.icon} alt="" />
        <span>Сторонний мод</span>
      </div>
    );
  }

  return (
    <a className="modPageLink" href={mod.sourceUrl} target="_blank" rel="noreferrer">
      <img src={source.icon} alt="" />
      <span>Открыть страницу мода</span>
    </a>
  );
}
