export function mergeDependencyKeys(...lists) {
  const set = new Set();
  for (const list of lists) {
    if (!Array.isArray(list)) continue;
    for (const key of list) {
      if (key) set.add(key);
    }
  }
  return [...set].sort((a, b) => a.localeCompare(b));
}

/** Собирает resolvedDependencies и обратные связи usedBy. */
export function normalizeModsGraph(mods) {
  for (const mod of mods) {
    mod.resolvedDependencies = mergeDependencyKeys(mod.dependencies, mod.jarDependencies);
  }
  return attachUsedBy(mods);
}

export function attachUsedBy(mods) {
  const buckets = new Map(mods.map((mod) => [mod.key, []]));

  for (const mod of mods) {
    const deps = mod.resolvedDependencies ?? mod.dependencies ?? [];
    for (const depKey of deps) {
      if (!depKey || depKey === mod.key || !buckets.has(depKey)) continue;
      const list = buckets.get(depKey);
      if (!list.includes(mod.key)) list.push(mod.key);
    }
  }

  const byKey = new Map(mods.map((mod) => [mod.key, mod]));

  for (const mod of mods) {
    mod.usedBy = (buckets.get(mod.key) ?? []).sort((a, b) =>
      (byKey.get(a)?.displayName ?? a).localeCompare(byKey.get(b)?.displayName ?? b)
    );
  }

  return mods;
}
