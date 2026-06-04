import { isExternalWebLink, openExternalUrl } from './openExternalUrl.js';

export function setupExternalLinks() {
  document.addEventListener(
    'click',
    (event) => {
      if (event.defaultPrevented || event.button !== 0) return;

      const anchor = event.target.closest?.('a[href]');
      if (!anchor || anchor.hasAttribute('download')) return;

      const href = anchor.getAttribute('href');
      if (!isExternalWebLink(href)) return;

      event.preventDefault();
      event.stopPropagation();

      const url =
        href.startsWith('mailto:') || href.startsWith('tel:')
          ? href
          : new URL(href, window.location.href).href;

      void openExternalUrl(url);
    },
    true
  );
}
