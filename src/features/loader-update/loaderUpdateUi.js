export function loaderUpdateRowStatus(current, target, checked) {
  if (!checked || current == null) return { label: 'не проверен', tone: 'unknown' };
  if (!target?.trim()) return { label: '—', tone: 'neutral' };
  if (current === target) return { label: 'совпадает', tone: 'ok' };
  return { label: `→ ${target}`, tone: 'outdated' };
}

export function loaderUpdateActionUi({
  checked,
  currentVersion,
  targetVersion,
  applySupported,
  checking,
  applying
}) {
  const busy = checking || applying;
  const upToDate =
    checked &&
    Boolean(currentVersion?.trim()) &&
    Boolean(targetVersion?.trim()) &&
    currentVersion === targetVersion;
  const needsUpdate =
    checked &&
    Boolean(currentVersion?.trim()) &&
    Boolean(targetVersion?.trim()) &&
    currentVersion !== targetVersion;
  const confirmReady = needsUpdate && applySupported;

  if (busy) {
    return {
      mode: 'busy',
      label: checking ? 'Проверка…' : 'Обновление…',
      upToDate: false,
      confirmReady: false
    };
  }

  if (upToDate) {
    return { mode: 'ok', label: 'Актуально', upToDate: true, confirmReady: false };
  }

  if (confirmReady) {
    return { mode: 'apply', label: 'Обновить', upToDate: false, confirmReady: true };
  }

  return { mode: 'check', label: 'Проверить', upToDate: false, confirmReady: false };
}

export function pickDefaultTargetVersion(catalog, currentVersion) {
  const versions = catalog?.availableVersions ?? [];
  if (!versions.length) return '';
  if (currentVersion && versions.includes(currentVersion)) {
    return currentVersion;
  }
  return catalog?.latestVersion || versions[0] || '';
}
