import { Check } from 'lucide-react';
import { ServerSyncPreviewFileRow } from './ServerSyncPreviewFileRow.jsx';
import { previewFileListStyle } from './previewFileList.js';
import { ServerSyncPreviewStat } from './ServerSyncPreviewStat.jsx';

export function ServerSyncPreviewSkipStat({
  count,
  files,
  title = 'Уже на сервере',
  label
}) {
  return (
    <ServerSyncPreviewStat
      icon={Check}
      count={count}
      variant="skip"
      label={label ?? `Уже на сервере: ${count}`}
      title={title}
    >
      <ul className="serverSyncPreviewFileList" style={previewFileListStyle(files.length)}>
        {files.map((name, index) => (
          <ServerSyncPreviewFileRow
            key={name}
            index={index}
            total={files.length}
            variant="skip"
            marker="✓"
          >
            <span className="serverSyncPreviewFileName">{name}</span>
          </ServerSyncPreviewFileRow>
        ))}
      </ul>
    </ServerSyncPreviewStat>
  );
}
