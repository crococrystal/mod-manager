export function projectIdFor(mod) {
  if (mod?.source === 'modrinth') return mod.modrinthId;
  if (mod?.source === 'curseforge') return mod.curseforgeId;
  return null;
}

export function formatNumber(value) {
  if (value == null) return '';
  return new Intl.NumberFormat('ru', { notation: 'compact', maximumFractionDigits: 1 }).format(value);
}

export function formatSize(value) {
  if (!value) return '';
  const units = ['Б', 'КБ', 'МБ', 'ГБ'];
  let size = value;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

export function versionMeta(version) {
  const parts = [];
  if (version.gameVersions?.length) parts.push(version.gameVersions.slice(0, 3).join(', '));
  if (version.loaders?.length) parts.push(version.loaders.join(', '));
  if (version.size) parts.push(formatSize(version.size));
  if (version.downloads != null) parts.push(formatNumber(version.downloads));
  return parts.join(' · ');
}

export function versionTypeLabel(releaseType) {
  return (releaseType || 'release').slice(0, 1).toUpperCase();
}

export function versionTypeClass(releaseType) {
  const kind = (releaseType || 'release').trim().toLowerCase();
  if (kind.startsWith('b')) return 'versionTypeBeta';
  if (kind.startsWith('a')) return 'versionTypeAlpha';
  return 'versionTypeRelease';
}

export function isInstalledVersion(mod, version) {
  if (!mod || !version) return false;
  if (mod.source === 'modrinth' && mod.modrinthVersionId && mod.modrinthVersionId === version.id) {
    return true;
  }
  const fileId = version.fileId ?? version.id;
  if (mod.source === 'curseforge' && mod.curseforgeFileId && mod.curseforgeFileId === fileId) {
    return true;
  }
  return Boolean(mod.filename && version.filename && mod.filename === version.filename);
}
