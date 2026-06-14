import { ServerSyncPreviewDeleteStat } from './preview/ServerSyncPreviewDeleteStat.jsx';
import { ServerSyncPreviewSkipStat } from './preview/ServerSyncPreviewSkipStat.jsx';
import { ServerSyncPreviewUpdateStat } from './preview/ServerSyncPreviewUpdateStat.jsx';
import { ServerSyncPreviewUploadStat } from './preview/ServerSyncPreviewUploadStat.jsx';

function normalizeDeleteFiles(files) {
  if (!files?.length) return [];
  return files.map((item) => (typeof item === 'string' ? item : item.filename));
}

export function ServerSyncDoneStats({
  uploadCount,
  updateCount,
  deleteCount,
  skipCount,
  uploadFiles,
  skipFiles,
  deleteFiles,
  updatePairs,
  className = 'serverSyncPathOverlayRight'
}) {
  const hasStats =
    uploadCount != null ||
    updateCount != null ||
    deleteCount != null ||
    skipCount != null;

  if (!hasStats) {
    return null;
  }

  const deleteList = normalizeDeleteFiles(deleteFiles);
  const deleteItems =
    deleteFiles?.length && typeof deleteFiles[0] === 'object' ? deleteFiles : deleteList;

  return (
    <span className={className}>
      {uploadCount != null ? (
        <ServerSyncPreviewUploadStat
          count={uploadCount}
          files={uploadFiles ?? []}
          title="Отправлено"
          label={`Отправлено: ${uploadCount}`}
        />
      ) : null}
      {updateCount != null ? (
        <ServerSyncPreviewUpdateStat
          count={updateCount}
          pairs={updatePairs ?? []}
          title="Обновлено"
          label={`Обновлено: ${updateCount}`}
        />
      ) : null}
      {deleteCount != null ? (
        <ServerSyncPreviewDeleteStat
          count={deleteCount}
          files={deleteItems}
          title="Удалено"
          label={`Удалено: ${deleteCount}`}
        />
      ) : null}
      {skipCount != null ? (
        <ServerSyncPreviewSkipStat
          count={skipCount}
          files={skipFiles ?? []}
          title="Уже на сервере"
          label={`Уже на сервере: ${skipCount}`}
        />
      ) : null}
    </span>
  );
}
