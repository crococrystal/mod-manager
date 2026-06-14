function normalizeKey(value) {
  return String(value ?? '')
    .trim()
    .toLowerCase();
}

function addKey(set, value) {
  const key = normalizeKey(value);
  if (key) set.add(key);
}

export function catalogItemMatchKeys(item) {
  const keys = new Set();
  addKey(keys, item?.id);
  addKey(keys, item?.slug);
  return keys;
}

export function modProviderMatchKeys(mod) {
  const modrinth = new Set();
  const curseforge = new Set();
  const any = new Set();

  const add = (target, value) => {
    addKey(target, value);
    addKey(any, value);
  };

  add(modrinth, mod?.modrinthId);
  add(curseforge, mod?.curseforgeId);

  const sourceUrl = String(mod?.sourceUrl ?? '');
  const curseforgeMatch = sourceUrl.match(/curseforge\.com\/minecraft\/mc-mods\/([^/?#]+)/i);
  if (curseforgeMatch) add(curseforge, curseforgeMatch[1]);
  const modrinthMatch = sourceUrl.match(/modrinth\.com\/mod\/([^/?#]+)/i);
  if (modrinthMatch) add(modrinth, modrinthMatch[1]);

  return { modrinth, curseforge, any };
}

function keysOverlap(left, right) {
  for (const key of left) {
    if (right.has(key)) return true;
  }
  return false;
}

export function modMatchesCatalogItem(mod, catalogSource, item) {
  const itemKeys = catalogItemMatchKeys(item);
  if (!itemKeys.size) return false;

  const providerKeys = modProviderMatchKeys(mod);
  const sourceKeys =
    catalogSource === 'curseforge'
      ? providerKeys.curseforge
      : catalogSource === 'modrinth'
      ? providerKeys.modrinth
      : providerKeys.any;

  if (keysOverlap(itemKeys, sourceKeys) || keysOverlap(itemKeys, providerKeys.any)) {
    return true;
  }

  const itemTitle = normalizeKey(item?.title);
  const modTitle = normalizeKey(mod?.displayName);
  return Boolean(itemTitle && modTitle && itemTitle === modTitle);
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
  const sameProvider = installedProjectIds?.has(String(item?.id));
  const matchedMod = findInstalledModForCatalogItem(mods, catalogSource, item);

  if (!sameProvider && !matchedMod) {
    return { show: false, source: null };
  }

  const installSource = matchedMod?.source ?? catalogSource;
  return { show: true, source: installSource };
}

export function isCatalogItemInstalled({ catalogSource, item, mods = [], installedProjectIds }) {
  return resolveCatalogInstalledIndicator({ catalogSource, item, mods, installedProjectIds }).show;
}
