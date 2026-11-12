import { t } from './i18n';
import { $, errText, esc, formValues, modal, toast, withBusy, type ModalHandle } from './dom';
import { nairaInput } from './format';
import { store } from '../store';

/** One field in a form built with {@link formModal}. */
export interface Field {
  name: string;
  label: string;
  type?: 'text' | 'number' | 'date' | 'select' | 'textarea' | 'money' | 'checkbox' | 'hidden';
  options?: Array<[string, string]>;
  value?: string | number | boolean | null;
  required?: boolean;
  help?: string;
  placeholder?: string;
  maxlength?: number;
  step?: string;
  /** Put this field and the next one side by side. */
  pair?: boolean;
  list?: string[];
}

const cur = () => esc(store.settings.currency_symbol || '₦');

function fieldHtml(f: Field): string {
  const v = f.value ?? '';
  const req = f.required ? 'required' : '';
  const help = f.help ? `<small>${esc(f.help)}</small>` : '';
  const id = `dl-${f.name}-${Math.random().toString(36).slice(2, 7)}`;
  switch (f.type) {
    case 'hidden': return `<input type="hidden" name="${f.name}" value="${esc(v)}">`;
    case 'select': return `<label class="field"><span>${esc(f.label)}</span><select name="${f.name}" ${req}>${(f.options ?? []).map(([k, l]) => `<option value="${esc(k)}" ${String(v) === k ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select>${help}</label>`;
    case 'textarea': return `<label class="field"><span>${esc(f.label)}</span><textarea name="${f.name}" rows="2" maxlength="${f.maxlength ?? 1000}" ${req} placeholder="${esc(f.placeholder ?? '')}">${esc(v)}</textarea>${help}</label>`;
    case 'checkbox': return `<label class="check"><input type="checkbox" name="${f.name}" ${v ? 'checked' : ''}><span>${esc(f.label)}${f.help ? `<br><small class="muted">${esc(f.help)}</small>` : ''}</span></label>`;
    case 'money': return `<label class="field"><span>${esc(f.label)}</span><span class="input-affix"><span class="affix">${cur()}</span><input name="${f.name}" inputmode="decimal" ${req} value="${typeof v === 'number' ? nairaInput(v) : esc(v)}" placeholder="${esc(f.placeholder ?? '')}"></span>${help}</label>`;
    case 'number': return `<label class="field"><span>${esc(f.label)}</span><input name="${f.name}" inputmode="decimal" ${req} value="${esc(v)}" placeholder="${esc(f.placeholder ?? '')}">${help}</label>`;
    case 'date': return `<label class="field"><span>${esc(f.label)}</span><input type="date" name="${f.name}" ${req} value="${esc(v)}">${help}</label>`;
    default: return `<label class="field"><span>${esc(f.label)}</span><input name="${f.name}" maxlength="${f.maxlength ?? 200}" ${req} value="${esc(v)}" placeholder="${esc(f.placeholder ?? '')}" ${f.list ? `list="${id}"` : ''}>${f.list ? `<datalist id="${id}">${f.list.map((o) => `<option value="${esc(o)}">`).join('')}</datalist>` : ''}${help}</label>`;
  }
}

export function fieldsHtml(fields: Field[]): string {
  const out: string[] = [];
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i];
    if (f.pair && fields[i + 1]) { out.push(`<div class="grid-2">${fieldHtml(f)}${fieldHtml(fields[i + 1])}</div>`); i++; } else out.push(fieldHtml(f));
  }
  return out.join('');
}

/** Values of a form built from fields; checkboxes become '1' or ''. */
export function readForm(form: HTMLFormElement): Record<string, string> {
  const v = formValues(form);
  form.querySelectorAll<HTMLInputElement>('input[type=checkbox]').forEach((c) => { v[c.name] = c.checked ? '1' : ''; });
  return v;
}

/** A number from a form value, or null when left empty. */
export const num = (s: string | undefined): number | null => {
  const t = (s ?? '').trim().replace(/,/g, '');
  if (!t) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
};

/**
 * A form in a pop-up. `submit` throws to show its message under the form;
 * returning normally closes the pop-up and shows `done` as a toast.
 */
export function formModal(opts: { title: string; subtitle?: string; intro?: string; fields: Field[]; submitLabel?: string; done?: string; size?: 'sm' | 'md' | 'lg'; submit: (v: Record<string, string>) => Promise<void> }): ModalHandle {
  const m = modal({ title: opts.title, subtitle: opts.subtitle, size: opts.size ?? 'md', body: `<form class="stack" novalidate>
    ${opts.intro ? `<p class="muted small">${opts.intro}</p>` : ''}
    ${fieldsHtml(opts.fields)}
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(opts.submitLabel ?? t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const missing = opts.fields.find((f) => f.required && f.type !== 'checkbox' && !(form.elements.namedItem(f.name) as HTMLInputElement | null)?.value.trim());
    if (missing) { $('[data-err]', form)!.textContent = `${missing.label} is required.`; return; }
    void withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await opts.submit(readForm(form)); } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      if (opts.done) toast(opts.done);
      m.close();
    });
  });
  return m;
}

/** A plain table. Header names ending in '#' are right-aligned numbers. */
export function tableHtml(headers: string[], rows: string[][], empty = 'Nothing recorded yet.'): string {
  const isNum = headers.map((h) => h.endsWith('#'));
  return `<div class="table-scroll"><table class="table compact"><thead><tr>${headers.map((h, i) => `<th class="${isNum[i] ? 'num' : ''}">${esc(h.replace(/#$/, ''))}</th>`).join('')}</tr></thead><tbody>
    ${rows.length ? rows.map((r) => `<tr>${r.map((c, i) => `<td class="${isNum[i] ? 'num' : ''}">${c}</td>`).join('')}</tr>`).join('') : `<tr><td colspan="${headers.length}" class="muted">${esc(empty)}</td></tr>`}
  </tbody></table></div>`;
}

/** Tabs as chips. Each button carries data-tab. */
export const tabsHtml = (tabs: Array<[string, string]>, active: string) =>
  `<div class="chips tabs-chips" role="tablist">${tabs.map(([id, label]) => `<button type="button" role="tab" class="chip ${id === active ? 'is-on' : ''}" aria-selected="${id === active}" data-tab="${id}">${esc(label)}</button>`).join('')}</div>`;

export const pill = (text: string, tone: 'ok' | 'warn' | 'bad' | 'mute' | 'info' = 'mute') => `<span class="pill pill-${tone}">${esc(text)}</span>`;

export const reviewPill = (status: string) => status === 'approved' || status === 'confirmed' ? pill('Reviewed', 'ok') : pill('Not yet reviewed', 'warn');

/** "1st of this month" to today, as ISO dates. */
export function monthRange(): { from: string; to: string } {
  const d = new Date();
  const to = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  return { from: `${to.slice(0, 8)}01`, to };
}

export const fmtNum = (n: number | null | undefined, digits = 1) => (n === null || n === undefined ? '—' : Number(n).toLocaleString('en-NG', { maximumFractionDigits: digits }));
