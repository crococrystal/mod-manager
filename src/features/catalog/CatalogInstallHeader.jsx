import { RefreshCw } from 'lucide-react';
import { openExternalUrl } from '../../lib/openExternalUrl.js';
import { sourceIcons } from '../../lib/modMeta.jsx';
import { ModCover } from '../mods/ModCover.jsx';

export function CatalogInstallHeader({
  source,
  candidate,
  preview,
  targetParts,
  pageUrl,
  uiLocked,
  refreshing,
  onRefresh
}) {
  const icon = sourceIcons[source]?.icon;

  return (
    <div className="dependencyModalHead catalogInstallHead">
      <ModCover
        mod={{
          coverUrl: preview?.iconUrl ?? candidate.iconUrl,
          displayName: preview?.title ?? candidate.title
        }}
        size="tile"
      />
      <div className="dependencyModalHeadText">
        <p className="dependencyModalSubtitle">
          {[sourceIcons[source]?.label, ...targetParts].filter(Boolean).join(' · ')}
        </p>
        <h3 className="dependencyModalTitle">{preview?.title ?? candidate.title}</h3>
      </div>
      <div className="catalogInstallHeadActions">
        {onRefresh ? (
          <button
            type="button"
            className="coverActionIcon catalogInstallHeadAction"
            onClick={onRefresh}
            disabled={uiLocked}
            title="Обновить описание и зависимости"
            aria-label="Обновить описание и зависимости"
          >
            <RefreshCw size={20} className={refreshing ? 'spin' : ''} />
          </button>
        ) : null}
        {icon ? (
          <button
            type="button"
            className="coverActionIcon catalogInstallHeadAction"
            onClick={() => void openExternalUrl(pageUrl)}
            disabled={uiLocked || !pageUrl}
            title="Открыть у поставщика"
            aria-label="Открыть у поставщика"
          >
            <img src={icon} alt="" className="catalogInstallProviderIcon" />
          </button>
        ) : null}
      </div>
    </div>
  );
}
