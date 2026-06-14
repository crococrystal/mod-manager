import { ArrowRight, Check, RefreshCw, X } from 'lucide-react';
import { IconButton } from '../../components/Button.jsx';
import { ServerSyncPreviewDeleteStat } from './preview/ServerSyncPreviewDeleteStat.jsx';
import { ServerSyncPreviewUpdateStat } from './preview/ServerSyncPreviewUpdateStat.jsx';
import { ServerSyncPreviewUploadStat } from './preview/ServerSyncPreviewUploadStat.jsx';

export function ServerSyncPathField({
  id,
  label,
  hint,
  value,
  placeholder,
  inputDisabled,
  syncDisabled,
  laneUi,
  previewUi,
  showResult,
  onChange,
  onBlur,
  onAction,
  onCancel,
  onDismissPreview,
  onDismissResult,
  onEditStart
}) {
  const showSyncOverlay = laneUi.syncing || (showResult && laneUi.main);
  const showPreviewOverlay = Boolean(
    !showSyncOverlay && (previewUi.checking || previewUi.starting || previewUi.ready)
  );
  const showOverlay = showSyncOverlay || showPreviewOverlay;
  const overlayUi = showSyncOverlay ? laneUi : previewUi;
  const showProgressBar = laneUi.syncing || previewUi.starting;
  const showDoneResult = showSyncOverlay && showResult && laneUi.doneParts;
  const doneDismissible = Boolean(showDoneResult && laneUi.ok);
  const previewUpToDate = Boolean(previewUi.previewParts?.upToDate);
  const previewWillUpload = Boolean(previewUi.previewParts?.uploadCount);
  const previewWillUpdate = Boolean(previewUi.previewParts?.updateCount);
  const confirmReady = Boolean(
    previewUi.ready && previewUi.ok && !previewUi.error && !previewUpToDate
  );
  const ActionIcon =
    doneDismissible || previewUpToDate ? Check : confirmReady ? ArrowRight : RefreshCw;
  const actionLabel =
    doneDismissible || previewUpToDate ? 'Готово' : confirmReady ? 'Отправить' : 'Проверить';

  const hasDeterminateProgress =
    laneUi.syncing && laneUi.phase === 'uploading' && laneUi.total > 0;
  const percent = hasDeterminateProgress
    ? Math.min(100, Math.max(0, Math.round((laneUi.current / laneUi.total) * 100)))
    : null;
  const hintText = laneUi.syncing && laneUi.filename ? laneUi.filename : hint;
  const hintClassName = [
    'cacheHint',
    laneUi.syncing && laneUi.filename ? 'cacheHint--syncFile' : ''
  ]
    .filter(Boolean)
    .join(' ');

  function handleOverlayClick() {
    if (laneUi.syncing) return;
    if (doneDismissible) {
      onDismissResult?.();
      return;
    }
    if (showPreviewOverlay) {
      onDismissPreview();
    }
  }

  return (
    <div className="field">
      <div className="fieldHeader">
        <label htmlFor={id}>{label}</label>
      </div>
      <div className="pathField serverSyncPathField">
        <div
          className={[
            'serverSyncPathInputWrap',
            showOverlay ? 'serverSyncPathInputWrap--overlay' : ''
          ]
            .filter(Boolean)
            .join(' ')}
        >
          <input
            id={id}
            value={value}
            disabled={inputDisabled || laneUi.syncing || previewUi.checking || previewUi.starting}
            placeholder={placeholder}
            autoCapitalize="none"
            autoCorrect="off"
            spellCheck={false}
            aria-hidden={showOverlay || undefined}
            tabIndex={showOverlay ? -1 : undefined}
            onChange={onChange}
            onBlur={onBlur}
            onFocus={onEditStart}
          />
          {showOverlay ? (
            <div
              className={[
                'serverSyncPathOverlay',
                overlayUi.syncing || overlayUi.checking || overlayUi.starting
                  ? 'serverSyncPathOverlay--pending'
                  : '',
                overlayUi.previewParts ? 'serverSyncPathOverlay--preview' : '',
                showDoneResult ? 'serverSyncPathOverlay--preview' : '',
                doneDismissible ||
                (overlayUi.ok && !overlayUi.previewParts && !showDoneResult)
                  ? 'serverSyncPathOverlay--ok'
                  : '',
                overlayUi.error ? 'serverSyncPathOverlay--error' : ''
              ]
                .filter(Boolean)
                .join(' ')}
              role="status"
              aria-live="polite"
              onClick={handleOverlayClick}
            >
              <div className="serverSyncPathOverlayRow">
                {showDoneResult ? (
                  <>
                    <span className="serverSyncPathOverlayText serverSyncPathOverlayUploaded">
                      {laneUi.doneParts.uploaded}
                    </span>
                    {laneUi.doneParts.skipped ? (
                      <span className="serverSyncPathOverlaySkipped">
                        {laneUi.doneParts.skipped}
                      </span>
                    ) : null}
                    {laneUi.doneParts.extra ? (
                      <span className="serverSyncPathOverlaySide">{laneUi.doneParts.extra}</span>
                    ) : null}
                  </>
                ) : showPreviewOverlay && previewUi.previewParts ? (
                  <>
                    <span className="serverSyncPathOverlayText">
                      {previewUi.previewParts.sync}
                    </span>
                    {previewUi.previewParts.uploadCount != null ||
                    previewUi.previewParts.updateCount != null ||
                    previewUi.previewParts.deleteCount != null ||
                    previewUi.previewParts.matches ? (
                      <span className="serverSyncPathOverlayRight">
                        {previewUi.previewParts.uploadCount != null ? (
                          <ServerSyncPreviewUploadStat
                            count={previewUi.previewParts.uploadCount}
                            files={previewUi.previewParts.uploadFiles}
                          />
                        ) : null}
                        {previewUi.previewParts.updateCount != null ? (
                          <ServerSyncPreviewUpdateStat
                            count={previewUi.previewParts.updateCount}
                            pairs={previewUi.previewParts.updatePairs}
                          />
                        ) : null}
                        {previewUi.previewParts.deleteCount != null ? (
                          <ServerSyncPreviewDeleteStat
                            count={previewUi.previewParts.deleteCount}
                            files={previewUi.previewParts.deleteFiles}
                          />
                        ) : null}
                        {previewUi.previewParts.matches ? (
                          <span className="serverSyncPathOverlayMatches">
                            {previewUi.previewParts.matches}
                          </span>
                        ) : null}
                      </span>
                    ) : null}
                  </>
                ) : laneUi.syncing && laneUi.side && laneUi.phase === 'pruning' ? (
                  <>
                    <span className="serverSyncPathOverlayText">{overlayUi.main}</span>
                    <span className="serverSyncPathOverlaySkipped">{overlayUi.side}</span>
                  </>
                ) : laneUi.syncing && laneUi.side ? (
                  <>
                    <span className="serverSyncPathOverlayText">{overlayUi.main}</span>
                    <span className="serverSyncPathOverlaySide">{overlayUi.side}</span>
                  </>
                ) : (
                  <span className="serverSyncPathOverlayText">{overlayUi.main}</span>
                )}
                {laneUi.syncing ? (
                  <button
                    type="button"
                    className="serverSyncPathOverlayCancel"
                    aria-label="Отменить синхронизацию"
                    onClick={(event) => {
                      event.stopPropagation();
                      onCancel();
                    }}
                  >
                    <X size={12} strokeWidth={2.2} aria-hidden="true" />
                  </button>
                ) : null}
              </div>
              {showProgressBar ? (
                <div className="serverSyncPathOverlayTrack" aria-hidden="true">
                  <div
                    className={`serverSyncPathOverlayBar${hasDeterminateProgress ? '' : ' indeterminate'}`}
                    style={hasDeterminateProgress ? { width: `${percent}%` } : undefined}
                  />
                </div>
              ) : null}
            </div>
          ) : null}
        </div>
        <IconButton
          icon={ActionIcon}
          label={actionLabel}
          className={[
            laneUi.syncing || previewUi.checking || previewUi.starting
              ? 'serverSyncPathSyncBtn--spinning'
              : '',
            previewUpToDate || doneDismissible || (confirmReady && !previewWillUpload)
              ? 'serverSyncPathSyncBtn--confirm'
              : '',
            confirmReady && (previewWillUpload || previewWillUpdate) ? 'serverSyncPathSyncBtn--confirmUpload' : ''
          ]
            .filter(Boolean)
            .join(' ')}
          onMouseDown={(event) => event.preventDefault()}
          onClick={onAction}
          disabled={syncDisabled || laneUi.syncing || previewUi.checking || previewUi.starting}
        />
      </div>
      <p className={hintClassName} title={hintText}>
        {hintText}
      </p>
    </div>
  );
}
