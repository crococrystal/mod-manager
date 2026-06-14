function syncProgressLabel(progress) {
  if (!progress?.total) return 'Синхронизация · Подготовка…';
  const tail = progress.name ? ` · ${progress.name}` : '';
  return `Синхронизация · ${progress.index}/${progress.total}${tail}`;
}

export function buildWorkspaceFooter({
  workspaceInitializing,
  bootstrapping,
  syncing,
  scanning,
  progress,
  updateAllBusy,
  updateAllProgress,
  updatesError,
  updateAllError,
  updatesFailedProjects,
  updatesLoadingVisible = false,
  isUpdatesMode = false,
  updatesCenterLoadingVisible = false
}) {
  const updatesCheckActive = isUpdatesMode && updatesLoadingVisible;
  const globalProgressActive = workspaceInitializing || updatesCheckActive;
  const updateAllProgressActive = updateAllBusy;

  const notice =
    !globalProgressActive && !updateAllProgressActive
      ? updatesError ||
        updateAllError ||
        (updatesFailedProjects > 0
          ? `Не удалось проверить ${updatesFailedProjects} модов у поставщика.`
          : '')
      : '';

  const percent =
    updateAllProgress?.total > 0
      ? Math.min(100, Math.round((updateAllProgress.current / updateAllProgress.total) * 100))
      : progress?.total > 0
      ? Math.min(100, Math.round((progress.index / progress.total) * 100))
      : 0;

  const indeterminate = updateAllProgressActive
    ? true
    : updatesCheckActive ||
      ((bootstrapping || syncing || scanning) && !(progress?.total > 0));

  const flyingProgress = globalProgressActive || updateAllProgressActive;
  const scopedProgress =
    !updateAllProgressActive && flyingProgress && !indeterminate && percent > 0;
  const fullIndeterminate =
    updateAllProgressActive || indeterminate || (flyingProgress && percent === 0);

  const label = syncing
    ? syncProgressLabel(progress)
    : updateAllProgressActive && updateAllProgress?.total > 0
    ? `Обновление ${updateAllProgress.current}/${updateAllProgress.total}${
        updateAllProgress.title ? ` · ${updateAllProgress.title}` : ''
      }`
    : updateAllProgressActive
    ? 'Обновление модов…'
    : bootstrapping && progress
    ? `Подготовка · ${progress.index}/${progress.total}${progress.name ? ` · ${progress.name}` : ''}`
    : scanning
    ? 'Сканирование модов…'
    : updatesCheckActive
    ? 'Проверка обновлений модов…'
    : bootstrapping
    ? 'Подготовка…'
    : notice;

  return {
    show:
      !updatesCenterLoadingVisible &&
      (globalProgressActive || updateAllProgressActive || Boolean(notice)),
    trackActive: globalProgressActive || updateAllProgressActive,
    cancelable: bootstrapping || syncing,
    percent,
    indeterminate: fullIndeterminate,
    scopedProgress,
    label,
    labelWarn: Boolean(notice)
  };
}
