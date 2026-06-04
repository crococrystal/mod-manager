import { Check, LoaderCircle } from 'lucide-react';
import { ModCover } from '../mods/ModCover.jsx';
import { sourceIcons } from '../../lib/modMeta.jsx';

function formatInstanceTarget(target) {
  if (!target) return '';
  const parts = [target.minecraftVersion, target.loader].filter(Boolean);
  return parts.join(' · ');
}

export function CatalogSearchPanel({
  source,
  target,
  results,
  loading,
  error,
  query,
  installedProjectIds,
  onSelect
}) {
  const providerLabel = sourceIcons[source]?.label ?? 'Каталог';
  const targetLabel = formatInstanceTarget(target);
  const isPopular = !query.trim();
  const showList = results.length > 0;

  if (loading && !showList) {
    return (
      <div className="catalogSearchState">
        <LoaderCircle className="spin" size={28} />
        <span>{isPopular ? `Загрузка модов ${providerLabel}…` : `Поиск на ${providerLabel}…`}</span>
      </div>
    );
  }

  if (error && !showList) {
    return <p className="catalogSearchError">{error}</p>;
  }

  if (!showList && !loading) {
    return (
      <p className="catalogSearchState">
        {isPopular ? 'Не удалось загрузить список модов.' : `Ничего не найдено${targetLabel ? ` для ${targetLabel}` : ''}.`}
      </p>
    );
  }

  return (
    <>
      <p className="catalogSearchTargetBar">
        {isPopular ? 'Популярные' : 'Результаты'}
        {targetLabel ? ` · ${targetLabel}` : ''}
      </p>
      <ul className="catalogSearchList">
        {results.map((item) => {
          const installed = installedProjectIds?.has(String(item.id));
          return (
            <li key={item.id}>
              <button type="button" className="catalogSearchRow" onClick={() => onSelect(item)}>
                <ModCover mod={{ coverUrl: item.iconUrl, displayName: item.title }} size="tile" />
                <span className="catalogSearchText">
                  <span className="catalogSearchTitleLine">
                    <strong>{item.title}</strong>
                    {installed ? <Check className="catalogSearchInstalledIcon" size={14} aria-label="Установлен" /> : null}
                  </span>
                  {item.summary ? <small>{item.summary}</small> : null}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      {loading ? (
        <div className="catalogSearchLoadingMore">
          <LoaderCircle className="spin" size={16} />
        </div>
      ) : null}
      {error ? <p className="catalogSearchError">{error}</p> : null}
    </>
  );
}
