import { Upload } from 'lucide-react';
import { ServerSyncPreviewFileRow } from './ServerSyncPreviewFileRow.jsx';
import { previewFileListStyle } from './previewFileList.js';
import { ServerSyncPreviewStat } from './ServerSyncPreviewStat.jsx';

export function ServerSyncPreviewUploadStat({ count, files }) {
  return (
    <ServerSyncPreviewStat
      icon={Upload}
      count={count}
      variant="upload"
      label={`Будет отправлено: ${count}`}
      title="Будет отправлено"
    >
      <ul className="serverSyncPreviewFileList" style={previewFileListStyle(files.length)}>
        {files.map((name, index) => (
          <ServerSyncPreviewFileRow
            key={name}
            index={index}
            total={files.length}
            variant="upload"
            marker="+"
          >
            <span className="serverSyncPreviewFileName">{name}</span>
          </ServerSyncPreviewFileRow>
        ))}
      </ul>
    </ServerSyncPreviewStat>
  );
}
