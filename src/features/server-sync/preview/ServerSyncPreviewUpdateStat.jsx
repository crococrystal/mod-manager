import { RefreshCw } from 'lucide-react';
import { formatRowIndex, previewFileListStyle } from './previewFileList.js';
import { ServerSyncPreviewStat } from './ServerSyncPreviewStat.jsx';

export function ServerSyncPreviewUpdateStat({ count, pairs }) {
  return (
    <ServerSyncPreviewStat
      icon={RefreshCw}
      count={count}
      variant="update"
      label={`Будет обновлено: ${count}`}
      title="Будет обновлено"
    >
      <ul
        className="serverSyncPreviewFileList serverSyncPreviewFileList--update"
        style={previewFileListStyle(pairs.length)}
      >
        {pairs.map((pair, index) => (
          <li key={pair.remote} className="serverSyncPreviewFileRow serverSyncPreviewFileRow--update">
            <span className="serverSyncPreviewFileIndex" aria-hidden="true">
              {formatRowIndex(index, pairs.length)}
            </span>
            <div className="serverSyncPreviewFileLine serverSyncPreviewFileLine--remote">
              <span className="serverSyncPreviewFileMarker" aria-hidden="true">
                −
              </span>
              <span className="serverSyncPreviewFileRemote">{pair.remote}</span>
            </div>
            <div className="serverSyncPreviewFileLine serverSyncPreviewFileLine--local">
              <span className="serverSyncPreviewFileMarker" aria-hidden="true">
                +
              </span>
              <span className="serverSyncPreviewFileLocal">{pair.local}</span>
            </div>
          </li>
        ))}
      </ul>
    </ServerSyncPreviewStat>
  );
}
