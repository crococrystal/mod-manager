export const EMPTY_PREVIEW_OVERLAY = {
  checking: false,
  starting: false,
  ready: false,
  main: '',
  side: '',
  previewParts: null,
  ok: false,
  error: false
};

function pluralMods(count) {
  const mod100 = count % 100;
  const mod10 = count % 10;
  if (mod100 >= 11 && mod100 <= 14) return 'модов';
  if (mod10 === 1) return 'мод';
  if (mod10 >= 2 && mod10 <= 4) return 'мода';
  return 'модов';
}

export function buildPreviewParts(preview) {
  if (!preview?.ok) {
    return null;
  }

  const local = preview.local ?? 0;
  const synced = preview.alreadySynced ?? 0;
  const toUpload = preview.toUpload ?? 0;
  const toDelete = preview.toDelete ?? 0;
  const toUpdate = preview.toUpdate ?? 0;
  const upToDate = toUpload === 0 && toDelete === 0 && toUpdate === 0 && local > 0;

  return {
    sync: `Модов: ${synced} из ${local} ${pluralMods(local)}`,
    uploadCount: toUpload > 0 ? toUpload : null,
    updateCount: toUpdate > 0 ? toUpdate : null,
    deleteCount: toDelete > 0 ? toDelete : null,
    uploadFiles: preview.toUploadNames ?? [],
    updatePairs: preview.toUpdatePairs ?? [],
    deleteFiles: preview.toDeleteItems?.length
      ? preview.toDeleteItems
      : (preview.toDeleteNames ?? []).map((filename) => ({ filename, side: 'universal' })),
    matches: upToDate ? 'Папка соответствует' : null,
    upToDate
  };
}

export function formatPreviewAriaLabel(previewParts) {
  if (!previewParts) return '';
  const parts = [previewParts.sync];
  if (previewParts.uploadCount != null) {
    parts.push(`Будет отправлено: ${previewParts.uploadCount}`);
  }
  if (previewParts.updateCount != null) {
    parts.push(`будет обновлено ${previewParts.updateCount}`);
  }
  if (previewParts.deleteCount != null) {
    parts.push(`будет удалено ${previewParts.deleteCount}`);
  }
  if (previewParts.matches) {
    parts.push(previewParts.matches);
  }
  return parts.join(' ');
}

export function previewToOverlayUi(preview, { checking = false, starting = false } = {}) {
  if (starting) {
    return {
      ...EMPTY_PREVIEW_OVERLAY,
      starting: true,
      main: 'Синхронизация…'
    };
  }

  if (checking) {
    return {
      ...EMPTY_PREVIEW_OVERLAY,
      checking: true,
      main: 'Проверка…'
    };
  }

  if (!preview) {
    return EMPTY_PREVIEW_OVERLAY;
  }

  if (!preview.ok) {
    return {
      checking: false,
      ready: true,
      main: preview.errors?.[0] || 'Ошибка проверки.',
      side: '',
      previewParts: null,
      ok: false,
      error: true
    };
  }

  const previewParts = buildPreviewParts(preview);

  return {
    checking: false,
    ready: true,
    main: formatPreviewAriaLabel(previewParts),
    side: '',
    previewParts,
    ok: true,
    error: false
  };
}
