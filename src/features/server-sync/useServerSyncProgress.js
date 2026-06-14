import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { cancelServerSyncLane, getServerSyncStatuses } from './serverSyncApi.js';
import { EMPTY_LANE_UI, progressToLaneUi } from './serverSyncProgressUi.js';

export function useServerSyncProgress() {
  const [lanes, setLanes] = useState({
    server: EMPTY_LANE_UI,
    distribution: EMPTY_LANE_UI
  });
  const [visibleResult, setVisibleResult] = useState({
    server: false,
    distribution: false
  });
  const dismissedResultRef = useRef({
    server: false,
    distribution: false
  });

  const applyLaneProgress = useCallback((lane, progress, { showDone = false } = {}) => {
    const ui = progressToLaneUi(progress);
    setLanes((current) => ({ ...current, [lane]: ui }));

    if (ui.syncing) {
      dismissedResultRef.current[lane] = false;
      setVisibleResult((current) => ({ ...current, [lane]: false }));
      return;
    }

    if (ui.showResult && (showDone || progress?.ok) && !dismissedResultRef.current[lane]) {
      setVisibleResult((current) => ({ ...current, [lane]: true }));
      return;
    }

    setVisibleResult((current) => ({ ...current, [lane]: false }));
  }, []);

  const applyProgress = useCallback(
    (progress) => {
      const lane = progress?.target;
      if (lane !== 'server' && lane !== 'distribution') return;
      applyLaneProgress(lane, progress, { showDone: true });
    },
    [applyLaneProgress]
  );

  const refresh = useCallback(async () => {
    try {
      const statuses = await getServerSyncStatuses();
      applyLaneProgress('server', statuses?.server);
      applyLaneProgress('distribution', statuses?.distribution);
      return statuses;
    } catch (err) {
      console.error('get_server_sync_statuses failed', err);
      return null;
    }
  }, [applyLaneProgress]);

  const cancel = useCallback(
    async (lane) => {
      try {
        await cancelServerSyncLane(lane);
        await refresh();
      } catch (err) {
        console.error('cancel_server_sync_lane failed', err);
      }
    },
    [refresh]
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten = null;

    void refresh();

    void listen('server-sync-progress', (event) => {
      if (!cancelled) {
        applyProgress(event.payload);
      }
    }).then((off) => {
      if (cancelled) {
        void off();
      } else {
        unlisten = off;
      }
    });

    return () => {
      cancelled = true;
      if (typeof unlisten === 'function') {
        void unlisten();
      }
    };
  }, [applyProgress, refresh]);

  const dismissLaneResult = useCallback((lane) => {
    dismissedResultRef.current[lane] = true;
    setVisibleResult((current) => ({ ...current, [lane]: false }));
  }, []);

  const server = lanes.server;
  const distribution = lanes.distribution;
  const syncing = server.syncing || distribution.syncing;

  return {
    server,
    distribution,
    visibleResult,
    syncing,
    refresh,
    cancel,
    dismissLaneResult
  };
}
