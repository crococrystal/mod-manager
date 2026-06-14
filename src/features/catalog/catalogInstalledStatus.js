function normalizeKey(value) {
  return String(value ?? '')
    .trim()
    .toLowerCase();
}

function normalizeTitleKey(value) {
  return normalizeKey(value).replace(/[^a-z0-9]+/g, '');
}

function addKey(set, value) {
  const key = normalizeKey(value);
  if (key) set.add(key);
}

function stripFilenameDecorations(value) {
  return String(value ?? '')
    .replace(/^[^\p{L}\p{N}]+[\s\-_]*/u, '')
    .trim();
}

function stripQualifiers(value) {
  return String(value ?? '')
    .replace(/^(?:\[.*?\]|【.*?】|\(.*?\))\s*/g, '')
    .trim();
}

function stripVersionSuffixes(value) {
  let result = String(value ?? '').trim();
  result = result.replace(/[-_+]?(?:mc)?\d+\.\d+(?:\.\d+)?(?:[-_+][a-z0-9.+-]+)*$/i, '');
  result = result.replace(/[-_+]?[\d]+(?:\.[\d]+){1,3}(?:[-_+][a-z0-9.+-]+)*$/i, '');
  return result.trim().replace(/[-_+]+$/, '');
}

function slugKey(value) {
  return normalizeTitleKey(String(value ?? '').replace(/[-_\s]+/g, ''));
}

function normalizedMatchKey(value) {
  return normalizeKey(String(value ?? '').replace(/[-_\s]+/g, ' '));
}

function matchKeysForDisplayName(displayName) {
  const trimmed = stripFilenameDecorations(displayName);
  const clean = stripQualifiers(trimmed);
  const stripped = stripVersionSuffixes(trimmed);
  const nameKeys = new Set();
  const slugKeys = new Set();

  for (const candidate of [trimmed, clean, stripped]) {
    const normalized = normalizedMatchKey(candidate);
    if (normalized) nameKeys.add(normalized);
    const slug = slugKey(candidate);
    if (slug) slugKeys.add(slug);
  }

  const stem = normalizedMatchKey(stripped);
  if (stem) nameKeys.add(stem);
  const stemSlug = slugKey(stripped);
  if (stemSlug) slugKeys.add(stemSlug);

  return { nameKeys, slugKeys };
}

const PREFIX_OK_SUFFIXES = new Set([
  'edition',
  'jeiedition',
  'lite',
  'plus',
  'extra',
  'fix',
  'api',
  'lib'
]);

function normalizedPrefixMatch(jarKey, catalogKey) {
  if (jarKey.length < 8 || catalogKey.length < 8) return false;
  if (jarKey === catalogKey) return true;
  if (jarKey.startsWith(catalogKey)) return true;
  if (!catalogKey.startsWith(jarKey)) return false;
  const extra = catalogKey.slice(jarKey.length);
  if (!extra) return true;
  if (extra.length <= 12) return PREFIX_OK_SUFFIXES.has(extra);
  return false;
}

function providerProjectMatchesItem(displayName, item) {
  const { nameKeys, slugKeys } = matchKeysForDisplayName(displayName);
  const jarStem = normalizeTitleKey(
    stripVersionSuffixes(stripFilenameDecorations(displayName))
  );

  const title = item?.title ?? '';
  const titleNorm = normalizedMatchKey(title);
  const titleKey = normalizeTitleKey(title);
  if (titleNorm && nameKeys.has(titleNorm)) return true;
  if (titleKey && normalizedPrefixMatch(jarStem, titleKey)) return true;
  if (titleKey && [...nameKeys].some((key) => normalizeTitleKey(key) === titleKey)) {
    return true;
  }

  const slug = item?.slug ?? '';
  const slugNorm = normalizedMatchKey(slug);
  const slugSlug = slugKey(slug);
  if (slugNorm && nameKeys.has(slugNorm)) return true;
  if (slugSlug && slugKeys.has(slugSlug)) return true;
  if (slugSlug && normalizedPrefixMatch(jarStem, slugSlug)) return true;

  return false;
}

