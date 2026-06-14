import { Trash2 } from 'lucide-react';
import { ServerSyncPreviewFileRow } from './ServerSyncPreviewFileRow.jsx';
import { previewFileListStyle } from './previewFileList.js';
import { ServerSyncPreviewModSideTag } from './ServerSyncPreviewModSideTag.jsx';
import { ServerSyncPreviewStat } from './ServerSyncPreviewStat.jsx';

export function ServerSyncPreviewDeleteStat({
  count,
  files,
  title = 'Будет удалено',
  label
}) {
  return (
    <ServerSyncPreviewStat
      icon={Trash2}
      count={count}
      variant="delete"
      label={label ?? `Будет удалено: ${count}`}
      title={title}
    >
      <ul className="serverSyncPreviewFileList" style={previewFileListStyle(files.length)}>
        {files.map((item, index) => {
          const filename = typeof item === 'string' ? item : item.filename;
          const side = typeof item === 'string' ? 'universal' : item.side;

          return (
            <ServerSyncPreviewFileRow
              key={filename}
              index={index}
              total={files.length}
              variant="delete"
              marker={<ServerSyncPreviewModSideTag side={side} />}
            >
              <span className="serverSyncPreviewFileName">{filename}</span>
            </ServerSyncPreviewFileRow>
          );
        })}
      </ul>
    </ServerSyncPreviewStat>
  );
}
