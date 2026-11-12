import { t } from '../lib/i18n';
import { call } from '../api';
import { exportCsv, nairaPlain } from '../lib/csv';
import { $, emptyState, errText, esc, toast } from '../lib/dom';
import { dateLabel, isoDate, money, qty } from '../lib/format';
import { icon } from '../lib/icons';
import { store } from '../store';
import type { NamedTotal, SalesReport } from '../types';
import { barChart } from './dashboard';
import type { ViewCtx } from './shell';

function presets(): Array<{ key: string; label: string; from: Date; to: Date }> {
  const t = new Date();
  const d = (y: number, m: number, day: number) => new Date(y, m, day);
  const y = t.getFullYear(), mo = t.getMonth(), da = t.getDate();
  const monday = d(y, mo, da - ((t.getDay() + 6) % 7));
  return [
    { key: 'today', label: 'Today', from: t, to: t },
    { key: 'yesterday', label: 'Yesterday', from: d(y, mo, da - 1), to: d(y, mo, da - 1) },
    { key: 'week', label: 'This week', from: monday, to: t },
    { key: 'month', label: 'This month', from: d(y, mo, 1), to: t },
    { key: 'last-month', label: 'Last month', from: d(y, mo - 1, 1), to: d(y, mo, 0) },
    { key: '90', label: 'Last 90 days', from: d(y, mo, da - 89), to: t },
  ];
}

let range = { from: isoDate(new Date(new Date().getFullYear(), new Date().getMonth(), 1)), to: isoDate(new Date()), key: 'month' };

