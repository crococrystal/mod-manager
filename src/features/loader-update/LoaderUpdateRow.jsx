import { ArrowRight, Check, RefreshCw } from 'lucide-react';
import { IconButton } from '../../components/Button.jsx';
import { loaderUpdateActionUi, loaderUpdateRowStatus } from './loaderUpdateUi.js';

function formatVersion(value, checked) {
  if (!checked || value == null) return '—';
  return value?.trim() ? value : '—';
}

export function LoaderUpdateRow({
  label,
  currentVersion,
  targetVersion,
  checked,
  applySupported,
  checking,
  applying,
  error,
  actionDisabled,
  onAction
}) {
  const busy = checking || applying;
  const showOverlay = busy || Boolean(error?.trim());
  const status = loaderUpdateRowStatus(currentVersion, targetVersion, checked);
  const action = loaderUpdateActionUi({
    checked,
    currentVersion,
    targetVersion,
    applySupported,
    checking,
    applying
  });
  const ActionIcon =
    action.upToDate ? Check : action.confirmReady ? ArrowRight : RefreshCw;

  const overlayMain = checking ? 'Проверка…' : 'Обновление…';
  const overlaySide =
    applying && targetVersion?.trim()
      ? `→ ${targetVersion.trim()}`
      : checking
        ? label.toLowerCase()
        : '';

  return (
    <div className="loaderUpdateRow">
      <div
        className={[
          'loaderUpdateRowMain',
          showOverlay ? 'loaderUpdateRowMain--overlay' : ''
        ]
          .filter(Boolean)
          .join(' ')}
      >
        <div className="loaderUpdateRowContent" aria-hidden={showOverlay || undefined}>
          <span className="loaderUpdateRowLabel">{label}</span>
          <span className="loaderUpdateRowCurrent">{formatVersion(currentVersion, checked)}</span>
          <span className={`loaderUpdateStatus loaderUpdateStatus--${status.tone}`}>
            {status.label}
          </span>
        </div>
        {showOverlay ? (
          error?.trim() && !busy ? (
            <div
              className="serverSyncPathOverlay serverSyncPathOverlay--error loaderUpdateRowOverlay"
              role="status"
              aria-live="polite"
            >
              <div className="serverSyncPathOverlayRow">
                <span className="serverSyncPathOverlayText">{error}</span>
              </div>
            </div>
          ) : busy ? (
            <div
              className="serverSyncPathOverlay serverSyncPathOverlay--pending loaderUpdateRowOverlay"
              role="status"
              aria-live="polite"
            >
              <div className="serverSyncPathOverlayRow">
                <span className="serverSyncPathOverlayText">{overlayMain}</span>
                {overlaySide ? (
                  <span className="serverSyncPathOverlaySide">{overlaySide}</span>
                ) : null}
              </div>
              <div className="serverSyncPathOverlayTrack" aria-hidden="true">
                <div className="serverSyncPathOverlayBar indeterminate" />
              </div>
            </div>
          ) : null
        ) : null}
      </div>
      <IconButton
        icon={ActionIcon}
        label={`${action.label} ${label.toLowerCase()}`}
        className={[
          busy ? 'serverSyncPathSyncBtn--spinning' : '',
          action.upToDate ? 'serverSyncPathSyncBtn--confirm' : '',
          action.confirmReady ? 'serverSyncPathSyncBtn--confirmUpload' : ''
        ]
          .filter(Boolean)
          .join(' ')}
        disabled={actionDisabled || busy || action.upToDate}
        onMouseDown={(event) => event.preventDefault()}
        onClick={onAction}
      />
    </div>
  );
}
