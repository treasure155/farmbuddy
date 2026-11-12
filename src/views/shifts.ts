import { t } from '../lib/i18n';
import { auth, call } from '../api';
import { $, emptyState, errText, esc, modal, toast, withBusy } from '../lib/dom';
import { dateTimeLabel, isoDate, money, relativeDay, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { printHtml } from '../lib/receipt';
import { store } from '../store';
import type { ShiftReport, ShiftRow } from '../types';
import type { ViewCtx } from './shell';

const cur = () => esc(store.settings.currency_symbol || '₦');

/** Start of a cashier's shift: count the float (change) in the drawer. */
export function openShiftDialog(after: (s: ShiftReport) => void) {
  const m = modal({ title: 'Open shift', size: 'sm', body: `<form class="stack">
    <p class="muted">Count the change money already in the drawer before your first sale. This is your starting cash (float), not a sale. At the end of the shift you count it again and the app shows if anything is short or over.</p>
    <label class="field"><span>Starting cash in the drawer (change money, not a sale)</span><span class="input-affix"><span class="affix">${cur()}</span><input name="float" inputmode="decimal" value="0" required autofocus></span></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">Open shift</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    withBusy(form.querySelector('.btn-primary'), async () => {
      try {
        const s = await call<ShiftReport>('open_shift', { openingFloatKobo: toKobo((form.elements.namedItem('float') as HTMLInputElement).value) });
        toast('Shift opened'); m.close(); after(s);
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); }
    });
  });
}

function reportMarkup(r: ShiftReport): string {
  const row = (a: string, b: string, cls = '') => `<div class="slip-row ${cls}"><span>${esc(a)}</span><span>${b}</span></div>`;
  return `<article class="slip slip-80">
    <header class="slip-head"><h3>${esc(store.settings.business_name || 'My Farm')}</h3><p>Shift report (Z report)</p></header>
    <div class="slip-meta">${row('Cashier', esc(r.user_name))}${row('Opened', esc(dateTimeLabel(r.opened_at)))}${r.closed_at ? row('Closed', esc(dateTimeLabel(r.closed_at))) : row('Status', 'Still open')}</div>
    <div class="slip-totals">
      ${row('Receipts', String(r.receipts))}
      ${row('Sales value', money(r.sales_kobo))}
      ${r.by_method.map((m) => row(`Received by ${m.method}`, money(m.amount_kobo))).join('')}
    </div>
    <div class="slip-pay">
      ${row('Starting cash (float)', money(r.opening_float_kobo))}
      ${row('+ Cash taken', money(r.cash_sales_kobo))}
      ${r.cash_refunds_kobo ? row('− Cash refunds', money(r.cash_refunds_kobo)) : ''}
      ${r.cash_expenses_kobo ? row('− Cash expenses', money(r.cash_expenses_kobo)) : ''}
      ${row('Drawer should have', money(r.expected_cash_kobo), 'slip-grand')}
      ${r.status === 'closed' ? `${row('Counted', money(r.counted_cash_kobo))}${row(r.difference_kobo === 0 ? 'Balanced' : r.difference_kobo > 0 ? 'Over' : 'Short', money(Math.abs(r.difference_kobo)), r.difference_kobo < 0 ? 'slip-due' : '')}` : ''}
    </div>
    ${r.note ? `<p class="slip-note">${esc(r.note)}</p>` : ''}
  </article>`;
}

export function showShiftReport(r: ShiftReport) {
  const m = modal({ title: `Shift report: ${r.user_name}`, subtitle: relativeDay(r.opened_at), size: 'md', body: `<div class="stack">
    <div class="paper-wrap">${reportMarkup(r)}</div>
    <div class="form-actions"><button class="btn btn-primary" data-print>${icon('print', 16)} Print</button></div></div>` });
  $('[data-print]', m.el)!.addEventListener('click', () => printHtml(reportMarkup(r), '@page { size: 80mm auto; margin: 3mm 2mm; }'));
}

function closeShiftDialog(r: ShiftReport, after: (s: ShiftReport) => void) {
  const m = modal({ title: 'Close shift and count cash', size: 'md', body: `<form class="stack">
    <div class="figures figures-3"><div><span>Starting cash (float)</span><b>${money(r.opening_float_kobo)}</b></div><div><span>Cash taken</span><b>${money(r.cash_sales_kobo)}</b></div><div><span>Expected in drawer</span><b>${money(r.expected_cash_kobo)}</b></div></div>
    ${r.cash_refunds_kobo || r.cash_expenses_kobo ? `<p class="muted small">Expected cash already takes off ${money(r.cash_refunds_kobo)} in cash refunds and ${money(r.cash_expenses_kobo)} in cash expenses paid during the shift.</p>` : ''}
    <label class="field"><span>Cash you counted</span><span class="input-affix"><span class="affix">${cur()}</span><input name="counted" inputmode="decimal" required autofocus></span></label>
    <p class="diff-line" data-diff></p>
    <label class="field"><span>Note <em class="muted">(optional)</em></span><input name="note" maxlength="300" placeholder="e.g. ₦500 short, customer owes change"></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">Close shift</button></div></form>` });
  const form = m.el.querySelector('form')!;
  const counted = form.elements.namedItem('counted') as HTMLInputElement;
  counted.addEventListener('input', () => {
    const d = toKobo(counted.value) - r.expected_cash_kobo;
    $('[data-diff]', form)!.innerHTML = counted.value === '' ? '' : d === 0 ? `<span class="ok-text">${icon('check', 15)} Drawer balances</span>` : d > 0 ? `<span class="warn-text">Over by ${money(d)}</span>` : `<span class="bad-text">Short by ${money(-d)}</span>`;
  });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    withBusy(form.querySelector('.btn-primary'), async () => {
      try {
        const s = await call<ShiftReport>('close_shift', { input: { counted_cash_kobo: toKobo(counted.value), note: (form.elements.namedItem('note') as HTMLInputElement).value } });
        toast('Shift closed'); m.close(); after(s); showShiftReport(s);
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); }
    });
  });
}