function modNameMatchesCatalogItem(mod, item) {
  const names = [mod?.displayName, mod?.base, mod?.filename?.replace(/\.jar$/i, '')].filter(
    Boolean
  );
  return names.some((name) => providerProjectMatchesItem(name, item));
}

export function catalogItemMatchKeys(item) {
  const keys = new Set();
  addKey(keys, item?.id);
  addKey(keys, item?.slug);
  addKey(keys, item?.title);
  addKey(keys, normalizeTitleKey(item?.title));
  addKey(keys, normalizeTitleKey(item?.slug));
  return keys;
}

export function modProviderMatchKeys(mod) {
  const modrinth = new Set();
  const curseforge = new Set();
  const any = new Set();

  const add = (target, value) => {
    addKey(target, value);
    addKey(any, value);
    addKey(any, normalizeTitleKey(value));
  };

  add(modrinth, mod?.modrinthId);
  add(curseforge, mod?.curseforgeId);

  const sourceUrl = String(mod?.sourceUrl ?? '');
  const curseforgeMatch = sourceUrl.match(/curseforge\.com\/minecraft\/mc-mods\/([^/?#]+)/i);
  if (curseforgeMatch) add(curseforge, curseforgeMatch[1]);
  const modrinthMatch = sourceUrl.match(/modrinth\.com\/mod\/([^/?#]+)/i);
  if (modrinthMatch) add(modrinth, modrinthMatch[1]);

  for (const name of [mod?.displayName, mod?.base, mod?.filename?.replace(/\.jar$/i, '')]) {
    if (!name) continue;
    const { nameKeys, slugKeys } = matchKeysForDisplayName(name);
    for (const key of nameKeys) any.add(key);
    for (const key of slugKeys) any.add(key);
  }

  return { modrinth, curseforge, any };
}

export function collectInstalledProjectIds(mods, catalogSource) {
  const ids = new Set();
  for (const mod of mods) {
    const keys = modProviderMatchKeys(mod);
    for (const key of keys.any) ids.add(key);
    if (catalogSource === 'modrinth') {
      for (const key of keys.modrinth) ids.add(key);
    }
    if (catalogSource === 'curseforge') {
      for (const key of keys.curseforge) ids.add(key);
    }
  }
  return ids;
}

function keysOverlap(left, right) {
  for (const key of left) {
    if (right.has(key)) return true;
  }
  return false;
}

function titlesMatch(catalogTitle, modTitle) {
  const left = normalizeKey(catalogTitle);
  const right = normalizeKey(modTitle);
  if (!left || !right) return false;
  if (left === right) return true;
  return normalizeTitleKey(catalogTitle) === normalizeTitleKey(modTitle);
}

export function modMatchesCatalogItem(mod, _catalogSource, item) {
  const itemKeys = catalogItemMatchKeys(item);
  const providerKeys = modProviderMatchKeys(mod);

  if (itemKeys.size && keysOverlap(itemKeys, providerKeys.any)) {
    return true;
  }

  if (modNameMatchesCatalogItem(mod, item)) {
    return true;
  }

  return titlesMatch(item?.title, mod?.displayName);
}

export function findInstalledModForCatalogItem(mods, catalogSource, item) {
  return mods.find((mod) => modMatchesCatalogItem(mod, catalogSource, item)) ?? null;
}

export function resolveCatalogInstalledIndicator({
  catalogSource,
  item,
  mods = [],
  installedProjectIds
}) {
  const matchedMod = findInstalledModForCatalogItem(mods, catalogSource, item);
  if (matchedMod) {
    return { show: true, source: matchedMod.source ?? catalogSource };
  }

  const itemKeys = catalogItemMatchKeys(item);
  if (itemKeys.size && keysOverlap(itemKeys, installedProjectIds ?? new Set())) {
    return { show: true, source: catalogSource };
  }

  return { show: false, source: null };
}

export function isCatalogItemInstalled({ catalogSource, item, mods = [], installedProjectIds }) {
  return resolveCatalogInstalledIndicator({ catalogSource, item, mods, installedProjectIds }).show;
}
