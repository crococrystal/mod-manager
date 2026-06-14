import { useCallback, useMemo, useRef } from 'react';
import { Check, LoaderCircle } from 'lucide-react';
import { ModCover } from '../mods/ModCover.jsx';
import { sourceIcons, UpdatesCurrentState } from '../../lib/modMeta.jsx';
import { useSelectedListDock } from '../../hooks/useSelectedListDock.js';
import { useTableDragSelect } from '../../hooks/useTableDragSelect.js';
import { useInfiniteScroll } from '../../hooks/useInfiniteScroll.js';
import { resolveCatalogInstalledIndicator } from './catalogInstalledStatus.js';

function CatalogInstalledBadge({ indicator }) {
  if (!indicator?.show) return null;

  const provider = sourceIcons[indicator.source] ?? sourceIcons.manual;
  const label = provider?.label ?? 'Установлен';

  return (
    <span
      className="catalogSearchInstalledMark"
      title={`Установлен с ${label}`}
      aria-label={`Установлен с ${label}`}
    >
      <span className="catalogSearchInstalledProvider">
        <img src={provider.icon} alt="" />
      </span>
      <Check className="catalogSearchInstalledCheck" size={14} strokeWidth={2.5} aria-hidden />
    </span>
  );
}

function formatInstanceTarget(target) {
  if (!target) return '';
  const parts = [target.minecraftVersion, target.loader].filter(Boolean);
  return parts.join(' · ');
}

function UpdateRowCells({ item, modForItem }) {
  const coverMod = modForItem?.(item) ?? { coverUrl: item.iconUrl, displayName: item.title };

  return (
    <>
      <td className="coverCell" onClick={(event) => event.stopPropagation()}>
        <ModCover mod={coverMod} />
      </td>
      <td>
        <strong>{item.title}</strong>
        {item.summary ? <small>{item.summary}</small> : null}
      </td>
    </>
  );
}

