import { Check, RefreshCw, Search, Settings, SlidersHorizontal, X } from 'lucide-react';
import headerAppLogo from '../assets/header-app-logo.svg';
import { filters, sourceIcons } from '../lib/modMeta.jsx';

export function AppToolbar({
  canShowWorkspace,
  query,
  searchSource,
  filter,
  settingsOpen,
  busy,
  updatesLoading = false,
  updatesStatus = 'idle',
  onQueryChange,
  onClearQuery,
  onToggleSearchSource,
  onFilterChange,
  onOpenSettings
}) {
  if (!canShowWorkspace) {
    return (
      <div className="topToolbar topToolbarEmpty" data-tauri-drag-region>
        <img
          src={headerAppLogo}
          alt="Mod Manager"
          className="topToolbarLogo"
          data-tauri-drag-region
        />
        <div className="segments" data-tauri-drag-region>
          <button
            type="button"
            className={`segmentsSettings${settingsOpen ? ' active' : ''}`}
            onClick={onOpenSettings}
            disabled={busy}
            aria-label="Настройки"
            title="Настройки"
            data-tauri-drag-region="false"
          >
            <Settings size={13} />
          </button>
        </div>
      </div>
    );
  }

  const updatesActive = searchSource === 'updates';
  const updatesIconBusy = updatesLoading;
  const updatesIconClass =
    updatesStatus === 'available'
      ? ' searchProviderToggleUpdates--available'
      : updatesStatus === 'current'
      ? ' searchProviderToggleUpdates--current'
      : updatesIconBusy
      ? ' searchProviderToggleUpdates--loading'
      : '';
  const updatesTitle = updatesIconBusy
    ? 'Проверка обновлений модов…'
    : updatesStatus === 'available'
    ? 'Есть доступные обновления'
    : updatesStatus === 'current'
    ? 'Все моды обновлены'
    : updatesActive
    ? 'Закрыть обновления'
    : 'Проверить обновления модов';
  const UpdatesIcon = updatesStatus === 'current' && !updatesIconBusy ? Check : RefreshCw;

  return (
    <div className="topToolbar" data-tauri-drag-region>
      <img
        src={headerAppLogo}
        alt="Mod Manager"
        className="topToolbarLogo"
        data-tauri-drag-region
      />
      <div className="segments" data-tauri-drag-region>
        {filters.map((item) => {
          const isActive = filter === item.id;
          const Icon = item.icon ?? SlidersHorizontal;
          const showIcon = Boolean(item.icon) || !isActive;
          return (
            <button
              key={item.id}
              className={isActive ? 'active' : ''}
              onClick={() => onFilterChange(item.id)}
              type="button"
              disabled={busy}
              title={item.label}
              aria-label={item.label}
              data-tauri-drag-region="false"
            >
              {showIcon ? <Icon className={`tagIcon ${item.tone ?? ''}`} size={13} /> : null}
              {isActive ? <span>{item.label}</span> : null}
            </button>
          );
        })}
        <button
          type="button"
          className={`segmentsSettings${settingsOpen ? ' active' : ''}`}
          onClick={onOpenSettings}
          disabled={busy}
          aria-label="Настройки"
          title="Настройки"
          data-tauri-drag-region="false"
        >
          <Settings size={13} />
        </button>
      </div>
      <label className="search" data-tauri-drag-region="false">
        <Search size={14} />
        <input
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder={
            searchSource === 'updates'
              ? 'Фильтр списка обновлений'
              : searchSource === 'modrinth'
              ? 'Поиск на Modrinth'
              : searchSource === 'curseforge'
              ? 'Поиск на CurseForge'
              : 'Поиск по названию или файлу'
          }
          data-tauri-drag-region="false"
        />
        {query ? (
          <button
            type="button"
            className="searchClear"
            onClick={onClearQuery}
            disabled={busy}
            aria-label="Очистить поиск"
            title="Очистить поиск"
            data-tauri-drag-region="false"
          >
            <X size={14} strokeWidth={2} />
          </button>
        ) : null}
        <span className="searchProviderToggles">
          {['modrinth', 'curseforge'].map((source) => {
            const icon = sourceIcons[source]?.icon;
            const active = searchSource === source;
            const label = sourceIcons[source]?.label ?? source;
            return (
              <button
                key={source}
                type="button"
                className={`searchProviderToggle${active ? ' active' : ''}`}
                onClick={() => onToggleSearchSource(source)}
                disabled={busy}
                title={active ? `${label}: локальный поиск` : `Искать на ${label}`}
                aria-label={active ? `Отключить поиск на ${label}` : `Искать на ${label}`}
                aria-pressed={active}
              >
                {icon ? <img src={icon} alt="" /> : null}
              </button>
            );
          })}
          <button
            type="button"
            className={`searchProviderToggle searchProviderToggleUpdates${updatesActive ? ' active' : ''}${updatesIconClass}`}
            onClick={() => onToggleSearchSource('updates')}
            disabled={busy}
            title={updatesTitle}
            aria-label={updatesTitle}
            aria-pressed={updatesActive}
          >
            <UpdatesIcon
              size={14}
              className={updatesIconBusy ? 'spin' : undefined}
              strokeWidth={updatesStatus === 'current' ? 2.5 : 2}
            />
          </button>
        </span>
      </label>
    </div>
  );
}