export async function render(ctx: ViewCtx) {
  ctx.header(t('page.reports.title', {}, 'Reports'), t('page.reports.subtitle', {}, 'Sales, profit, payments and best sellers for any period.'), `<button class="btn btn-ghost" data-export>${icon('download', 16)} Export items</button>`);
  ctx.main.innerHTML = `<div class="toolbar">
      <div class="chips" role="group" aria-label="Period">${presets().map((p) => `<button class="chip ${range.key === p.key ? 'is-on' : ''}" data-preset="${p.key}">${p.label}</button>`).join('')}</div>
      <label class="date-field"><span>From</span><input type="date" data-from value="${range.from}"></label>
      <label class="date-field"><span>To</span><input type="date" data-to value="${range.to}" max="${isoDate(new Date())}"></label>
    </div><div data-report></div>`;
  const out = $('[data-report]', ctx.main)!;
  let report: SalesReport | null = null;

  const load = async () => {
    out.innerHTML = '<div class="loading"><span></span><span></span><span></span></div>';
    try { report = await call<SalesReport>('sales_report', { from: range.from, to: range.to }); }
    catch (e) { out.innerHTML = emptyState('Report could not load', errText(e)); return; }
    const r = report;
    const margin = r.net_kobo - r.tax_kobo > 0 ? Math.round((r.profit_kobo / (r.net_kobo - r.tax_kobo)) * 100) : 0;
    const share = (rows: NamedTotal[], title: string, sub: string) => {
      const total = rows.reduce((a, x) => a + x.total_kobo, 0) || 1;
      return `<section class="panel"><header class="panel-head"><div><h2>${title}</h2><p class="muted small">${sub}</p></div></header>
        ${rows.length ? `<ul class="share-list">${rows.map((x) => `<li><div class="share-top"><span>${esc(x.name)}</span><b>${money(x.total_kobo)}</b></div><div class="share-bar"><span style="width:${Math.max(1, (x.total_kobo / total) * 100)}%"></span></div><small class="muted">${x.count} ${x.count === 1 ? 'entry' : 'entries'} · ${Math.round((x.total_kobo / total) * 100)}%</small></li>`).join('')}</ul>` : '<p class="muted pad">Nothing in this period.</p>'}</section>`;
    };
    out.innerHTML = `
      <p class="period-line">${esc(dateLabel(r.from))}${r.from !== r.to ? ` to ${esc(dateLabel(r.to))}` : ''}</p>
      <section class="ledger ledger-5" aria-label="Summary">
        <div class="ledger-cell ledger-lead"><span class="ledger-label">Sales</span><strong class="figure">${money(r.net_kobo)}</strong><span class="ledger-note">${r.receipts} receipt${r.receipts === 1 ? '' : 's'}${r.voided ? ` · ${r.voided} voided` : ''}${r.returns_kobo ? ` · ${money(r.returns_kobo, { whole: true })} returned` : ''}</span></div>
        <div class="ledger-cell"><span class="ledger-label">Gross profit</span><strong class="figure ${r.profit_kobo < 0 ? 'is-bad' : ''}">${money(r.profit_kobo)}</strong><span class="ledger-note">${margin}% margin after ${money(r.cost_kobo, { whole: true })} cost</span></div>
        <div class="ledger-cell"><span class="ledger-label">Net profit</span><strong class="figure ${r.net_profit_kobo < 0 ? 'is-bad' : ''}">${money(r.net_profit_kobo)}</strong><span class="ledger-note">After ${money(r.expenses_kobo, { whole: true })} in expenses</span></div>
        <div class="ledger-cell"><span class="ledger-label">Received</span><strong class="figure">${money(r.collected_kobo)}</strong><span class="ledger-note">${r.refunds_kobo ? `After ${money(r.refunds_kobo, { whole: true })} refunded · ` : ''}${r.discount_kobo ? `${money(r.discount_kobo, { whole: true })} given as discount` : 'No discounts given'}</span></div>
        <div class="ledger-cell"><span class="ledger-label">Still owed</span><strong class="figure ${r.outstanding_kobo ? 'is-warn' : ''}">${money(r.outstanding_kobo)}</strong><span class="ledger-note">${r.tax_kobo ? `${esc(store.settings.tax_label || 'Tax')} collected ${money(r.tax_kobo, { whole: true })}` : 'From sales in this period'}</span></div>
      </section>
      ${r.by_day.length > 1 ? `<section class="panel"><header class="panel-head"><div><h2>Sales by day</h2></div></header>${barChart(r.by_day, { height: 200 })}</section>` : ''}
      <div class="report-grid">
        ${share(r.by_method, 'Money received by method', 'Counted on the day it was paid. Use this to balance the cash.')}
        ${share(r.by_category, 'Sales by category', 'Where your money comes from')}
        ${share(r.by_cashier, 'Sales by staff', 'Receipts issued per person')}
        ${r.by_expense.length ? share(r.by_expense, 'Expenses by category', 'What the business spent in this period') : ''}
      </div>
      <section class="panel flush">
        <header class="panel-head pad"><div><h2>${esc(store.label('items'))} sold</h2><p class="muted small">Sorted by sales. Profit uses the cost price at the time of each sale.</p></div></header>
        ${r.items.length ? `<div class="table-scroll"><table class="table">
          <thead><tr><th>${esc(store.label('item'))}</th><th class="num">Quantity</th><th class="num">Sales</th><th class="num">Cost</th><th class="num">Profit</th></tr></thead>
          <tbody>${r.items.map((i) => `<tr><td>${esc(i.name)}</td><td class="num">${qty(i.quantity)} <small class="muted">${esc(i.unit)}</small></td><td class="num">${money(i.revenue_kobo)}</td><td class="num">${i.cost_kobo ? money(i.cost_kobo) : '<span class="muted">—</span>'}</td><td class="num"><b class="${i.profit_kobo < 0 ? 'bad-text' : ''}">${money(i.profit_kobo)}</b></td></tr>`).join('')}</tbody></table></div>`
          : emptyState('No sales in this period', 'Pick another date range above.')}
      </section>`;
  };
  await load();

  ctx.main.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-preset]');
    if (!b) return;
    const p = presets().find((x) => x.key === b.dataset.preset)!;
    range = { from: isoDate(p.from), to: isoDate(p.to), key: p.key };
    ($('[data-from]', ctx.main) as HTMLInputElement).value = range.from;
    ($('[data-to]', ctx.main) as HTMLInputElement).value = range.to;
    ctx.main.querySelectorAll('[data-preset]').forEach((c) => c.classList.toggle('is-on', c === b));
    load();
  });
  ctx.main.addEventListener('change', (e) => {
    const el = e.target as HTMLInputElement;
    if (!el.matches('[data-from],[data-to]')) return;
    range = { from: ($('[data-from]', ctx.main) as HTMLInputElement).value, to: ($('[data-to]', ctx.main) as HTMLInputElement).value, key: '' };
    ctx.main.querySelectorAll('[data-preset]').forEach((c) => c.classList.remove('is-on'));
    if (range.from && range.to) load();
  });
  $('[data-export]')!.addEventListener('click', () => {
    if (!report) return toast('Load a report first', 'error');
    exportCsv(`sales-${range.from}-to-${range.to}.csv`, ['Item', 'Unit', 'Quantity', 'Sales', 'Cost', 'Profit'],
      report.items.map((i) => [i.name, i.unit, i.quantity, nairaPlain(i.revenue_kobo), nairaPlain(i.cost_kobo), nairaPlain(i.profit_kobo)]));
  });
}
