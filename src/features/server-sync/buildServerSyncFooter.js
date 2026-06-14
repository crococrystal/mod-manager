const LANES = [
  { key: 'server', shortLabel: 'Сервер' },
  { key: 'distribution', shortLabel: 'Automodpack' }
];

function laneFooterItem(lane, ui, visibleResult) {
  const show = ui.syncing || (visibleResult && ui.main);
  if (!show) return null;

  if (ui.doneParts) {
    return {
      lane,
      main: ui.doneParts.title,
      stats: {
        uploadCount: ui.doneParts.uploadCount,
        updateCount: ui.doneParts.updateCount,
        deleteCount: ui.doneParts.deleteCount,
        skipCount: ui.doneParts.skipCount,
        uploadFiles: ui.doneParts.uploadFiles,
        skipFiles: ui.doneParts.skipFiles,
        deleteFiles: ui.doneParts.deleteFiles,
        updatePairs: ui.doneParts.updatePairs
      },
      syncing: ui.syncing,
      error: ui.error,
      phase: ui.phase,
      current: ui.current,
      total: ui.total
    };
  }

  return {
    lane,
    main: ui.main,
    side: ui.side,
    extra: null,
    syncing: ui.syncing,
    error: ui.error,
    phase: ui.phase,
    current: ui.current,
    total: ui.total
  };
}

function formatFooterLabel(items) {
  if (items.length === 1) {
    return items[0].main;
  }

  return items
    .map((item) => {
      const lane = LANES.find((entry) => entry.key === item.lane);
      return `${lane?.shortLabel ?? item.lane}: ${item.main}`;
    })
    .join(' · ');
}

function footerStats(items) {
  if (items.length !== 1 || items[0].syncing || !items[0].stats) {
    return null;
  }

  const { uploadCount, updateCount, deleteCount, skipCount } = items[0].stats;
  if (
    uploadCount == null &&
    updateCount == null &&
    deleteCount == null &&
    skipCount == null
  ) {
    return null;
  }

  return items[0].stats;
}

export function buildServerSyncFooter({
  enabled,
  server,
  distribution,
  visibleResult
}) {
  if (!enabled) {
    return null;
  }

  const items = LANES.map(({ key }) =>
    laneFooterItem(key, key === 'server' ? server : distribution, visibleResult[key])
  ).filter(Boolean);

  if (!items.length) {
    return null;
  }

  const syncingItems = items.filter((item) => item.syncing);
  const active = syncingItems[0] ?? items[0];
  const hasUploadProgress = active.phase === 'uploading' && active.total > 0;
  const percent = hasUploadProgress
    ? Math.min(100, Math.max(0, Math.round((active.current / active.total) * 100)))
    : 0;

  return {
    show: true,
    trackActive: syncingItems.length > 0,
    cancelable: syncingItems.length > 0,
    cancelLanes: syncingItems.map((item) => item.lane),
    percent,
    indeterminate: syncingItems.length > 0 && !hasUploadProgress,
    scopedProgress: syncingItems.length > 0 && hasUploadProgress,
    label: formatFooterLabel(items),
    stats: footerStats(items),
    labelWarn: items.some((item) => item.error)
  };
}

export function mergeWorkspaceFooters(workspace, serverSync) {
  if (serverSync?.show) return serverSync;
  return workspace;
}
