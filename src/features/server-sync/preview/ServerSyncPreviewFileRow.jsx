import { formatRowIndex } from './previewFileList.js';

export function ServerSyncPreviewFileRow({ index, total, variant, marker, children }) {
  const isTextMarker = typeof marker === 'string';

  return (
    <li className={`serverSyncPreviewFileRow serverSyncPreviewFileRow--${variant}`}>
      <span className="serverSyncPreviewFileIndex" aria-hidden="true">
        {formatRowIndex(index, total)}
      </span>
      {marker != null ? (
        <span
          className={`serverSyncPreviewFileMarker${isTextMarker ? '' : ' serverSyncPreviewFileMarker--tag'}`}
          aria-hidden="true"
        >
          {marker}
        </span>
      ) : null}
      <div className="serverSyncPreviewFileContent">{children}</div>
    </li>
  );
}
