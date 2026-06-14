import { formatFiles } from './serverSyncRu.js';

function buildDoneParts(progress) {
  const uploaded = progress.uploaded ?? 0;
  const skipped = progress.skipped ?? 0;
  const deleted = progress.deleted ?? 0;
  const deletedExtra = progress.deletedExtra ?? 0;
  const replacedRemote = progress.replacedRemote ?? 0;
  const extras = [];

  if (replacedRemote > 0) {
    extras.push(`Обновлено: ${replacedRemote}`);
  }
  if (deletedExtra > 0) {
    extras.push(`Удалено лишних: ${deletedExtra}`);
  } else if (deleted > 0 && replacedRemote === 0 && uploaded === 0) {
    extras.push(`Удалено ${formatFiles(deleted)}`);
  }
  if (progress.errors?.length) {
    extras.push(
      progress.errors.length === 1
        ? progress.errors[0]
        : `${progress.errors.length} ошибок`
    );
  }

  let primary;
  if (uploaded > 0) {
    primary = `Загружено: ${uploaded}`;
  } else if (deletedExtra > 0) {
    primary = `Удалено лишних: ${deletedExtra}`;
  } else if (deleted > 0) {
    primary = `Удалено: ${deleted}`;
  } else {
    primary = `Загружено: ${uploaded}`;
  }

  return {
    primary,
    uploaded: primary,
    skipped: skipped > 0 ? `Уже были на сервере: ${skipped}` : null,
    extra: extras.length ? extras.join(' · ') : null
  };
}

function formatDoneMessage(progress) {
  if (
    !progress.ok &&
    progress.errors?.length === 1 &&
    !(progress.uploaded ?? 0) &&
    !(progress.skipped ?? 0)
  ) {
    return progress.errors[0];
  }

  const parts = buildDoneParts(progress);
  return [parts.primary, parts.skipped, parts.extra].filter(Boolean).join(' ');
}

export function progressToLaneUi(progress) {
  if (!progress) {
    return {
      syncing: false,
      showResult: false,
      main: '',
      side: '',
      filename: '',
      phase: '',
      current: 0,
      total: 0,
      ok: false,
      error: false,
      doneParts: null
    };
  }

  const filename = progress.filename?.trim() ?? '';

  if (progress.active) {
    if (progress.phase === 'checking') {
      return {
        syncing: true,
        showResult: false,
        main: progress.totalAll
          ? `Проверка… ${progress.totalAll}`
          : 'Проверка…',
        side: '',
        filename: '',
        phase: 'checking',
        current: 0,
        total: 0,
        ok: false,
        error: false
      };
    }

    if (progress.phase === 'pruning') {
      const total = progress.total ?? 0;
      const already = progress.alreadySynced ?? 0;

      return {
        syncing: true,
        showResult: false,
        main: total > 0 ? `Удаление ${formatFiles(total)}…` : 'Удаление лишних…',
        side: already > 0 ? `Уже были на сервере: ${already}` : '',
        filename,
        phase: 'pruning',
        current: 0,
        total,
        ok: false,
        error: false
      };
    }

    const total = progress.total ?? 0;
    const current = progress.current ?? 0;
    const already = progress.alreadySynced ?? 0;

    if (total > 0) {
      return {
        syncing: true,
        showResult: false,
        main: `Синхронизация ${current}/${total}`,
        side: already ? `${already} уже на сервере` : '',
        filename,
        phase: 'uploading',
        current,
        total,
        ok: false,
        error: false
      };
    }

    if (already > 0) {
      return {
        syncing: true,
        showResult: false,
        main: `Все ${already} уже на сервере`,
        side: '',
        filename,
        phase: 'uploading',
        current: 0,
        total: 0,
        ok: false,
        error: false
      };
    }

    return {
      syncing: true,
      showResult: false,
      main: 'Синхронизация…',
      side: '',
      filename,
      phase: 'uploading',
      current: 0,
      total: 0,
      ok: false,
      error: false
    };
  }

  if (progress.done) {
    if (
      !progress.ok &&
      progress.errors?.length === 1 &&
      !(progress.uploaded ?? 0) &&
      !(progress.skipped ?? 0)
    ) {
      return {
        syncing: false,
        showResult: true,
        main: progress.errors[0],
        side: '',
        filename: '',
        doneParts: null,
        phase: '',
        current: 0,
        total: 0,
        ok: false,
        error: true
      };
    }

    const doneParts = buildDoneParts(progress);

    return {
      syncing: false,
      showResult: true,
      main: formatDoneMessage(progress),
      side: doneParts.skipped ?? '',
      filename: '',
      doneParts,
      phase: '',
      current: 0,
      total: 0,
      ok: Boolean(progress.ok),
      error: !progress.ok
    };
  }

  return {
    syncing: false,
    showResult: false,
    main: '',
    side: '',
    filename: '',
    doneParts: null,
    phase: '',
    current: 0,
    total: 0,
    ok: false,
    error: false
  };
}

export const EMPTY_LANE_UI = progressToLaneUi(null);
