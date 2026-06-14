export function formatRowIndex(index, total) {
  return String(index + 1).padStart(String(total).length, '0');
}

export function previewFileListStyle(total) {
  return { '--preview-index-ch': String(total).length };
}
