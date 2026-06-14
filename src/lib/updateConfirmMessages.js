export function formatUpdateAllConfirmMessage(count) {
  const mod = Math.abs(count) % 100;
  const last = mod % 10;
  if (count === 1) return 'Обновить 1 мод до последней доступной версии?';
  if (mod > 10 && mod < 20) return `Обновить ${count} модов до последних доступных версий?`;
  if (last > 1 && last < 5) return `Обновить ${count} мода до последних доступных версий?`;
  return `Обновить ${count} модов до последних доступных версий?`;
}

export function formatSingleUpdateConfirmMessage(mod, version) {
  const name = mod?.displayName ?? 'мод';
  const versionNumber = version?.versionNumber ?? version?.name ?? 'новой версии';
  return `Обновить ${name} до версии ${versionNumber}?`;
}
