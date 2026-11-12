import { icon } from './icons';

export const esc = (s: unknown) =>
  String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);

export const $ = <T extends Element = HTMLElement>(sel: string, root: ParentNode = document) => root.querySelector<T>(sel);
export const $$ = <T extends Element = HTMLElement>(sel: string, root: ParentNode = document) => Array.from(root.querySelectorAll<T>(sel));

export const errText = (e: unknown) => (e instanceof Error ? e.message : String(e)).replace(/^Error:\s*/, '');

export function debounce<A extends unknown[]>(fn: (...a: A) => void, ms = 200) {
  let t: number | undefined;
  return (...a: A) => { window.clearTimeout(t); t = window.setTimeout(() => fn(...a), ms); };
}

export function formValues(form: HTMLFormElement): Record<string, string> {
  const out: Record<string, string> = {};
  new FormData(form).forEach((v, k) => { out[k] = typeof v === 'string' ? v : ''; });
  // Unchecked checkboxes are absent from FormData; record them explicitly.
  $$<HTMLInputElement>('input[type=checkbox][name]', form).forEach((cb) => { out[cb.name] = cb.checked ? '1' : '0'; });
  return out;
}

// ---------------------------------------------------------------- toasts

export function toast(message: string, tone: 'ok' | 'error' | 'info' = 'ok') {
  let host = $('#toasts');
  if (!host) { host = document.createElement('div'); host.id = 'toasts'; host.setAttribute('role', 'status'); document.body.append(host); }
  const node = document.createElement('div');
  node.className = `toast toast-${tone}`;
  node.innerHTML = `${icon(tone === 'error' ? 'alert' : 'check', 16)}<span>${esc(message)}</span>`;
  host.append(node);
  window.setTimeout(() => node.classList.add('leaving'), tone === 'error' ? 6500 : 3800);
  window.setTimeout(() => node.remove(), tone === 'error' ? 7000 : 4300);
}

// ---------------------------------------------------------------- busy buttons

export async function withBusy<T>(button: HTMLButtonElement | null, task: () => Promise<T>): Promise<T | undefined> {
  if (button?.disabled) return undefined;
  const original = button?.innerHTML;
  if (button) { button.disabled = true; button.classList.add('is-busy'); }
  try { return await task(); }
  catch (e) { toast(errText(e), 'error'); return undefined; }
  finally { if (button) { button.disabled = false; button.classList.remove('is-busy'); if (original !== undefined) button.innerHTML = original; } }
}

// ---------------------------------------------------------------- modals

export interface ModalHandle { el: HTMLElement; close: () => void }
let openModals = 0;

export function modal(opts: { title: string; body: string; size?: 'sm' | 'md' | 'lg' | 'drawer'; onClose?: () => void; subtitle?: string }): ModalHandle {
  const lastFocus = document.activeElement as HTMLElement | null;
  const wrap = document.createElement('div');
  wrap.className = `modal-wrap ${opts.size === 'drawer' ? 'is-drawer' : ''}`;
  const id = `m${Date.now()}`;
  wrap.innerHTML = `<div class="modal-scrim" data-close></div>
    <section class="modal modal-${opts.size ?? 'md'}" role="dialog" aria-modal="true" aria-labelledby="${id}">
      <header class="modal-head"><div><h2 id="${id}">${esc(opts.title)}</h2>${opts.subtitle ? `<p class="muted small">${esc(opts.subtitle)}</p>` : ''}</div>
      <button type="button" class="icon-btn" data-close aria-label="Close">${icon('close', 18)}</button></header>
      <div class="modal-body">${opts.body}</div>
    </section>`;
  document.body.append(wrap);
  openModals++;
  document.body.classList.add('has-modal');
  const close = () => {
    if (!wrap.isConnected) return;
    wrap.classList.add('closing');
    document.removeEventListener('keydown', onKey, true);
    window.setTimeout(() => { wrap.remove(); openModals--; if (!openModals) document.body.classList.remove('has-modal'); lastFocus?.focus?.(); }, 140);
    opts.onClose?.();
  };
  const onKey = (e: KeyboardEvent) => {
    const stack = $$('.modal-wrap');
    if (e.key === 'Escape' && wrap === stack[stack.length - 1]) { e.stopPropagation(); close(); }
    if (e.key === 'Tab') trapFocus(wrap, e);
  };
  document.addEventListener('keydown', onKey, true);
  wrap.addEventListener('click', (e) => { if ((e.target as HTMLElement).closest('[data-close]')) close(); });
  window.setTimeout(() => {
    const target = $<HTMLElement>('[autofocus], input:not([type=hidden]):not([disabled]), select, textarea, button.btn-primary', wrap.querySelector('.modal-body')!) ?? $<HTMLElement>('.icon-btn', wrap);
    target?.focus();
  }, 30);
  return { el: wrap.querySelector('.modal') as HTMLElement, close };
}

function trapFocus(root: HTMLElement, e: KeyboardEvent) {
  const f = $$<HTMLElement>('a[href], button:not([disabled]), input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])', root).filter((x) => x.offsetParent !== null);
  if (!f.length) return;
  const first = f[0], last = f[f.length - 1];
  if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
  else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
}

export function confirmDialog(opts: { title: string; message: string; confirm: string; danger?: boolean; reason?: { label: string; placeholder?: string; required?: boolean } }): Promise<string | null> {
  return new Promise((resolve) => {
    let done = false;
    const m = modal({
      title: opts.title, size: 'sm',
      body: `<form class="stack" data-confirm>
        <p class="lead-text">${esc(opts.message)}</p>
        ${opts.reason ? `<label class="field"><span>${esc(opts.reason.label)}</span><input name="reason" maxlength="200" ${opts.reason.required ? 'required minlength="3"' : ''} placeholder="${esc(opts.reason.placeholder ?? '')}"></label>` : ''}
        <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>Cancel</button><button class="btn ${opts.danger ? 'btn-danger' : 'btn-primary'}" type="submit">${esc(opts.confirm)}</button></div>
      </form>`,
      onClose: () => { if (!done) resolve(null); },
    });
    m.el.querySelector('form')!.addEventListener('submit', (e) => {
      e.preventDefault();
      done = true;
      resolve(((e.currentTarget as HTMLFormElement).elements.namedItem('reason') as HTMLInputElement | null)?.value.trim() ?? '');
      m.close();
    });
  });
}

export const emptyState = (title: string, text: string, action = '') =>
  `<div class="empty"><div class="empty-mark">${icon('box', 26)}</div><h3>${esc(title)}</h3><p>${esc(text)}</p>${action}</div>`;
