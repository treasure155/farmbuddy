import { auth, call } from '../api';
import { $, esc } from '../lib/dom';
import { greeting, money, moneyShort, parseLocal, qty, relativeDay } from '../lib/format';
import { icon } from '../lib/icons';
import { store } from '../store';
import type { Dashboard, ReceiptSummary } from '../types';
import type { ViewCtx } from './shell';
import { statusPill } from './receipts';

const dayName = new Intl.DateTimeFormat('en-GB', { weekday: 'short' });
const longDate = new Intl.DateTimeFormat('en-GB', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });

export function barChart(points: Array<{ date: string; total_kobo: number; receipts: number }>, opts: { height?: number; labelEvery?: number } = {}): string {
  const h = opts.height ?? 180;
  const max = Math.max(1, ...points.map((p) => p.total_kobo));
  const n = points.length;
  const gap = n > 20 ? 2 : 10;
  const w = 100 / n;
  const every = opts.labelEvery ?? (n > 14 ? Math.ceil(n / 8) : 1);
  const today = points[n - 1]?.date;
  return `<figure class="bars" style="--h:${h}px">
    <div class="bars-plot" role="img" aria-label="Sales per day">
      ${points.map((p) => {
        const pct = (p.total_kobo / max) * 100;
        const d = parseLocal(p.date);
        return `<div class="bar-col" style="width:${w}%;padding:0 ${gap / 2}px" title="${esc(d.toDateString())}: ${esc(money(p.total_kobo))} from ${p.receipts} receipt${p.receipts === 1 ? '' : 's'}">
          ${p.total_kobo ? `<span class="bar-value">${esc(moneyShort(p.total_kobo))}</span>` : ''}
          <span class="bar ${p.date === today && n <= 7 ? 'is-today' : ''}" style="height:${Math.max(p.total_kobo ? 2 : 0, pct)}%"></span>
        </div>`;
      }).join('')}
    </div>
    <div class="bars-axis">${points.map((p, i) => `<span style="width:${w}%">${i % every === 0 || i === n - 1 ? esc(n <= 7 ? dayName.format(parseLocal(p.date)) : `${parseLocal(p.date).getDate()}/${parseLocal(p.date).getMonth() + 1}`) : ''}</span>`).join('')}</div>
  </figure>`;
}

