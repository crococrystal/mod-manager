import { RefreshCw } from 'lucide-react';

function formatModCount(count) {
  const mod = Math.abs(count) % 100;
  const last = mod % 10;
  if (mod > 10 && mod < 20) return `${count} модов`;
  if (last > 1 && last < 5) return `${count} мода`;
  if (last === 1) return `${count} мод`;
  return `${count} модов`;
}

export function UpdatesVersionPanelEmpty({
  placement = 'center',
  updateCount = 0,
  busy = false,
  error = '',
  onUpdateAllRequest
}) {
  const isFooter = placement === 'footer';
  const canUpdate = updateCount > 0 && !busy;

  const content = (
    <div className="updatesVersionPanelEmptyContent">
      {updateCount > 0 ? (
        <p className="updatesVersionPanelEmptyTitle">{formatModCount(updateCount)}</p>
      ) : null}
      <p className="updatesVersionPanelEmptyText">
        Обновите все моды
        <br />
        до последних версий
      </p>
      {error ? <p className="updatesVersionPanelEmptyError">{error}</p> : null}
    </div>
  );

  if (isFooter) {
    return (
      <button
        type="button"
        className={`updatesVersionPanelEmpty updatesVersionPanelEmpty--footer${
          canUpdate ? ' updatesVersionPanelEmpty--interactive' : ''
        }`}
        onClick={onUpdateAllRequest}
        disabled={!canUpdate}
        aria-label="Обновить все моды до последних версий"
      >
        <div className="updatesVersionPanelEmptyRow">
          {content}
          <RefreshCw className="updatesVersionPanelEmptyIcon" strokeWidth={2} aria-hidden />
        </div>
      </button>
    );
  }

  return (
    <button
      type="button"
      className={`updatesVersionPanelEmpty updatesVersionPanelEmpty--center${
        canUpdate ? ' updatesVersionPanelEmpty--interactive' : ''
      }`}
      onClick={onUpdateAllRequest}
      disabled={!canUpdate}
      aria-label="Обновить все моды до последних версий"
    >
      <RefreshCw className="updatesVersionPanelEmptyIcon" size={40} strokeWidth={2} aria-hidden />
      {content}
    </button>
  );
}
