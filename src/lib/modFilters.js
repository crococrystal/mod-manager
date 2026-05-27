export function modMatchesWorkspaceFilters(mod, { query = '', filter = 'all' } = {}) {
  const needle = query.trim().toLowerCase();
  const matchesQuery =
    !needle ||
    `${mod.displayName} ${mod.filename} ${mod.description ?? ''}`.toLowerCase().includes(needle);
  const matchesFilter =
    filter === 'all' ||
    (filter === 'library' && mod.library) ||
    (filter === 'technical' && mod.technical) ||
    mod.side === filter;
  return matchesQuery && matchesFilter;
}
