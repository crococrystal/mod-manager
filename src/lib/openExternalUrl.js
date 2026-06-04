import { openUrl } from '@tauri-apps/plugin-opener';

export async function openExternalUrl(url) {
  if (!url) return;
  try {
    await openUrl(url);
  } catch {
    window.open(url, '_blank', 'noopener,noreferrer');
  }
}

export function isExternalWebLink(rawHref, baseHref = window.location.href) {
  if (!rawHref) return false;
  const trimmed = rawHref.trim();
  if (!trimmed || trimmed.startsWith('#') || trimmed.startsWith('javascript:')) {
    return false;
  }
  if (trimmed.startsWith('mailto:') || trimmed.startsWith('tel:')) {
    return true;
  }
  try {
    const url = new URL(trimmed, baseHref);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return false;
    }
    return url.origin !== new URL(baseHref).origin;
  } catch {
    return false;
  }
}
