export function canRefreshProviderLabels(mod) {
  return (
    (mod?.source === 'modrinth' && mod?.modrinthId) ||
    (mod?.source === 'curseforge' && mod?.curseforgeId)
  );
}

export function tagsForMode(mod, sideMode) {
  if (sideMode === 'manual') {
    return {
      sideMode: 'manual',
      side: mod?.manualSide ?? mod?.side ?? 'unknown',
      library: Boolean(mod?.manualLibrary),
      technical: Boolean(mod?.manualTechnical)
    };
  }
  return {
    sideMode: 'auto',
    side: mod?.providerSide ?? 'unknown',
    library: Boolean(mod?.providerLibrary),
    technical: Boolean(mod?.providerTechnical)
  };
}
