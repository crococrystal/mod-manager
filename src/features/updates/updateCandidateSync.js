import { readProviderVersionsCache } from '../../lib/providerVersionsCache.js';
import { isInstalledVersion, projectIdFor } from '../mods/versionListUtils.js';

const SUMMARY_ARROW = ' → ';

export function modNeedsUpdate(mod, cacheScope) {
  if (!mod) return false;

  const projectId = projectIdFor(mod);
  if (!projectId) return true;

  const cached = readProviderVersionsCache(cacheScope, mod.source, projectId);
  if (!cached?.versions?.length) return true;

  const latest = cached.versions[0];
  return !isInstalledVersion(mod, latest);
}

function latestLabelFromCandidateSummary(summary) {
  if (!summary) return null;
  const index = summary.lastIndexOf(SUMMARY_ARROW);
  if (index < 0) return null;
  const label = summary.slice(index + SUMMARY_ARROW.length).trim();
  return label || null;
}

export function buildUpdateCandidateSummary(mod, cacheScope, fallbackSummary) {
  if (!mod) return fallbackSummary ?? null;

  const current = mod.installedVersion?.trim() || '—';
  const projectId = projectIdFor(mod);
  let latestLabel = null;

  if (projectId) {
    const cached = readProviderVersionsCache(cacheScope, mod.source, projectId);
    const latest = cached?.versions?.[0];
    if (latest) {
      latestLabel = latest.versionNumber ?? latest.name ?? null;
    }
  }

  if (!latestLabel) {
    latestLabel = latestLabelFromCandidateSummary(fallbackSummary);
  }
  if (!latestLabel) return fallbackSummary ?? null;

  return `${current}${SUMMARY_ARROW}${latestLabel}`;
}

export function refreshUpdateCandidateSummary(candidate, mod, cacheScope) {
  const summary = buildUpdateCandidateSummary(mod, cacheScope, candidate.summary);
  if (!summary || summary === candidate.summary) return candidate;
  return { ...candidate, summary };
}

export function syncUpdateCandidatesList(candidates, modsByKey, cacheScope) {
  const seen = new Set();
  return candidates.flatMap((candidate) => {
    const key = candidate.key ?? candidate.id;
    if (!key || seen.has(key)) return [];
    seen.add(key);
    const mod = modsByKey.get(key);
    if (!modNeedsUpdate(mod, cacheScope)) return [];
    return [refreshUpdateCandidateSummary(candidate, mod, cacheScope)];
  });
}

export function filterStaleUpdateCandidates(candidates, modsByKey, cacheScope) {
  return syncUpdateCandidatesList(candidates, modsByKey, cacheScope);
}

export function staleUpdateCandidateKeys(candidates, modsByKey, cacheScope) {
  const next = new Set(
    filterStaleUpdateCandidates(candidates, modsByKey, cacheScope).map(
      (item) => item.key ?? item.id
    )
  );
  return candidates
    .map((item) => item.key ?? item.id)
    .filter((key) => key && !next.has(key));
}
