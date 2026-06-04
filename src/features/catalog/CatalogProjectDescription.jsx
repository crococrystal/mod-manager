import { useLayoutEffect, useMemo, useRef } from 'react';
import 'github-markdown-css/github-markdown-dark.css';
import { descriptionToHtml } from './catalogDescriptionHtml.js';

function labelBrokenImage(img) {
  if (!img || img.dataset.catalogBrokenImage === '1') return;
  img.dataset.catalogBrokenImage = '1';

  const label = img.getAttribute('alt') || img.getAttribute('title') || '';
  if (label.trim()) {
    const fallback = document.createElement('span');
    fallback.className = 'catalogDescriptionBrokenImage';
    fallback.textContent = label.trim();
    img.replaceWith(fallback);
  } else {
    img.remove();
  }
}

function showLoadedImage(img) {
  if (!img || img.dataset.catalogBrokenImage === '1') return;
  img.classList.add('catalogDescriptionImageLoaded');
}

function verifyImage(img) {
  if (!img || img.dataset.catalogBrokenImage === '1') return;
  if (!img.getAttribute('src')) {
    labelBrokenImage(img);
    return;
  }

  if (typeof img.decode === 'function') {
    img.decode()
      .then(() => showLoadedImage(img))
      .catch(() => labelBrokenImage(img));
    return;
  }

  if (img.complete) {
    if (img.naturalWidth > 0) showLoadedImage(img);
    else labelBrokenImage(img);
  }
}

export function CatalogProjectDescription({ description }) {
  const rootRef = useRef(null);
  const html = useMemo(() => descriptionToHtml(description), [description]);

  useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root) return undefined;

    const verifyAllImages = () => {
      root.querySelectorAll('img').forEach((img) => verifyImage(img));
    };
    const handleImageLoad = (event) => {
      if (event.target instanceof HTMLImageElement) verifyImage(event.target);
    };
    const handleImageError = (event) => {
      if (event.target instanceof HTMLImageElement) labelBrokenImage(event.target);
    };
    const observer = new MutationObserver(verifyAllImages);

    root.addEventListener('load', handleImageLoad, true);
    root.addEventListener('error', handleImageError, true);
    observer.observe(root, { childList: true, subtree: true });
    verifyAllImages();

    return () => {
      root.removeEventListener('load', handleImageLoad, true);
      root.removeEventListener('error', handleImageError, true);
      observer.disconnect();
    };
  }, [html]);

  if (!html) return null;

  return (
    <div
      ref={rootRef}
      className="markdown-body catalogDescription"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
