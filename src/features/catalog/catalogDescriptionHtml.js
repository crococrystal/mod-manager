import { marked } from 'marked';

const FORBIDDEN_TAGS = new Set([
  'script',
  'style',
  'link',
  'iframe',
  'object',
  'embed',
  'form',
  'input',
  'button',
  'meta'
]);

marked.setOptions({
  gfm: true,
  breaks: true
});

function isSafeUrl(value, attribute) {
  if (!value?.trim()) return attribute === 'src' ? null : '#';
  try {
    const parsed = new URL(value.trim(), 'https://modrinth.com');
    if (parsed.protocol === 'https:' || parsed.protocol === 'http:') {
      return parsed.href;
    }
  } catch {
    return null;
  }
  return null;
}

export function sanitizeCatalogHtml(html) {
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const walk = (node) => {
    [...node.children].forEach((child) => {
      const tag = child.tagName?.toLowerCase();
      if (!tag) return;
      if (FORBIDDEN_TAGS.has(tag)) {
        child.remove();
        return;
      }
      [...child.attributes].forEach((attr) => {
        const name = attr.name.toLowerCase();
        if (name.startsWith('on')) {
          child.removeAttribute(attr.name);
          return;
        }
        if (name === 'style' || name === 'class' || name === 'id') {
          child.removeAttribute(attr.name);
          return;
        }
        if (name === 'href' || name === 'src') {
          const safe = isSafeUrl(attr.value, name);
          if (safe) child.setAttribute(attr.name, safe);
          else child.removeAttribute(attr.name);
        }
      });
      if (tag === 'a' && child.hasAttribute('href')) {
        child.setAttribute('target', '_blank');
        child.setAttribute('rel', 'noopener noreferrer');
      }
      walk(child);
    });
  };
  walk(doc.body);
  doc.querySelectorAll('table').forEach((table) => {
    if (table.parentElement?.classList?.contains('catalogDescriptionTableWrap')) return;
    const wrap = doc.createElement('div');
    wrap.className = 'catalogDescriptionTableWrap';
    table.parentNode?.insertBefore(wrap, table);
    wrap.appendChild(table);
  });
  return doc.body.innerHTML;
}

export function descriptionToHtml(description) {
  const text = description?.trim();
  if (!text) return '';
  const rendered = marked.parse(text, { async: false });
  return sanitizeCatalogHtml(typeof rendered === 'string' ? rendered : '');
}
