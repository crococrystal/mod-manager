import { sideOptions } from '../../../lib/modMeta.jsx';

export function ServerSyncPreviewModSideTag({ side }) {
  if (!side) return null;

  const sideMeta = sideOptions.find((item) => item.id === side);
  if (!sideMeta) return null;

  const SideIcon = sideMeta.icon;

  return (
    <span className="serverSyncPreviewFileTagMark" title={sideMeta.label}>
      <SideIcon className={`tagIcon ${sideMeta.tone}`} size={14} strokeWidth={2.2} />
    </span>
  );
}