export async function render(ctx: ViewCtx) {
  const s = auth.session!;
  const firstName = (s.full_name || s.username).split(/\s+/)[0];
  ctx.header('Sales overview', longDate.format(new Date()));
  const [d, recent, security] = await Promise.all([
    call<Dashboard>('dashboard'),
    call<ReceiptSummary[]>('list_receipts', { filter: {} }),
    auth.can('owner') ? call<{ key_set: boolean; questions: string[] }>('security_status').catch(() => null) : Promise.resolve(null),
  ]);
  const needsRecovery = !!security && (!security.key_set || security.questions.length !== 3);
  const mine = d.scope === 'mine';
  const diff = d.today_sales_kobo - d.yesterday_sales_kobo;
  const compare = d.yesterday_sales_kobo === 0 && d.today_sales_kobo === 0 ? 'No sales yet today'
    : d.yesterday_sales_kobo === 0 ? 'Nothing sold yesterday'
    : `${diff >= 0 ? 'Up' : 'Down'} ${money(Math.abs(diff), { whole: true })} on yesterday`;
  const collection = store.flag('track_collection');
  const week = d.last_7_days.reduce((a, p) => a + p.total_kobo, 0);

  ctx.main.innerHTML = `
    ${!mine && (d.expired_count || d.expiring_count) ? `<div class="callout callout-warn"><div><b>${d.expired_count ? `${d.expired_count} batch${d.expired_count === 1 ? ' has' : 'es have'} expired` : `${d.expiring_count} batch${d.expiring_count === 1 ? '' : 'es'} expiring soon`}</b><span>${d.expired_count && d.expiring_count ? `${d.expiring_count} more expire within ${esc(store.settings.expiry_warn_days || '30')} days. ` : ''}Sell, discount or remove them before they become a loss.</span></div><button class="btn btn-primary btn-sm" data-go="inventory" data-tab="expiry">See batches</button></div>` : ''}
    ${needsRecovery ? `<div class="callout callout-warn"><div><b>Set up account recovery</b><span>Without a recovery key and security questions, a forgotten owner password cannot be reset.</span></div><button class="btn btn-primary btn-sm" data-go="settings" data-section="security">Set it up</button></div>` : ''}
    <section class="ledger" aria-label="Today at a glance">
      <div class="ledger-cell ledger-lead">
        <span class="ledger-label">${mine ? 'Your sales today' : 'Sales today'}</span>
        <strong class="figure">${money(d.today_sales_kobo)}</strong>
        <span class="ledger-note">${d.today_receipts} receipt${d.today_receipts === 1 ? '' : 's'} · ${esc(compare)}${d.today_returns_kobo ? ` · ${money(d.today_returns_kobo, { whole: true })} returned` : ''}</span>
      </div>
      <div class="ledger-cell">
        <span class="ledger-label">Money received today</span>
        <strong class="figure">${money(d.today_collected_kobo)}</strong>
        <span class="ledger-note">${!mine && d.today_expenses_kobo ? `${money(d.today_expenses_kobo, { whole: true })} spent on expenses today` : 'Includes old balances paid today'}</span>
      </div>
      <button class="ledger-cell ledger-link" data-go="receipts" data-status="owing">
        <span class="ledger-label">Owed by ${esc(store.label('customer').toLowerCase())}s</span>
        <strong class="figure ${d.outstanding_kobo ? 'is-warn' : ''}">${money(d.outstanding_kobo)}</strong>
        <span class="ledger-note">${d.owing_receipts} unpaid or part-paid receipt${d.owing_receipts === 1 ? '' : 's'}</span>
      </button>
      ${collection ? `<button class="ledger-cell ledger-link" data-go="receipts" data-fulfilment="ready">
        <span class="ledger-label">Ready for pickup</span>
        <strong class="figure">${d.ready_collections}</strong>
        <span class="ledger-note">${d.pending_collections} still in progress</span>
      </button>` : `<div class="ledger-cell">
        <span class="ledger-label">Running low</span>
        <strong class="figure ${d.low_stock_count ? 'is-warn' : ''}">${d.low_stock_count}</strong>
        <span class="ledger-note">of ${d.product_count} ${esc(store.label('items').toLowerCase())} for sale</span>
      </div>`}
    </section>

    <div class="dash-grid">
      <section class="panel">
        <header class="panel-head"><div><h2>Last 7 days</h2><p class="muted small">${money(week)} in total${mine ? ' from your receipts' : ''}</p></div></header>
        ${week ? barChart(d.last_7_days) : `<div class="empty compact"><p>Sales will show here once you start selling.</p><button class="btn btn-ghost" data-go="pos">Sell produce</button></div>`}
      </section>
      <section class="panel">
        <header class="panel-head"><div><h2>Running low</h2><p class="muted small">At or below the reorder level</p></div>${auth.can('manager') ? `<button class="btn btn-ghost btn-sm" data-go="inventory" data-low="1">See all</button>` : ''}</header>
        ${d.low_stock.length ? `<ul class="low-list">${d.low_stock.map((l) => `
          <li><span class="low-name">${esc(l.name)}</span><span class="low-qty ${l.stock_qty <= 0 ? 'is-out' : ''}">${l.stock_qty <= 0 ? 'Out of stock' : `${qty(l.stock_qty)} ${esc(l.unit)} left`}</span></li>`).join('')}</ul>`
          : `<div class="empty compact">${icon('check', 22)}<p>Stock levels look healthy.</p></div>`}
      </section>
    </div>

    <section class="panel">
      <header class="panel-head"><div><h2>Recent receipts</h2></div><button class="btn btn-ghost btn-sm" data-go="receipts">All receipts</button></header>
      ${recent.length ? `<table class="table table-click"><thead><tr><th>Receipt</th><th>${esc(store.label('customer'))}</th><th>Served by</th><th>When</th><th>Status</th><th class="num">Total</th></tr></thead><tbody>
        ${recent.slice(0, 6).map((r) => `<tr data-receipt="${r.id}" tabindex="0"><td class="mono-ish">${esc(r.receipt_no)}</td><td>${esc(r.customer_name || 'Walk-in')}</td><td>${esc(r.cashier_name)}</td><td>${esc(relativeDay(r.created_at))}</td><td>${statusPill(r.payment_status)}</td><td class="num">${money(r.total_kobo)}</td></tr>`).join('')}
      </tbody></table>` : `<div class="empty compact"><p>No receipts yet. Sales you make appear here.</p></div>`}
    </section>`;

  const root = ctx.main;
  root.onclick = (e) => {
    const t = e.target as HTMLElement;
    const go = t.closest<HTMLElement>('[data-go]');
    if (go) ctx.go(go.dataset.go!, { status: go.dataset.status, fulfilment: go.dataset.fulfilment, low: go.dataset.low, section: go.dataset.section, tab: go.dataset.tab });
    const row = t.closest<HTMLElement>('[data-receipt]');
    if (row) ctx.go('receipts', { open: Number(row.dataset.receipt) });
  };
  root.onkeydown = (e) => { if (e.key === 'Enter') { const row = (e.target as HTMLElement).closest<HTMLElement>('[data-receipt]'); if (row) ctx.go('receipts', { open: Number(row.dataset.receipt) }); } };
  $('[data-actions] [data-go]')?.addEventListener('click', () => ctx.go('pos'));
}
