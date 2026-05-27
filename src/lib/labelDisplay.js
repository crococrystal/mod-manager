export function canRefreshProviderLabels(mod) {
  return (
    (mod?.source === 'modrinth' && mod?.modrinthId) ||
    (mod?.source === 'curseforge' && mod?.curseforgeId)
  );
}

export function tagsForMode(mod, sideMode) {
  if (sideMode === 'manual') {
    const side = mod?.manualSide ?? mod?.side ?? 'universal';
    return {
      sideMode: 'manual',
      side: side === 'unknown' ? 'universal' : side,
      library: Boolean(mod?.manualLibrary),
      technical: Boolean(mod?.manualTechnical)
    };
  }
  return {
    sideMode: 'auto',
    side: mod?.providerSide ?? 'universal',
    library: Boolean(mod?.providerLibrary),
    technical: Boolean(mod?.providerTechnical)
  };
}