export async function render(ctx: ViewCtx) {
  ctx.header(t('page.shifts.title', {}, 'Shifts & cash-up'), t('page.shifts.subtitle', {}, 'Open a shift with your float, close it with a cash count, and see if the drawer balances.'));
  const manager = auth.can('manager');
  const today = new Date();
  const range = { from: isoDate(new Date(today.getFullYear(), today.getMonth(), today.getDate() - 13)), to: isoDate(today) };
  ctx.main.innerHTML = `<section class="panel" data-current></section>
    ${manager ? `<section class="panel flush"><header class="panel-head pad"><div><h2>All shifts</h2><p class="muted small">Every cashier's shifts, with shortages in red.</p></div>
      <div class="toolbar tight"><label class="date-field"><span>From</span><input type="date" data-from value="${range.from}"></label><label class="date-field"><span>To</span><input type="date" data-to value="${range.to}"></label></div></header><div data-list></div></section>` : ''}`;
  const drawCurrent = async () => {
    const r = await call<ShiftReport | null>('current_shift');
    const box = $('[data-current]', ctx.main)!;
    box.innerHTML = r ? `<header class="panel-head"><div><h2>Your shift is open</h2><p class="muted small">Since ${esc(relativeDay(r.opened_at))}</p></div>
        <div class="row-actions"><button class="btn btn-ghost" data-report>${icon('receipt', 16)} X report</button><button class="btn btn-primary" data-close-shift>Close shift & count cash</button></div></header>
        <div class="figures"><div><span>Receipts</span><b>${r.receipts}</b></div><div><span>Sales</span><b>${money(r.sales_kobo)}</b></div><div><span>Starting cash (float)</span><b>${money(r.opening_float_kobo)}</b></div><div><span>Cash expected now</span><b>${money(r.expected_cash_kobo)}</b></div></div>`
      : `<div class="empty compact">${icon('cash', 26)}<h3>No open shift</h3><p>Open a shift at the start of work. ${store.flag('require_shift') ? 'Selling is blocked until you do.' : 'It keeps your cash drawer honest.'}</p><button class="btn btn-primary" data-open-shift>Open shift</button></div>`;
    box.onclick = (e) => {
      const t = e.target as HTMLElement;
      if (t.closest('[data-open-shift]')) openShiftDialog(() => { drawCurrent(); drawList(); });
      if (t.closest('[data-close-shift]') && r) closeShiftDialog(r, () => { drawCurrent(); drawList(); });
      if (t.closest('[data-report]') && r) showShiftReport(r);
    };
  };
  const drawList = async () => {
    if (!manager) return;
    const list = $('[data-list]', ctx.main)!;
    const rows = await call<ShiftRow[]>('list_shifts', { from: range.from, to: range.to }).catch((e) => { list.innerHTML = emptyState('Could not load shifts', errText(e)); return null; });
    if (!rows) return;
    list.innerHTML = rows.length ? `<div class="table-scroll"><table class="table table-click"><thead><tr><th>Cashier</th><th>Opened</th><th>Closed</th><th class="num">Float</th><th class="num">Expected</th><th class="num">Counted</th><th class="num">Difference</th></tr></thead><tbody>
      ${rows.map((s) => `<tr data-shift="${s.id}" tabindex="0"><td><b>${esc(s.user_name)}</b></td><td>${esc(relativeDay(s.opened_at))}</td><td>${s.closed_at ? esc(relativeDay(s.closed_at)) : '<span class="pill pill-info">Open</span>'}</td>
        <td class="num">${money(s.opening_float_kobo)}</td><td class="num">${s.status === 'closed' ? money(s.expected_cash_kobo) : '—'}</td><td class="num">${s.status === 'closed' ? money(s.counted_cash_kobo) : '—'}</td>
        <td class="num">${s.status !== 'closed' ? '—' : s.difference_kobo === 0 ? '<span class="ok-text">Balanced</span>' : `<b class="${s.difference_kobo < 0 ? 'bad-text' : 'warn-text'}">${s.difference_kobo < 0 ? 'Short ' : 'Over '}${money(Math.abs(s.difference_kobo))}</b>`}</td></tr>`).join('')}</tbody></table></div>`
      : emptyState('No shifts in this period', 'Shifts appear here once cashiers open them.');
    list.onclick = async (e) => {
      const tr = (e.target as HTMLElement).closest<HTMLElement>('[data-shift]');
      if (tr) showShiftReport(await call<ShiftReport>('shift_report', { id: Number(tr.dataset.shift) }));
    };
  };
  ctx.main.addEventListener('change', (e) => {
    const el = e.target as HTMLInputElement;
    if (el.matches('[data-from]')) range.from = el.value;
    if (el.matches('[data-to]')) range.to = el.value;
    if (el.matches('[data-from],[data-to]')) drawList();
  });
  await Promise.all([drawCurrent(), drawList()]);
}

