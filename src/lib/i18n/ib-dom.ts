import { IB_TEXT } from './ib-text';
import { lang } from './index';

/**
 * Shows approved Ibibio for interface words that the screens write directly
 * (buttons, tabs, headings, labels, menu tiles), while the language is Ibibio.
 *
 * Only an element's whole text is swapped, and only when it matches an entry
 * in IB_TEXT exactly, so sentences that are not translated yet stay in English.
 * The farmer's own data is never changed: table cells, form fields, and
 * anything marked data-no-translate are skipped. Switching language reloads
 * the app, so English comes back as it was.
 */
const SKIP = 'td, input, textarea, pre, code, [contenteditable], [data-no-translate], .flock-card b, .crop-card b, .proc-card b, .farm-tile b, .product-tile, .receipt-paper';
const ATTRS = ['placeholder', 'title', 'aria-label'] as const;

function translateText(node: Text) {
  const raw = node.nodeValue;
  if (!raw) return;
  const t = raw.trim();
  if (!t || t.length > 60) return;
  const ib = IB_TEXT[t];
  if (!ib) return;
  const el = node.parentElement;
  if (!el || el.closest(SKIP)) return;
  node.nodeValue = raw.replace(t, ib);
}

function translateAttrs(el: Element) {
  if (el.closest('[data-no-translate]')) return;
  for (const a of ATTRS) {
    const v = el.getAttribute(a);
    if (v) { const ib = IB_TEXT[v.trim()]; if (ib) el.setAttribute(a, ib); }
  }
}

function walk(root: Node) {
  if (root.nodeType === Node.TEXT_NODE) { translateText(root as Text); return; }
  if (!(root instanceof Element)) return;
  translateAttrs(root);
  const tw = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
  for (let n = tw.nextNode(); n; n = tw.nextNode()) {
    if (n.nodeType === Node.TEXT_NODE) translateText(n as Text);
    else translateAttrs(n as Element);
  }
}

let observer: MutationObserver | null = null;

/** Starts translating the screen when the language is Ibibio. Safe to call more than once. */
export function startIbibioScreens(root: HTMLElement = document.body) {
  if (lang() !== 'ib' || observer) return;
  walk(root);
  observer = new MutationObserver((records) => {
    if (lang() !== 'ib') return;
    for (const r of records) {
      if (r.type === 'characterData') translateText(r.target as Text);
      else if (r.type === 'attributes') translateAttrs(r.target as Element);
      else r.addedNodes.forEach(walk);
    }
  });
  observer.observe(root, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: [...ATTRS] });
}

export const _forTests = { walk };
