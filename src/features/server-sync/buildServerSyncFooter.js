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
      main: ui.doneParts.uploaded,
      side: ui.doneParts.skipped,
      extra: ui.doneParts.extra,
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
    const item = items[0];
    return [item.main, item.side, item.extra].filter(Boolean).join(' ');
  }

  return items
    .map((item) => {
      const lane = LANES.find((entry) => entry.key === item.lane);
      return `${lane?.shortLabel ?? item.lane}: ${item.main}`;
    })
    .join(' · ');
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
    labelWarn: items.some((item) => item.error)
  };
}

export function mergeWorkspaceFooters(workspace, serverSync) {
  if (serverSync?.show) return serverSync;
  return workspace;
}