export function CatalogSearchPanel({
  source,
  target,
  results,
  loading,
  loadingMore = false,
  hasMore = false,
  error,
  query,
  installedProjectIds,
  installedMods = [],
  modForItem,
  selectedKey,
  selectedKeys,
  updatesCheckedAtMs = null,
  updatesReady = false,
  updatesLoading = false,
  updatesBlocked = false,
  onSelect,
  onSelectDrag,
  onContextMenu,
  onLoadMore
}) {
  const isUpdates = source === 'updates';
  const providerLabel = isUpdates ? 'Обновления' : (sourceIcons[source]?.label ?? 'Каталог');
  const targetLabel = formatInstanceTarget(target);
  const isPopular = !isUpdates && !query.trim();
  const showList = results.length > 0;
  const showUpdatesLoadingCenter = isUpdates && loading && !showList;
  const listRef = useRef(null);
  const selectedDockRef = useRef(null);

  const { dragSelecting, handleRowMouseDown, handleRowMouseEnter, handleRowClick } = useTableDragSelect({
    enabled: isUpdates,
    wrapRef: listRef,
    onSelectDrag,
    getItemKey: (item) => item?.key ?? item?.id
  });

  const selectedItem = useMemo(() => {
    if (!isUpdates || !selectedKey) return null;
    return results.find((item) => (item.key ?? item.id) === selectedKey) ?? null;
  }, [isUpdates, results, selectedKey]);

  const rowSelector = useCallback(
    (wrap) => {
      if (!selectedKey) return null;
      return wrap.querySelector(`tr[data-update-key="${CSS.escape(selectedKey)}"]`);
    },
    [selectedKey]
  );

  const topLimitSelector = useCallback(
    (wrap) => wrap.parentElement?.querySelector('.catalogSearchTargetBar'),
    []
  );

  useSelectedListDock({
    active: isUpdates && Boolean(selectedKey),
    wrapRef: listRef,
    dockRef: selectedDockRef,
    rowSelector,
    topLimitSelector,
    scrollIntoViewKey: isUpdates ? selectedKey : null,
    deps: [results, selectedKey]
  });

  const infiniteEnabled = !isUpdates && Boolean(onLoadMore);
  const sentinelRef = useInfiniteScroll({
    enabled: infiniteEnabled,
    rootRef: listRef,
    hasMore,
    loading,
    loadingMore,
    onLoadMore,
    watchKey: results.length
  });

  if (isUpdates && updatesBlocked) {
    if (updatesLoading) {
      return (
        <div className="catalogSearchState">
          <LoaderCircle className="spin" size={28} />
          <span>Проверка обновлений модов…</span>
        </div>
      );
    }
    return <div className="catalogSearchPanel" aria-busy="true" />;
  }

  if (showUpdatesLoadingCenter || (loading && !showList && !isUpdates)) {
    return (
      <div className="catalogSearchState">
        <LoaderCircle className="spin" size={28} />
        <span>
          {isUpdates
            ? 'Проверка обновлений модов…'
            : isPopular
            ? `Загрузка модов ${providerLabel}…`
            : `Поиск на ${providerLabel}…`}
        </span>
      </div>
    );
  }

  if (error && !showList) {
    return <p className="catalogSearchError">{error}</p>;
  }

  if (!showList && !loading) {
    if (isUpdates && query.trim()) {
      return (
        <p className="catalogSearchState">
          {`Ничего не найдено${targetLabel ? ` для ${targetLabel}` : ''}.`}
        </p>
      );
    }
    if (isUpdates && updatesReady && updatesCheckedAtMs && !updatesLoading && !updatesBlocked) {
      return <UpdatesCurrentState checkedAtMs={updatesCheckedAtMs} />;
    }
    return (
      <p className="catalogSearchState">
        {isUpdates
          ? `Обновлений не найдено${targetLabel ? ` для ${targetLabel}` : ''}.`
          : isPopular
          ? 'Не удалось загрузить список модов.'
          : `Ничего не найдено${targetLabel ? ` для ${targetLabel}` : ''}.`}
      </p>
    );
  }

  return (
    <>
      <div className="catalogSearchPanel">
        <p className="catalogSearchTargetBar">
          {isUpdates ? 'Доступны обновления' : isPopular ? 'Популярные' : 'Результаты'}
          {targetLabel ? ` · ${targetLabel}` : ''}
        </p>
        {isUpdates ? (
          <div
            ref={listRef}
            className={`tableWrap scrollArea${dragSelecting ? ' tableWrapDragSelecting' : ''}`}
          >
            <table className="updatesModTable">
              <tbody>
                {results.map((item) => {
                  const itemKey = item.key ?? item.id;
                  const selected = selectedKeys?.has(itemKey);

                  return (
                    <tr
                      key={item.id}
                      data-update-key={itemKey}
                      className={selected ? 'selected' : ''}
                      onMouseDown={(event) => handleRowMouseDown(item, event)}
                      onMouseEnter={(event) => handleRowMouseEnter(item, event)}
                      onClick={(event) => handleRowClick(item, event, onSelect)}
                      onContextMenu={(event) => onContextMenu?.(item, event)}
                    >
                      <UpdateRowCells item={item} modForItem={modForItem} />
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        ) : (
          <div ref={listRef} className="tableWrap scrollArea">
            <ul className="catalogSearchList">
              {results.map((item) => {
                const installedIndicator = resolveCatalogInstalledIndicator({
                  catalogSource: source,
                  item,
                  mods: installedMods,
                  installedProjectIds
                });
                const coverMod =
                  modForItem?.(item) ?? { coverUrl: item.iconUrl, displayName: item.title };

                return (
                  <li key={item.id}>
                    <button type="button" className="catalogSearchRow" onClick={() => onSelect(item)}>
                      <ModCover mod={coverMod} size="tile" />
                      <span className="catalogSearchText">
                        <span className="catalogSearchTitleLine">
                          <strong>{item.title}</strong>
                          <CatalogInstalledBadge indicator={installedIndicator} />
                        </span>
                        {item.summary ? <small>{item.summary}</small> : null}
                      </span>
                    </button>
                  </li>
                );
              })}
              {loadingMore ? (
                <li className="catalogSearchLoadingMore" aria-busy="true">
                  <LoaderCircle className="spin" size={18} />
                </li>
              ) : null}
              {hasMore ? <li ref={sentinelRef} className="catalogSearchSentinel" aria-hidden="true" /> : null}
            </ul>
          </div>
        )}
      </div>
      {isUpdates && selectedItem ? (
        <div ref={selectedDockRef} className="selectedListDock updatesSelectedDock" aria-hidden="true">
          <table className="updatesModTable">
            <tbody>
              <tr className="selected">
                <UpdateRowCells item={selectedItem} modForItem={modForItem} />
              </tr>
            </tbody>
          </table>
        </div>
      ) : null}
    </>
  );
}
