import { auth, call } from '../api';
import { $, $$, confirmDialog, emptyState, errText, esc, toast } from '../lib/dom';
import { exportCsv } from '../lib/csv';
import { dateLabel, isoDate, money, nairaInput, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { printReceipt } from '../lib/receipt';
import { receiptText, shareWhatsApp } from '../lib/whatsapp';
import { store } from '../store';
import type { Receipt } from '../types';
import type { ViewCtx } from './shell';

// ------------------------------------------------------------------ types (poultry.rs)
interface Line { label: string; amount_kobo: number }
interface Alert { level: 'info' | 'warn' | 'bad'; text: string }
interface Plan { id: number; name: string; purpose: string; due_from: string; due_to: string; status: string; done_date: string; state: 'due' | 'overdue' | 'later' | 'done' | 'skipped' }
interface Batch {
  id: number; farm_id: number | null; name: string; bird_type: string; breed: string; house: string; manager: string; status: 'open' | 'closed'; is_demo: boolean;
  arrival_date: string; age_days: number; weeks: number; sell_age_weeks: number; sell_date: string; days_to_sell: number;
  planned_birds: number; chicks_ordered: number; chicks_delivered: number; dead_on_arrival: number; initial_live: number; added: number; deaths: number; removed: number;
  sold_birds: number; reserved_birds: number; alive: number; sick_now: number; deaths_today: number; mortality_pct: number; last_record: string; recorded_today: boolean;
  bags_per_100: number; bag_kg: number; planned_bags: number; planned_kg: number; buy_bags_plan: number; plan_starter: number; plan_grower: number; plan_finisher: number;
  issued_bags: number; eaten_kg: number; wasted_kg: number; plan_left_bags: number; feed_cost_kobo: number; feed_per_bird_kg: number;
  latest_avg_kg: number | null; prev_avg_kg: number | null; gain_kg: number | null; adg_g: number | null; fcr: number | null; est_market_kg: number | null; last_weighed: string;
  withdrawal_until: string; eligible: boolean; plan: Plan[];
  costs: Line[]; total_cost_kobo: number; pending_expenses: number; budget_kobo: number; revenue_kobo: number; received_kobo: number; customers_owe_kobo: number; suppliers_owed_kobo: number;
  price_per: 'bird' | 'kg'; expected_price_kobo: number; expected_weight_kg: number; remaining_value_kobo: number; profit_kobo: number; projected_profit_kobo: number; alerts: Alert[];
  money_hidden?: boolean;
}
interface Feed {
  id: number; name: string; brand: string; bag_kg: number; min_bags: number; full_bags: number; available: number; purchased: number; issued: number; damaged: number;
  avg_price_kobo: number; value_kobo: number; owed_kobo: number; last_supplier: string; last_price_kobo: number; daily_bags: number; days_left: number | null; finish_date: string; buy_bags: number;
  level: 'normal' | 'reducing' | 'restock' | 'urgent' | 'finished'; message: string;
}
interface Limits { warn_pct: number; low_pct: number; critical_days: number; mortality_pct: number; weigh_days: number }
interface Home { batches: Batch[]; feeds: Feed[]; limits: Limits; categories: string[]; timetable: [number, number, string, string][] }
type Row = Record<string, string | number | null>;
interface Detail { batch: Batch; form: Row; daily: Row[]; health: Row[]; weights: Row[]; expenses: Row[]; sales: Row[]; feeds: Feed[] }

const naira = (k: number) => money(k, { whole: true });
const n = (v: unknown) => Number(v ?? 0) || 0;
const s = (v: unknown) => String(v ?? '');
const plural = (k: number, w: string) => `${qty(k)} ${w}${k === 1 ? '' : 's'}`;
const PILL: Record<string, string> = { normal: 'pill-ok', reducing: 'pill-warn', restock: 'pill-warn', urgent: 'pill-bad', finished: 'pill-bad' };
const LEVEL: Record<string, string> = { normal: 'Normal stock', reducing: 'Reducing', restock: 'Restock now', urgent: 'Urgent', finished: 'Finished' };
let cats: string[] = [];

// ------------------------------------------------------------------ a simple question form

interface Q {
  name: string; label: string; type?: 'text' | 'number' | 'money' | 'date' | 'select' | 'yesno' | 'chips' | 'area';
  value?: unknown; options?: [string, string][]; hint?: string; when?: (v: Record<string, string>) => boolean; wide?: boolean;
}
interface Section { title: string; qs: Q[] }

function ask(ctx: ViewCtx, o: { title: string; lead: string; sections: Section[]; button: string; calc?: (v: Record<string, string>) => string; back: () => void; save: (v: Record<string, string>) => Promise<void> }) {
  ctx.header(o.title, o.lead);
  const field = (q: Q) => {
    const v = q.value === undefined || q.value === null ? '' : String(q.value);
    const hint = q.hint ? `<small>${q.hint}</small>` : '';
    const box = (inner: string) => `<label class="q ${q.wide ? 'q-wide' : ''}" data-q="${q.name}"><span class="q-text">${q.label}</span>${inner}${hint}</label>`;
    switch (q.type) {
      case 'money': return box(`<span class="naira-input"><i>₦</i><input name="${q.name}" inputmode="decimal" placeholder="0" value="${esc(v && v !== '0' ? nairaInput(Number(v)) : '')}"></span>`);
      case 'number': return box(`<input name="${q.name}" inputmode="decimal" placeholder="0" value="${esc(v && v !== '0' ? v : '')}">`);
      case 'date': return box(`<input type="date" name="${q.name}" value="${esc(v)}">`);
      case 'select': return box(`<select name="${q.name}">${(q.options ?? []).map(([k, l]) => `<option value="${esc(k)}" ${k === v ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select>`);
      case 'area': return box(`<textarea name="${q.name}" rows="2" maxlength="500">${esc(v)}</textarea>`);
      case 'yesno': case 'chips': {
        const opts = q.type === 'yesno' ? [['1', 'Yes'], ['0', 'No']] as [string, string][] : q.options ?? [];
        const cur = q.type === 'yesno' ? (v === 'true' || v === '1' ? '1' : v === '' ? '' : '0') : v;
        return `<div class="q ${q.wide ? 'q-wide' : ''}" data-q="${q.name}"><span class="q-text">${q.label}</span><div class="chip-choice">${opts.map(([k, l]) => `<label><input type="radio" name="${q.name}" value="${esc(k)}" ${cur === k ? 'checked' : ''}><span>${esc(l)}</span></label>`).join('')}</div>${hint}</div>`;
      }
      default: return box(`<input name="${q.name}" maxlength="200" value="${esc(v)}">`);
    }
  };
  ctx.main.innerHTML = `<form class="run-form" novalidate>
    <div class="run-form-main">${o.sections.map((sec, i) => `<section class="qs"><h2><span>${i + 1}</span>${sec.title}</h2><div class="q-grid">${sec.qs.map(field).join('')}</div></section>`).join('')}</div>
    <aside class="run-form-result" aria-live="polite">
      ${o.calc ? '<div data-calc></div>' : ''}
      <p class="form-error" role="alert" data-err></p>
      <button class="btn btn-primary btn-lg btn-block">${icon('check', 18)} ${o.button}</button>
      <button type="button" class="btn btn-ghost btn-block" data-cancel>Cancel</button>
    </aside></form>`;
  const form = $<HTMLFormElement>('.run-form', ctx.main)!;
  const all = o.sections.flatMap((x) => x.qs);
  const values = () => {
    const out: Record<string, string> = {};
    for (const q of all) {
      if (q.type === 'yesno' || q.type === 'chips') out[q.name] = (form.querySelector(`input[name="${q.name}"]:checked`) as HTMLInputElement | null)?.value ?? '';
      else out[q.name] = ((form.elements.namedItem(q.name) as HTMLInputElement | null)?.value ?? '').trim();
    }
    return out;
  };
  const draw = () => {
    const v = values();
    for (const q of all) if (q.when) { const el = $<HTMLElement>(`[data-q="${q.name}"]`, form); if (el) el.hidden = !q.when(v); }
    if (o.calc) $('[data-calc]', form)!.innerHTML = o.calc(v);
  };
  form.addEventListener('input', draw);
  form.addEventListener('change', draw);
  draw();
  $('[data-cancel]', form)!.addEventListener('click', o.back);
  form.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const err = $('[data-err]', form)!; err.textContent = '';
    const btn = $<HTMLButtonElement>('button.btn-primary', form)!; btn.disabled = true;
    try { await o.save(values()); } catch (ex) { err.textContent = errText(ex); btn.disabled = false; }
  });
}
const kobo = (v: string) => toKobo(v) || 0;
const num = (v: string) => Number(String(v).replace(/,/g, '')) || 0;
const int = (v: string) => Math.round(num(v));
const row = (a: string, b: string, cls = '') => `<div class="pl-row ${cls}"><span>${a}</span><span>${b}</span></div>`;

// ------------------------------------------------------------------ routing

export async function render(ctx: ViewCtx) {
  const p = ctx.params;
  const back = (id?: number) => () => (id ? ctx.go('batches', { open: id }) : ctx.go('batches'));
  const home = await call<Home>('pb_home');
  cats = home.categories;
  const pickBatch = () => home.batches.filter((b) => b.status === 'open' && store.inFarm(b.farm_id));
  const target = (p.batch ? Number(p.batch) : 0) || (pickBatch().length === 1 ? pickBatch()[0].id : 0);
  switch (p.form) {
    case 'batch': return batchForm(ctx, p.id ? Number(p.id) : null, back(p.id ? Number(p.id) : undefined));
    case 'daily': return target ? dailyForm(ctx, target, home, back(target)) : choose(ctx, pickBatch(), 'daily');
    case 'feed': return feedForm(ctx, home, back(target || undefined));
    case 'health': return target ? healthForm(ctx, target, p.plan ? Number(p.plan) : null, back(target)) : choose(ctx, pickBatch(), 'health');
    case 'weigh': return target ? weighForm(ctx, target, back(target)) : choose(ctx, pickBatch(), 'weigh');
    case 'expense': return target ? expenseForm(ctx, target, back(target)) : choose(ctx, pickBatch(), 'expense');
    case 'sell': return target ? saleForm(ctx, target, p.reservation ? Number(p.reservation) : null, back(target)) : choose(ctx, pickBatch(), 'sell');
    case 'limits': return limitsForm(ctx, home.limits, back());
  }
  if (typeof p.open === 'number') return batchPage(ctx, p.open);
  return homePage(ctx, home);
}

function choose(ctx: ViewCtx, list: Batch[], form: string) {
  ctx.header('Which batch?', 'Choose the batch this record is for.');
  ctx.main.innerHTML = list.length ? `<div class="run-grid">${list.map((b) => `<button type="button" class="run-card" data-b="${b.id}"><span class="run-head"><span class="run-icon">${icon('bird', 20)}</span><span class="run-name"><b>${esc(b.name)}</b><small>Day ${b.age_days} · ${plural(b.alive, 'bird')} alive</small></span></span></button>`).join('')}</div>`
    : emptyState('No open batch', 'Start a new batch first.');
  ctx.main.onclick = (e) => { const b = (e.target as HTMLElement).closest<HTMLElement>('[data-b]'); if (b) ctx.go('batches', { form, batch: b.dataset.b! }); };
}

// ------------------------------------------------------------------ home

function feedCard(f: Feed, showMoney: boolean) {
  return `<div class="feed-card lvl-${f.level}">
    <div class="feed-top"><b>${esc(f.name)}</b><span class="pill ${PILL[f.level]}">${LEVEL[f.level]}</span></div>
    <div class="feed-big">${qty(f.available)} <small>bags left</small></div>
    <div class="feed-meta">${f.days_left !== null ? `<span>About <b>${qty(f.days_left)} days</b> of feed · finishes ${esc(dateLabel(f.finish_date))}</span>` : '<span>Days left show after a few daily records</span>'}
      ${f.daily_bags ? `<span>Using about ${qty(Math.round(f.daily_bags * 10) / 10)} bags a day</span>` : ''}
      ${f.buy_bags ? `<span>Buy <b>${qty(f.buy_bags)} bags</b> to fill the store</span>` : ''}
      ${showMoney && f.last_price_kobo ? `<span>Last price ${naira(f.last_price_kobo)} a bag${f.last_supplier ? ` from ${esc(f.last_supplier)}` : ''}</span>` : ''}
      ${showMoney && f.owed_kobo ? `<span class="bad-text">You owe ${naira(f.owed_kobo)} for this feed</span>` : ''}</div>
    ${f.level !== 'normal' ? `<p class="feed-msg">${esc(f.message)}</p>` : ''}
  </div>`;
}

function homePage(ctx: ViewCtx, home: Home) {
  const manager = auth.can('manager'); const seller = auth.canSell();
  const batches = home.batches.filter((b) => store.inFarm(b.farm_id));
  const open = batches.filter((b) => b.status === 'open');
  ctx.header('Livestock', 'From the day the birds arrive to the day they are sold. Answer simple questions; FarmBuddy does the counting.',
    `${auth.can('owner') ? `<button class="btn btn-ghost" data-limits>${icon('sliders', 16)} Alert limits</button>` : ''}`);
  const choice = (attr: string, ic: string, t: string, h: string) => `<button type="button" class="big-choice" ${attr}><span>${icon(ic, 26)}</span><span class="bc-text"><b>${t}</b><small>${h}</small></span></button>`;
  ctx.main.innerHTML = `
    <section class="big-choices big-choices-4">
      ${choice('data-f="daily"', 'calendar', "Today's record", 'Deaths, sick birds, feed given')}
      ${manager ? choice('data-f="feed"', 'box', 'Buy feed', 'Add bags to the feed store') : ''}
      ${seller ? choice('data-f="sell"', 'cash', 'Sell birds', 'Makes a receipt') : ''}
      ${manager ? choice('data-f="batch"', 'plus', 'Start a new batch', 'When new chicks arrive') : ''}
    </section>
    ${open.length ? (() => {
      const sum = (f: (b: Batch) => number) => open.reduce((t, b) => t + f(b), 0);
      const due = open.reduce((t, b) => t + b.plan.filter((p) => p.state === 'due' || p.state === 'overdue').length, 0);
      const low = home.feeds.filter((f) => f.level !== 'normal').length;
      const tile = (l: string, v: string, sub = '', tone = '') => `<div class="kpi ${tone}"><small>${l}</small><b>${v}</b>${sub ? `<span>${sub}</span>` : ''}</div>`;
      return `<section class="stack"><h2 class="section-title">Dashboard</h2><div class="kpi-grid">
        ${tile('Birds alive', qty(sum((b) => b.alive)), `${open.length} batch${open.length === 1 ? '' : 'es'} running`)}
        ${tile('Died', qty(sum((b) => b.deaths)), `${qty(sum((b) => b.deaths_today))} today`)}
        ${tile('Feed bags in store', qty(home.feeds.reduce((t, f) => t + Math.max(0, f.available), 0)), low ? `${low} feed${low === 1 ? '' : 's'} to buy` : 'enough for now', low ? 'kpi-warn' : '')}
        ${tile('Vaccination', due ? `${due} due` : 'Nothing due', '', due ? 'kpi-warn' : '')}
        ${tile('Sold', qty(sum((b) => b.sold_birds)), `${qty(sum((b) => b.reserved_birds))} reserved`)}
        ${manager ? `${tile('Spent', naira(sum((b) => b.total_cost_kobo)))}${tile('Sales', naira(sum((b) => b.revenue_kobo)), `owed to you ${naira(sum((b) => b.customers_owe_kobo))}`)}
          ${tile('Expected profit', `${sum((b) => b.projected_profit_kobo) < 0 ? 'Loss ' : ''}${naira(Math.abs(sum((b) => b.projected_profit_kobo)))}`, 'when all birds are sold', sum((b) => b.projected_profit_kobo) < 0 ? 'kpi-bad' : 'kpi-ok')}` : ''}
      </div></section>`;
    })() : ''}
    <section class="stack"><h2 class="section-title">Feed store</h2>
      ${home.feeds.length ? `<div class="feed-grid">${home.feeds.map((f) => feedCard(f, manager)).join('')}</div>` : emptyState('No feed in the store yet', manager ? 'Tap “Buy feed” to add your starter, grower or finisher feed.' : 'Ask the manager to add the feed.')}
    </section>
    <section class="stack"><h2 class="section-title">Batches</h2>
      ${batches.length ? `<div class="run-grid">${batches.map((b) => batchCard(b, manager)).join('')}</div>` : emptyState('No batch yet', manager ? 'Tap “Start a new batch” when your chicks arrive.' : 'Ask the manager to start the batch.')}
    </section>
    <p class="muted small">Goats, pigs, fish and other animals: <a href="#" data-old>open the other livestock records</a>.</p>`;
  ctx.main.onclick = (e) => {
    const el = e.target as HTMLElement;
    const f = el.closest<HTMLElement>('[data-f]');
    if (f) { if ((f.dataset.f === 'daily' || f.dataset.f === 'sell') && !open.length) { toast('Start a batch first', 'error'); return; } ctx.go('batches', { form: f.dataset.f! }); return; }
    const b = el.closest<HTMLElement>('[data-batch]'); if (b) { ctx.go('batches', { open: Number(b.dataset.batch) }); return; }
    if (el.closest('[data-old]')) { e.preventDefault(); ctx.go('livestock'); }
  };
  $('[data-limits]')?.addEventListener('click', () => ctx.go('batches', { form: 'limits' }));
}

function batchCard(b: Batch, showMoney: boolean) {
  const next = b.plan.find((p) => p.state === 'overdue' || p.state === 'due') ?? b.plan.find((p) => p.state === 'later');
  const bad = b.alerts.filter((a) => a.level === 'bad').length;
  return `<button type="button" class="run-card ${b.status === 'closed' ? 'is-closed' : ''}" data-batch="${b.id}">
    <span class="run-head"><span class="run-icon">${icon('bird', 20)}</span>
      <span class="run-name"><b>${esc(b.name)}</b><small>${esc(b.bird_type)} · day ${b.age_days}${b.house ? ` · ${esc(b.house)}` : ''}</small></span>
      <span class="run-pills">${bad ? `<span class="pill pill-bad">${bad} alert${bad === 1 ? '' : 's'}</span>` : ''}<span class="pill ${b.status === 'open' ? 'pill-ok' : 'pill-mute'}">${b.status === 'open' ? 'Running' : 'Closed'}</span></span></span>
    <span class="run-tiles">
      <span><small>Alive</small><b>${qty(b.alive)}</b></span>
      <span><small>Died</small><b>${qty(b.deaths)} (${b.mortality_pct}%)</b></span>
      <span><small>Feed used</small><b>${qty(b.issued_bags)} of ${qty(b.planned_bags)} bags</b></span>
      ${next && b.status === 'open' ? `<span><small>${next.state === 'later' ? 'Next' : next.state === 'due' ? 'Due now' : 'Overdue'}</small><b>${esc(next.name.split(',')[0])}</b></span>` : ''}
      ${showMoney && !b.money_hidden ? `<span class="run-pl ${b.projected_profit_kobo < 0 ? 'is-loss' : 'is-profit'}"><small>${b.status === 'closed' ? (b.profit_kobo < 0 ? 'Loss' : 'Profit') : 'Expected profit'}</small><b>${naira(Math.abs(b.status === 'closed' ? b.profit_kobo : b.projected_profit_kobo))}</b></span>` : ''}
    </span></button>`;
}

// ------------------------------------------------------------------ one batch: the owner's dashboard

async function batchPage(ctx: ViewCtx, id: number) {
  const d = await call<Detail>('pb_batch', { id });
  const b = d.batch;
  const manager = auth.can('manager'); const owner = auth.can('owner'); const showMoney = manager && !b.money_hidden;
  const open = b.status === 'open';
  ctx.header(b.name, `${b.bird_type}${b.breed ? ` (${b.breed})` : ''} · arrived ${dateLabel(b.arrival_date)} · day ${b.age_days}${open ? '' : ' · closed'}`,
    `<button class="btn btn-ghost" data-all>${icon('back', 16)} All batches</button><button class="btn btn-ghost" data-print>${icon('print', 16)} Report</button>${manager ? `<button class="btn btn-ghost" data-edit>${icon('edit', 16)} Change answers</button>` : ''}`);
  const feedLeft = d.feeds.reduce((t, f) => t + Math.max(0, f.available), 0);
  const soonest = d.feeds.filter((f) => f.days_left !== null).sort((a, b2) => (a.days_left ?? 0) - (b2.days_left ?? 0))[0];
  const toBuy = d.feeds.filter((f) => f.level !== 'normal');
  const due = b.plan.filter((p) => p.state === 'due' || p.state === 'overdue');
  const tile = (label: string, value: string, sub = '', tone = '') => `<div class="kpi ${tone}"><small>${label}</small><b>${value}</b>${sub ? `<span>${sub}</span>` : ''}</div>`;
  const act = (f: string, ic: string, t: string, extra = '') => `<button type="button" class="btn btn-ghost" data-act="${f}" ${extra}>${icon(ic, 16)} ${t}</button>`;
  ctx.main.innerHTML = `
    ${b.is_demo ? `<div class="callout callout-warn">${icon('info', 18)}<span>Demo batch.</span></div>` : ''}
    <section class="kpi-grid">
      ${tile('Birds alive', qty(b.alive), `${b.sick_now ? `${b.sick_now} sick · ` : ''}${b.reserved_birds ? `${b.reserved_birds} reserved · ` : ''}${b.sold_birds} sold`)}
      ${tile('Died so far', `${qty(b.deaths)}`, `${b.mortality_pct}% · ${b.deaths_today} today`, b.mortality_pct > 5 ? 'kpi-bad' : '')}
      ${tile('Feed bags left', qty(feedLeft), soonest ? `finishes ${dateLabel(soonest.finish_date)}` : 'in the store', toBuy.length ? 'kpi-warn' : '')}
      ${tile('To buy', toBuy.length ? toBuy.map((f) => `${f.buy_bags ? qty(f.buy_bags) + ' bags ' : ''}${esc(f.name)}`).join(', ') : 'Nothing now', '', toBuy.length ? 'kpi-warn' : '')}
      ${tile('Vaccination', due.length ? esc(due[0].name.split(',')[0]) : 'Nothing due', due.length ? (due[0].state === 'overdue' ? 'overdue' : 'due now') : '', due.some((x) => x.state === 'overdue') ? 'kpi-bad' : due.length ? 'kpi-warn' : '')}
      ${showMoney ? `${tile('Spent', naira(b.total_cost_kobo), b.budget_kobo ? `budget ${naira(b.budget_kobo)}` : '')}
        ${tile('Sold', naira(b.revenue_kobo), `received ${naira(b.received_kobo)}`)}
        ${tile('Owed to you', naira(b.customers_owe_kobo), b.suppliers_owed_kobo ? `you owe ${naira(b.suppliers_owed_kobo)}` : '', b.customers_owe_kobo ? 'kpi-warn' : '')}
        ${tile(b.profit_kobo < 0 ? 'Loss so far' : 'Profit so far', naira(Math.abs(b.profit_kobo)), '', b.profit_kobo < 0 ? 'kpi-bad' : 'kpi-ok')}
        ${tile('Expected when all are sold', `${b.projected_profit_kobo < 0 ? 'Loss ' : 'Profit '}${naira(Math.abs(b.projected_profit_kobo))}`, `${plural(b.alive, 'bird')} at ${naira(b.expected_price_kobo)} per ${b.price_per}`, b.projected_profit_kobo < 0 ? 'kpi-bad' : 'kpi-ok')}` : ''}
    </section>
    ${b.alerts.length ? `<section class="alert-list">${b.alerts.map((a) => `<div class="callout ${a.level === 'bad' ? 'callout-bad' : a.level === 'warn' ? 'callout-warn' : ''}">${icon(a.level === 'info' ? 'info' : 'alert', 16)}<span>${esc(a.text)}</span></div>`).join('')}</section>` : ''}
    ${open ? `<section class="act-row">
      ${act('daily', 'calendar', "Today's record")}${act('health', 'shield', 'Vaccine or medicine')}${act('weigh', 'chart', 'Weigh birds')}
      ${manager ? act('feed', 'box', 'Buy feed') + act('expense', 'cash', 'Money spent') : ''}${auth.canSell() ? act('sell', 'cash', 'Sell birds') : ''}
      ${manager ? `<button type="button" class="btn btn-ghost" data-close>${icon('check', 16)} Close batch</button>` : ''}
    </section>` : manager ? `<section class="act-row"><button type="button" class="btn btn-ghost" data-reopen>Reopen batch</button></section>` : ''}

    <div class="run-layout">
      <section class="panel"><header class="panel-head"><h2>Birds</h2></header>
        ${row('Chicks ordered', qty(b.chicks_ordered))}${row('Chicks delivered', qty(b.chicks_delivered))}${row('Dead on arrival', qty(b.dead_on_arrival))}
        ${row('<b>Alive at the start</b>', `<b>${qty(b.initial_live)}</b>`, 'pl-total')}${b.added ? row('Added later', qty(b.added)) : ''}
        ${row('Died', qty(b.deaths))}${b.removed ? row('Removed or transferred', qty(b.removed)) : ''}${row('Sold', qty(b.sold_birds))}
        ${row('<b>Alive now</b>', `<b>${qty(b.alive)}</b>`, 'pl-total')}${b.reserved_birds ? row('Reserved for buyers', qty(b.reserved_birds)) : ''}
        ${!b.eligible ? `<div class="callout callout-bad">${icon('alert', 16)}<span>Do not sell or slaughter before ${esc(dateLabel(b.withdrawal_until))} (medicine withdrawal).</span></div>` : ''}
      </section>
      <section class="panel"><header class="panel-head"><h2>Feed: plan and actual</h2></header>
        ${row(`Plan: ${qty(b.bags_per_100)} bags per 100 birds × ${qty(b.planned_birds)} birds`, `<b>${qty(b.planned_bags)} bags</b>`)}
        ${row(`= ${qty(b.planned_kg)} kg (bags of ${qty(b.bag_kg)} kg)`, `buy ${qty(b.buy_bags_plan)} bags`)}
        ${b.plan_starter || b.plan_grower || b.plan_finisher ? row('Starter / grower / finisher planned', `${qty(b.plan_starter)} / ${qty(b.plan_grower)} / ${qty(b.plan_finisher)}`) : ''}
        ${row('<b>Bags given so far</b>', `<b class="${b.issued_bags > b.planned_bags ? 'bad-text' : ''}">${qty(b.issued_bags)}</b>`, 'pl-total')}
        ${row('Bags left in the plan', qty(b.plan_left_bags))}${row('Feed eaten', `${qty(Math.round(b.eaten_kg))} kg (${qty(b.feed_per_bird_kg)} kg a bird)`)}
        ${b.wasted_kg ? row('Wasted', `${qty(b.wasted_kg)} kg`) : ''}${showMoney ? row('Cost of feed given', naira(b.feed_cost_kobo)) : ''}
        <p class="muted small">The plan is your budget. Feed really used is counted from the daily records.</p>
      </section>
    </div>

    <div class="run-layout">
      <section class="panel"><header class="panel-head"><h2>Growth</h2></header>
        ${b.latest_avg_kg !== null ? `${row('Average weight now', `<b>${qty(b.latest_avg_kg)} kg</b>`)}${b.prev_avg_kg !== null ? row('Last time', `${qty(b.prev_avg_kg)} kg`) : ''}
          ${b.gain_kg !== null ? row('Gained', `${qty(b.gain_kg)} kg${b.adg_g !== null ? ` (${qty(b.adg_g)} g a day)` : ''}`) : ''}${b.fcr !== null ? row('FCR (feed ÷ weight)', qty(b.fcr)) : ''}
          ${b.est_market_kg !== null ? row(`Expected weight on ${dateLabel(b.sell_date)}`, `${qty(b.est_market_kg)} kg`) : ''}` : '<p class="muted">Weigh some birds every week to see growth and FCR.</p>'}
      </section>
      ${showMoney ? `<section class="panel pl-card"><header class="panel-head"><h2>Profit and loss</h2></header>
        <div class="pl-group"><p class="pl-title">Spent</p>${b.costs.map((c) => row(esc(c.label), naira(c.amount_kobo))).join('')}${row('<b>Total spent</b>', `<b>${naira(b.total_cost_kobo)}</b>`, 'pl-total')}</div>
        <div class="pl-group"><p class="pl-title">Sold</p>${row('Sales', naira(b.revenue_kobo))}${row('Money received', naira(b.received_kobo))}${row('Customers owe', naira(b.customers_owe_kobo))}</div>
        <div class="pl-result ${b.profit_kobo < 0 ? 'is-loss' : 'is-profit'}"><span>${b.profit_kobo < 0 ? 'Loss' : 'Profit'} so far</span><b>${naira(Math.abs(b.profit_kobo))}</b></div>
        ${open ? `<div class="pl-unsold">${row(`${plural(b.alive, 'bird')} not sold yet, at your price`, naira(b.remaining_value_kobo))}${row('<b>Expected when all are sold</b>', `<b class="${b.projected_profit_kobo < 0 ? 'bad-text' : 'ok-text'}">${b.projected_profit_kobo < 0 ? 'Loss ' : 'Profit '}${naira(Math.abs(b.projected_profit_kobo))}</b>`, 'pl-total')}</div>` : ''}
        ${b.suppliers_owed_kobo ? row('You still owe suppliers', `<b class="bad-text">${naira(b.suppliers_owed_kobo)}</b>`) : ''}
      </section>` : ''}
    </div>

    ${b.plan.length ? `<section class="panel"><header class="panel-head"><h2>Vaccination and health timetable</h2></header>
      <p class="muted small">A planning calendar. Your vet must confirm the final dates and products (hatchery records, vaccine instructions, local advice). Do not vaccinate very sick or stressed birds without the vet.</p>
      <div class="table-wrap"><table class="table"><thead><tr><th>When</th><th>What</th><th>Why</th><th>Status</th><th></th></tr></thead><tbody>
      ${b.plan.map((p) => `<tr class="plan-${p.state}"><td>${esc(dateLabel(p.due_from))}${p.due_to !== p.due_from ? ` – ${esc(dateLabel(p.due_to))}` : ''}</td><td>${esc(p.name)}</td><td>${esc(p.purpose)}</td>
        <td><span class="pill ${p.state === 'done' ? 'pill-ok' : p.state === 'overdue' ? 'pill-bad' : p.state === 'due' ? 'pill-warn' : 'pill-mute'}">${p.state === 'done' ? `Done ${esc(dateLabel(p.done_date))}` : p.state === 'skipped' ? 'Not needed' : p.state === 'due' ? 'Due now' : p.state === 'overdue' ? 'Overdue' : 'Later'}</span></td>
        <td class="row-actions">${open && (p.state === 'due' || p.state === 'overdue' || p.state === 'later') ? `<button class="btn btn-ghost btn-sm" data-do="${p.id}">Record</button><button class="btn btn-ghost btn-sm" data-skip="${p.id}">Not needed</button>` : ''}</td></tr>`).join('')}
      </tbody></table></div></section>` : ''}

    ${tableBlock('Daily records', d.daily, ['Day', 'Died', 'Sick', 'Removed', 'Added', 'Feed', 'Bags', 'Wasted kg', 'Notes', 'By'], (r) => [dateLabel(s(r.day)), r.deaths, r.sick, r.removed, r.added, s(r.feed_name), r.bags_issued, r.kg_wasted, [s(r.cause), s(r.observations), s(r.problems)].filter(Boolean).join(' · '), s(r.user_name)], 'daily')}
    ${tableBlock('Vaccines, medicine and vitamins', d.health, ['Date', 'Type', 'Name', 'Birds', 'Method', 'Given by', 'Withdrawal', 'Next due'], (r) => [dateLabel(s(r.done_date)), s(r.kind), s(r.name), r.birds, s(r.method), s(r.given_by), n(r.withdrawal_days) ? `${r.withdrawal_days} days` : '—', s(r.next_due) ? dateLabel(s(r.next_due)) : '—'], 'health')}
    ${tableBlock('Weighing', d.weights, ['Date', 'Birds weighed', 'Total kg', 'Average kg', 'Notes'], (r) => [dateLabel(s(r.weigh_date)), r.sampled, r.total_kg, r.avg_kg, [s(r.smaller), s(r.notes)].filter(Boolean).join(' · ')], 'weight')}
    ${showMoney ? tableBlock('Money spent', d.expenses, ['Date', 'Category', 'What', 'Amount', 'Paid', 'Approved'], (r) => [dateLabel(s(r.spent_date)), s(r.category), s(r.item), naira(n(r.amount_kobo)), naira(n(r.paid_kobo)), n(r.approved) ? `Yes (${s(r.approved_by)})` : owner ? h(`<button class="btn btn-ghost btn-sm" data-approve="${r.id}">Approve</button>`) : 'Waiting'], 'expense') : ''}
    ${tableBlock('Sales', d.sales, ['Date', 'Status', 'Birds', 'Buyer', 'Price', 'Amount', 'Owed', 'Receipt'], (r) => [dateLabel(s(r.sale_date)), s(r.status) === 'reserved' ? h(`Reserved${auth.canSell() && open ? ` <button class="btn btn-ghost btn-sm" data-confirm-sale="${r.id}">Confirm sale</button>` : ''}`) : s(r.payment_status) === 'void' ? 'Voided' : 'Sold',
      r.birds, s(r.buyer) || '—', showMoney ? `${naira(n(r.price_kobo))}/${s(r.price_per)}` : '', showMoney ? naira(n(r.amount_kobo)) : '', showMoney && n(r.owed_kobo) ? naira(n(r.owed_kobo)) : '—', s(r.receipt_no) || '—'], 'reservation')}
  `;
  const go = (form: string, extra: Record<string, string> = {}) => ctx.go('batches', { form, batch: String(id), ...extra });
  ctx.main.onclick = async (e) => {
    const el = e.target as HTMLElement;
    const a = el.closest<HTMLElement>('[data-act]'); if (a) return go(a.dataset.act!);
    const doP = el.closest<HTMLElement>('[data-do]'); if (doP) return go('health', { plan: doP.dataset.do! });
    const conf = el.closest<HTMLElement>('[data-confirm-sale]'); if (conf) return go('sell', { reservation: conf.dataset.confirmSale! });
    const skip = el.closest<HTMLElement>('[data-skip]');
    if (skip) { await call('pb_mark_plan', { id: Number(skip.dataset.skip), status: 'skipped' }); return ctx.go('batches', { open: id }); }
    const ap = el.closest<HTMLElement>('[data-approve]');
    if (ap) { await call('pb_approve_expense', { id: Number(ap.dataset.approve) }); toast('Expense approved'); return ctx.go('batches', { open: id }); }
    const del = el.closest<HTMLElement>('[data-del]');
    if (del) {
      const reason = await confirmDialog({ title: 'Remove this record?', message: 'The removal and your reason are kept in the activity log.', confirm: 'Remove', danger: true, reason: { label: 'Why?', required: true } });
      if (reason === null) return;
      try { await call('pb_delete', { kind: del.dataset.kind, id: Number(del.dataset.del), reason }); toast('Removed'); ctx.go('batches', { open: id }); } catch (ex) { toast(errText(ex), 'error'); }
      return;
    }
    if (el.closest('[data-close]') || el.closest('[data-reopen]')) {
      const reopen = !!el.closest('[data-reopen]');
      if (!reopen && (await confirmDialog({ title: `Close ${b.name}?`, message: b.alive > 0 ? `${plural(b.alive, 'bird')} are still alive and not sold. Close only when the batch is finished.` : 'The final result is kept.', confirm: 'Close batch' })) === null) return;
      await call('pb_close_batch', { id, reopen }); ctx.go('batches', { open: id });
    }
    const csv = el.closest<HTMLElement>('[data-csv]');
    if (csv) await exportCsv(`${b.name}-daily-records.csv`, ['Day', 'Died', 'Cause', 'Sick', 'Removed', 'Added', 'Feed', 'Bags given', 'Kg eaten', 'Kg wasted', 'Water (l)', 'Observations', 'Problems', 'Tomorrow', 'Recorded by'],
      d.daily.map((r) => [r.day, r.deaths, r.cause, r.sick, r.removed, r.added, r.feed_name, r.bags_issued, r.kg_eaten, r.kg_wasted, r.water_l, r.observations, r.problems, r.tomorrow, r.user_name]));
  };
  $('[data-all]')?.addEventListener('click', () => ctx.go('batches'));
  $('[data-edit]')?.addEventListener('click', () => ctx.go('batches', { form: 'batch', id: String(id) }));
  $('[data-print]')?.addEventListener('click', () => window.print());
}

const h = (html: string) => ({ html });
function tableBlock(title: string, list: Row[], heads: string[], cells: (r: Row) => unknown[], kind: string) {
  const owner = auth.can('owner');
  const cell = (v: unknown) => (v && typeof v === 'object' && 'html' in (v as object) ? (v as { html: string }).html : esc(v ?? ''));
  return `<section class="panel"><header class="panel-head"><h2>${title}</h2>${kind === 'daily' && list.length ? '<button class="btn btn-ghost btn-sm" data-csv>Excel (CSV)</button>' : ''}</header>
    ${list.length ? `<div class="table-wrap"><table class="table"><thead><tr>${heads.map((x) => `<th>${x}</th>`).join('')}${owner ? '<th></th>' : ''}</tr></thead><tbody>
      ${list.map((r) => `<tr>${cells(r).map((c) => `<td>${cell(c)}</td>`).join('')}${owner ? `<td>${kind !== 'reservation' || r.status === 'reserved' ? `<button class="btn btn-ghost btn-sm" data-del="${r.id}" data-kind="${kind}" title="Remove">${icon('trash', 14)}</button>` : ''}</td>` : ''}</tr>`).join('')}
    </tbody></table></div>` : '<p class="muted">Nothing recorded yet.</p>'}</section>`;
}

// ------------------------------------------------------------------ forms (one per job, plain questions)

async function batchForm(ctx: ViewCtx, id: number | null, back: () => void) {
  const f: Row = id ? (await call<Detail>('pb_batch', { id })).form : {};
  const v = (k: string, def: unknown = '') => (id ? f[k] : def);
  ask(ctx, {
    title: id ? 'Change batch answers' : 'Start a new batch', lead: 'Answer what you know now. You can change it later.', button: id ? 'Save changes' : 'Start batch', back,
    sections: [
      { title: 'The batch', qs: [
        { name: 'name', label: 'What is the name or number of this batch?', value: v('name'), hint: 'e.g. Batch 3, or December broilers' },
        { name: 'bird_type', label: 'What type of birds are you rearing?', type: 'select', value: v('bird_type', 'Broilers'), options: ['Broilers', 'Layers', 'Cockerels', 'Noilers', 'Turkeys', 'Other birds'].map((x) => [x, x]) },
        { name: 'breed', label: 'Which breed?', value: v('breed'), hint: 'e.g. Arbor Acres, Ross, Cobb' },
        { name: 'house', label: 'Which poultry house or pen?', value: v('house') },
        { name: 'manager', label: 'Who is responsible for this batch?', value: v('manager') },
        { name: 'planned_birds', label: 'How many birds do you intend to rear?', type: 'number', value: v('planned_birds') },
        { name: 'arrival_date', label: 'When do (did) the chicks arrive?', type: 'date', value: v('arrival_date', isoDate(new Date())) },
        { name: 'age_at_arrival', label: 'How old are the birds when they arrive (days)?', type: 'number', value: v('age_at_arrival', 1) },
        { name: 'weeks', label: 'How many weeks will you rear them?', type: 'number', value: v('weeks', 8) },
        { name: 'sell_age_weeks', label: 'At what age (weeks) will you sell them?', type: 'number', value: v('sell_age_weeks', 8) },
      ] },
      { title: 'The chicks you received', qs: [
        { name: 'chicks_ordered', label: 'How many chicks did you order?', type: 'number', value: v('chicks_ordered') },
        { name: 'chicks_delivered', label: 'How many were delivered?', type: 'number', value: v('chicks_delivered') },
        { name: 'dead_on_arrival', label: 'How many were dead on arrival?', type: 'number', value: v('dead_on_arrival') },
        { name: 'chicks_paid_for', label: 'How many chicks did you pay for?', type: 'number', value: v('chicks_paid_for') },
        { name: 'chick_price_kobo', label: 'How much did each chick cost?', type: 'money', value: v('chick_price_kobo') },
        { name: 'chick_transport_kobo', label: 'How much was transport?', type: 'money', value: v('chick_transport_kobo') },
        { name: 'chick_supplier', label: 'Who supplied the chicks?', value: v('chick_supplier') },
        { name: 'chick_amount_paid_kobo', label: 'How much have you paid the supplier?', type: 'money', value: v('chick_amount_paid_kobo') },
        { name: 'supplier_receipt', label: "Do you have the supplier's receipt?", type: 'yesno', value: v('supplier_receipt', 0) },
        { name: 'hatchery_vaccines', label: 'Which vaccines did the hatchery give?', value: v('hatchery_vaccines'), hint: "e.g. Marek's, Newcastle/IB. Leave empty if you don't know." },
      ] },
      { title: 'Feed plan', qs: [
        { name: 'bags_per_100', label: 'How many bags of feed for every 100 birds?', type: 'number', value: v('bags_per_100', 17), hint: 'Your farm standard is 17. Change it if your plan is different.' },
        { name: 'bag_kg', label: 'How many kg is one bag?', type: 'number', value: v('bag_kg', 25) },
        { name: 'plan_starter', label: 'Bags of starter you intend to use', type: 'number', value: v('plan_starter') },
        { name: 'plan_grower', label: 'Bags of grower (if any)', type: 'number', value: v('plan_grower') },
        { name: 'plan_finisher', label: 'Bags of finisher', type: 'number', value: v('plan_finisher') },
        { name: 'feed_change', label: 'At what age do you change feed?', value: v('feed_change'), hint: 'e.g. starter to week 3, finisher from week 4' },
      ] },
      { title: 'Selling plan and budget', qs: [
        { name: 'expected_sell', label: 'How many birds do you expect to sell?', type: 'number', value: v('expected_sell') },
        { name: 'price_per', label: 'Will you sell per bird or per kg?', type: 'chips', value: v('price_per', 'bird'), options: [['bird', 'Per bird'], ['kg', 'Per kg']] },
        { name: 'expected_price_kobo', label: 'Expected selling price (per bird or per kg)', type: 'money', value: v('expected_price_kobo') },
        { name: 'expected_weight_kg', label: 'Expected average selling weight (kg)', type: 'number', value: v('expected_weight_kg') },
        { name: 'budget_kobo', label: 'Your production budget for this batch', type: 'money', value: v('budget_kobo') },
        ...(id ? [] : [{ name: 'add_timetable', label: 'Add the broiler vaccination and health timetable (day 1–56)?', type: 'yesno' as const, value: 1, hint: 'Your vet must confirm the dates and products.' }]),
        { name: 'notes', label: 'Anything else?', type: 'area', value: v('notes'), wide: true },
      ] },
    ],
    calc: (x) => {
      const birds = num(x.planned_birds), per = num(x.bags_per_100), kg = num(x.bag_kg) || 25;
      const bags = Math.round(birds / 100 * per * 1000) / 1000;
      const alive = int(x.chicks_delivered) - int(x.dead_on_arrival);
      const chickCost = int(x.chicks_paid_for) * kobo(x.chick_price_kobo);
      const balance = chickCost + kobo(x.chick_transport_kobo) - kobo(x.chick_amount_paid_kobo);
      return `<h2>What FarmBuddy works out</h2><div class="pl-group">
        ${row('Feed needed', `<b>${qty(bags)} bags</b>`)}${row('Total feed', `${qty(Math.round(bags * kg * 10) / 10)} kg`)}${row('Buy (whole bags)', `${qty(Math.ceil(bags))} bags`)}
        ${row('Feed per bird', birds ? `${qty(Math.round(per * kg / 100 * 100) / 100)} kg` : '—')}</div>
        <div class="pl-group">${row('Alive at arrival', alive > 0 ? qty(alive) : '—')}${row('Chicks cost', naira(chickCost))}${row('Average per chick (with transport)', alive > 0 ? naira((chickCost + kobo(x.chick_transport_kobo)) / alive) : '—')}
        ${balance > 0 ? row('<b>Still owed to supplier</b>', `<b class="bad-text">${naira(balance)}</b>`) : ''}</div>
        <p class="muted small">The feed plan is a budget, not a fact: real use depends on breed, feed, weather and deaths. Record what you really use every day.</p>`;
    },
    save: async (x) => {
      const saved = await call<number>('pb_save_batch', { form: {
        id, farm_id: id ? (f.farm_id ?? null) : (store.farmId ?? null), name: x.name, bird_type: x.bird_type, breed: x.breed, house: x.house, manager: x.manager,
        planned_birds: int(x.planned_birds), arrival_date: x.arrival_date, age_at_arrival: int(x.age_at_arrival), weeks: int(x.weeks), sell_age_weeks: int(x.sell_age_weeks),
        expected_sell: int(x.expected_sell), expected_weight_kg: num(x.expected_weight_kg), price_per: x.price_per || 'bird', expected_price_kobo: kobo(x.expected_price_kobo), budget_kobo: kobo(x.budget_kobo),
        bags_per_100: num(x.bags_per_100), bag_kg: num(x.bag_kg) || 25, plan_starter: num(x.plan_starter), plan_grower: num(x.plan_grower), plan_finisher: num(x.plan_finisher), feed_change: x.feed_change,
        chicks_ordered: int(x.chicks_ordered), chicks_delivered: int(x.chicks_delivered), dead_on_arrival: int(x.dead_on_arrival), chicks_paid_for: int(x.chicks_paid_for),
        chick_price_kobo: kobo(x.chick_price_kobo), chick_transport_kobo: kobo(x.chick_transport_kobo), chick_supplier: x.chick_supplier, chick_amount_paid_kobo: kobo(x.chick_amount_paid_kobo),
        supplier_receipt: x.supplier_receipt === '1', hatchery_vaccines: x.hatchery_vaccines, notes: x.notes, add_timetable: x.add_timetable === '1' } });
      toast(id ? 'Saved' : 'Batch started'); ctx.go('batches', { open: saved });
    },
  });
}

async function dailyForm(ctx: ViewCtx, id: number, home: Home, back: () => void) {
  const b = home.batches.find((x) => x.id === id)!;
  const feeds = home.feeds;
  ask(ctx, {
    title: `Today's record · ${b.name}`, lead: `Day ${b.age_days}. FarmBuddy counts ${plural(b.alive, 'bird')} alive before this record.`, button: 'Save record', back,
    sections: [
      { title: 'The birds', qs: [
        { name: 'day', label: 'Which day is this record for?', type: 'date', value: isoDate(new Date()) },
        { name: 'deaths', label: 'How many birds died today?', type: 'number' },
        { name: 'cause', label: 'What was the cause of death, if known?', when: (v) => int(v.deaths) > 0 },
        { name: 'disposed', label: 'Have you disposed of the dead birds safely?', type: 'yesno', value: 1, when: (v) => int(v.deaths) > 0 },
        { name: 'sick', label: 'How many birds are sick or kept apart now?', type: 'number' },
        { name: 'removed', label: 'How many birds were removed or transferred out?', type: 'number' },
        { name: 'added', label: 'How many birds were added?', type: 'number' },
      ] },
      { title: 'Feed and water', qs: [
        ...(feeds.length ? [{ name: 'feed_id', label: 'Which feed did you give?', type: 'select' as const, options: [['', 'No feed from the store today'], ...feeds.map((f) => [String(f.id), `${f.name} (${qty(f.available)} bags in store)`] as [string, string])] as [string, string][], value: feeds.length === 1 ? String(feeds[0].id) : '' }] : []),
        { name: 'bags_issued', label: 'How many bags did you take from the store to the birds?', type: 'number', when: (v) => !!v.feed_id },
        { name: 'kg_wasted', label: 'Was any feed spilled or wasted? How many kg?', type: 'number' },
        { name: 'kg_eaten', label: 'If you measured it: kg the birds actually ate', type: 'number', hint: 'Leave empty if you did not measure.' },
        { name: 'feed_unusual', label: 'Did the birds eat much more or much less than usual?', type: 'chips', options: [['', 'Normal'], ['more', 'More'], ['less', 'Less']], value: '' },
        { name: 'water_l', label: 'Water used (litres), if you measure it', type: 'number' },
      ] },
      { title: 'Notes for the owner', qs: [
        { name: 'observations', label: 'What did you notice? (sick birds, droppings, behaviour)', type: 'area', wide: true },
        { name: 'problems', label: 'Any problem that needs attention?', type: 'area', wide: true },
        { name: 'tomorrow', label: 'What is planned for tomorrow?', type: 'area', wide: true },
      ] },
    ],
    calc: (v) => {
      const alive = b.alive + int(v.added) - int(v.deaths) - int(v.removed);
      const f = feeds.find((x) => String(x.id) === v.feed_id);
      const left = f ? f.available - num(v.bags_issued) : null;
      return `<h2>After this record</h2><div class="pl-group">${row('Alive at the start of today', qty(b.alive))}${row('Died', qty(int(v.deaths)))}${int(v.added) ? row('Added', qty(int(v.added))) : ''}${int(v.removed) ? row('Removed', qty(int(v.removed))) : ''}
        ${row('<b>Alive now</b>', `<b class="${alive < 0 ? 'bad-text' : ''}">${qty(alive)}</b>`, 'pl-total')}</div>
        ${f ? `<div class="pl-group">${row(`${esc(f.name)} in store`, `${qty(f.available)} bags`)}${row('Given today', `${qty(num(v.bags_issued))} bags`)}${row('<b>Left in store</b>', `<b class="${(left ?? 0) < 0 ? 'bad-text' : ''}">${qty(left ?? 0)} bags</b>`, 'pl-total')}</div>` : ''}
        <p class="muted small">Saving again for the same day changes that day's record. Changes are logged.</p>`;
    },
    save: async (v) => {
      await call('pb_save_daily', { form: { batch_id: id, day: v.day, deaths: int(v.deaths), cause: v.cause, disposed: v.disposed !== '0', sick: int(v.sick), removed: int(v.removed), added: int(v.added),
        feed_id: v.feed_id ? Number(v.feed_id) : null, bags_issued: v.feed_id ? num(v.bags_issued) : 0, kg_eaten: num(v.kg_eaten), kg_wasted: num(v.kg_wasted), water_l: num(v.water_l),
        feed_unusual: v.feed_unusual, observations: v.observations, problems: v.problems, tomorrow: v.tomorrow } });
      toast('Record saved'); ctx.go('batches', { open: id });
    },
  });
}

function feedForm(ctx: ViewCtx, home: Home, back: () => void) {
  const feeds = home.feeds;
  ask(ctx, {
    title: 'Feed store', lead: 'Record feed you bought, or feed that got damaged. The store, days left and restocking alerts update at once.', button: 'Save', back,
    sections: [
      { title: 'Which feed?', qs: [
        { name: 'feed', label: 'Which feed?', type: 'select', options: [...feeds.map((f) => [String(f.id), `${f.name} (${qty(f.available)} bags now)`] as [string, string]), ['new', '+ A new type of feed']], value: feeds[0] ? String(feeds[0].id) : 'new' },
        { name: 'name', label: 'Name of the feed', hint: 'e.g. Broiler starter, Broiler finisher', when: (v) => v.feed === 'new' },
        { name: 'brand', label: 'Brand', hint: 'e.g. Top Feeds, Vital, Chikun', when: (v) => v.feed === 'new' },
        { name: 'bag_kg', label: 'Weight of one bag (kg)', type: 'number', value: 25, when: (v) => v.feed === 'new' },
        { name: 'min_bags', label: 'Warn me when the store is down to how many bags?', type: 'number', when: (v) => v.feed === 'new' },
        { name: 'full_bags', label: 'How many bags is a full store for you?', type: 'number', hint: 'Used for "50% left" and "buy X bags".', when: (v) => v.feed === 'new' },
        { name: 'opening', label: 'How many bags of it do you have now (before this purchase)?', type: 'number', when: (v) => v.feed === 'new' },
        { name: 'opening_price', label: 'What did those bags cost each?', type: 'money', when: (v) => v.feed === 'new' && num(v.opening) > 0 },
      ] },
      { title: 'What happened?', qs: [
        { name: 'kind', label: 'What do you want to record?', type: 'chips', value: 'purchase', options: [['purchase', 'I bought feed'], ['damaged', 'Feed got damaged or lost'], ['returned', 'Unused feed came back to the store'], ['none', 'Nothing else']] },
        { name: 'move_date', label: 'When?', type: 'date', value: isoDate(new Date()), when: (v) => v.kind !== 'none' },
        { name: 'bags', label: 'How many bags?', type: 'number', when: (v) => v.kind !== 'none' },
        { name: 'price', label: 'How much did each bag cost?', type: 'money', when: (v) => v.kind === 'purchase' },
        { name: 'supplier', label: 'Who supplied the feed?', when: (v) => v.kind === 'purchase' },
        { name: 'paid', label: 'How much have you paid?', type: 'money', hint: 'Leave the full amount if you paid everything.', when: (v) => v.kind === 'purchase' },
        { name: 'good', label: 'Is the feed in good condition?', type: 'yesno', value: 1, when: (v) => v.kind === 'purchase' },
        { name: 'receipt', label: 'Do you have a receipt?', type: 'yesno', value: 0, when: (v) => v.kind === 'purchase' },
        { name: 'note', label: 'Note', when: (v) => v.kind !== 'none' && v.kind !== 'purchase' },
      ] },
    ],
    calc: (v) => {
      const f = feeds.find((x) => String(x.id) === v.feed);
      const now = f ? f.available : num(v.opening);
      const delta = v.kind === 'purchase' || v.kind === 'returned' ? num(v.bags) : v.kind === 'damaged' ? -num(v.bags) : 0;
      const total = v.kind === 'purchase' ? Math.round(num(v.bags) * kobo(v.price)) : 0;
      const paidField = $<HTMLInputElement>('input[name=paid]');
      if (paidField && !paidField.dataset.touched) paidField.value = total ? nairaInput(total) : '';
      return `<h2>Store after this</h2><div class="pl-group">${row('Bags now', qty(now))}${delta ? row(delta > 0 ? 'Adding' : 'Taking out', qty(Math.abs(delta))) : ''}${row('<b>Bags after</b>', `<b>${qty(now + delta)}</b>`, 'pl-total')}</div>
        ${total ? `<div class="pl-group">${row('Total cost', naira(total))}${total - kobo(v.paid) > 0 ? row('<b>Still owed</b>', `<b class="bad-text">${naira(total - kobo(v.paid))}</b>`) : ''}</div>` : ''}`;
    },
    save: async (v) => {
      let feedId = Number(v.feed);
      if (v.feed === 'new') feedId = await call<number>('pb_save_feed', { form: { farm_id: store.farmId ?? null, name: v.name, brand: v.brand, bag_kg: num(v.bag_kg) || 25, min_bags: num(v.min_bags), full_bags: num(v.full_bags), opening_bags: num(v.opening), opening_price_kobo: kobo(v.opening_price) } });
      if (v.kind !== 'none' && num(v.bags) > 0) await call('pb_feed_move', { form: { feed_id: feedId, move_date: v.move_date, kind: v.kind, bags: num(v.bags), price_kobo: kobo(v.price), paid_kobo: kobo(v.paid), supplier: v.supplier, good_condition: v.good !== '0', has_receipt: v.receipt === '1', note: v.note } });
      toast('Feed store updated'); back();
    },
  });
  $<HTMLInputElement>('input[name=paid]')?.addEventListener('input', (e) => ((e.target as HTMLElement).dataset.touched = '1'));
}

async function healthForm(ctx: ViewCtx, id: number, planId: number | null, back: () => void) {
  const d = await call<Detail>('pb_batch', { id });
  const plan = d.batch.plan.find((p) => p.id === planId);
  const isVit = plan ? /vitamin|electrolyte/i.test(plan.name) : false;
  ask(ctx, {
    title: `Vaccine, medicine or vitamins · ${d.batch.name}`, lead: plan ? `For: ${plan.name}` : 'Never give a vaccine or medicine without recording it.', button: 'Save', back,
    sections: [
      { title: 'What was given', qs: [
        { name: 'kind', label: 'What is it?', type: 'chips', value: plan ? (isVit ? 'vitamin' : 'vaccine') : 'vaccine', options: [['vaccine', 'Vaccine'], ['medicine', 'Medicine (e.g. antibiotic)'], ['vitamin', 'Vitamins or electrolytes']] },
        { name: 'name', label: 'Name of the vaccine or medicine', value: plan ? plan.name.replace(/^Confirm\s+/i, '') : '' },
        { name: 'purpose', label: 'What is it for? (disease or condition)', value: plan?.purpose ?? '' },
        { name: 'done_date', label: 'When was it given (or when did treatment begin)?', type: 'date', value: isoDate(new Date()) },
        { name: 'end_date', label: 'When did treatment end?', type: 'date', when: (v) => v.kind === 'medicine' },
        { name: 'brand', label: 'Brand or strain', hint: 'e.g. Lasota' },
        { name: 'method', label: 'How was it given?', type: 'select', options: [['Drinking water', 'Drinking water'], ['Eye drop', 'Eye drop'], ['Injection', 'Injection'], ['Spray', 'Spray'], ['In feed', 'In feed'], ['Other', 'Other']] },
        { name: 'birds', label: 'How many birds received it?', type: 'number', value: d.batch.alive },
        { name: 'doses', label: 'How many doses did you buy?', type: 'number', when: (v) => v.kind === 'vaccine' },
        { name: 'expiry', label: 'Expiry date on the pack', type: 'date' },
        { name: 'cost', label: 'How much did it cost?', type: 'money' },
        { name: 'given_by', label: 'Who gave it?' },
      ] },
      { title: 'After', qs: [
        { name: 'success', label: 'Was it given successfully?', type: 'yesno', value: 1 },
        { name: 'reaction', label: 'Any unusual reaction?', type: 'area' },
        { name: 'withdrawal_days', label: 'Withdrawal period on the label or from the vet (days)', type: 'number', hint: 'Birds cannot be sold or slaughtered until it ends. Leave empty if there is none.', when: (v) => v.kind !== 'vaccine' },
        { name: 'next_due', label: 'When is the next dose due?', type: 'date' },
      ] },
    ],
    calc: (v) => {
      const end = v.kind === 'medicine' && v.end_date ? v.end_date : v.done_date;
      const days = int(v.withdrawal_days);
      if (!days || !end) return '<p class="muted small">FarmBuddy never guesses a withdrawal period. Use the product label or your vet.</p>';
      const dt = new Date(end); dt.setDate(dt.getDate() + days);
      return `<div class="callout callout-warn">${icon('alert', 16)}<span>No selling or slaughter before <b>${esc(dateLabel(isoDate(dt)))}</b>.</span></div>`;
    },
    save: async (v) => {
      await call('pb_save_health', { form: { batch_id: id, plan_id: planId, kind: v.kind, name: v.name, purpose: v.purpose, done_date: v.done_date, end_date: v.kind === 'medicine' ? v.end_date : '',
        brand: v.brand, method: v.method, doses: int(v.doses), birds: int(v.birds), expiry: v.expiry, cost_kobo: kobo(v.cost), given_by: v.given_by, success: v.success !== '0', reaction: v.reaction,
        withdrawal_days: v.kind === 'vaccine' ? 0 : int(v.withdrawal_days), next_due: v.next_due } });
      toast('Saved'); back();
    },
  });
}

async function weighForm(ctx: ViewCtx, id: number, back: () => void) {
  const d = await call<Detail>('pb_batch', { id });
  const b = d.batch;
  ask(ctx, {
    title: `Weigh birds · ${b.name}`, lead: 'Weigh a few birds (10 or more) once a week.', button: 'Save weighing', back,
    sections: [{ title: 'The weighing', qs: [
      { name: 'weigh_date', label: 'When did you weigh?', type: 'date', value: isoDate(new Date()) },
      { name: 'sampled', label: 'How many birds did you weigh?', type: 'number' },
      { name: 'total_kg', label: 'Total weight of those birds (kg)', type: 'number' },
      { name: 'smaller', label: 'Are some birds much smaller than others?', type: 'chips', value: '', options: [['', 'No'], ['A few', 'A few'], ['Many', 'Many']] },
      { name: 'notes', label: 'Any growth problem you noticed?', type: 'area', wide: true },
    ] }],
    calc: (v) => {
      const avg = int(v.sampled) > 0 ? num(v.total_kg) / int(v.sampled) : 0;
      return `<h2>Result</h2><div class="pl-group">${row('Average weight', avg ? `<b>${qty(Math.round(avg * 1000) / 1000)} kg</b>` : '—')}${b.latest_avg_kg !== null ? row('Last weighing', `${qty(b.latest_avg_kg)} kg`) : ''}
        ${avg && b.latest_avg_kg !== null ? row('Gained', `${qty(Math.round((avg - b.latest_avg_kg) * 1000) / 1000)} kg`) : ''}</div>`;
    },
    save: async (v) => { await call('pb_save_weight', { form: { batch_id: id, weigh_date: v.weigh_date, sampled: int(v.sampled), total_kg: num(v.total_kg), smaller: v.smaller, notes: v.notes } }); toast('Saved'); back(); },
  });
}

function expenseForm(ctx: ViewCtx, id: number, back: () => void) {
  ask(ctx, {
    title: 'Money spent', lead: auth.can('owner') ? 'Every naira spent on this batch.' : 'Every naira spent on this batch. The owner approves it.', button: 'Save', back,
    sections: [{ title: 'The expense', qs: [
      { name: 'category', label: 'What kind of expense is it?', type: 'select', options: cats.filter((c) => c !== 'Feed' && c !== 'Purchase of chicks').map((c) => [c, c]), hint: 'Feed is recorded under “Buy feed”; chicks on the batch.' },
      { name: 'item', label: 'What did you buy or pay for?' },
      { name: 'spent_date', label: 'When?', type: 'date', value: isoDate(new Date()) },
      { name: 'quantity', label: 'How many?', type: 'number', value: 1 },
      { name: 'unit_price', label: 'How much for one?', type: 'money' },
      { name: 'bought_by', label: 'Who made the purchase?' },
      { name: 'supplier', label: 'Who did you buy from?' },
      { name: 'paid', label: 'How much was paid?', type: 'money' },
      { name: 'method', label: 'How was it paid?', type: 'select', options: [['Cash', 'Cash'], ['Bank transfer', 'Bank transfer'], ['POS / card', 'POS / card'], ['Not paid yet', 'Not paid yet']] },
      { name: 'receipt', label: 'Do you have a receipt?', type: 'yesno', value: 0 },
    ] }],
    calc: (v) => { const t = Math.round(num(v.quantity || '1') * kobo(v.unit_price)); return `<h2>Total</h2><div class="pl-group">${row('Amount', `<b>${naira(t)}</b>`)}${t - kobo(v.paid) > 0 ? row('Still owed', `<b class="bad-text">${naira(t - kobo(v.paid))}</b>`) : ''}</div>`; },
    save: async (v) => {
      await call('pb_save_expense', { form: { batch_id: id, spent_date: v.spent_date, category: v.category, item: v.item, quantity: num(v.quantity) || 1, unit_price_kobo: kobo(v.unit_price), paid_kobo: v.method === 'Not paid yet' ? 0 : kobo(v.paid), method: v.method, bought_by: v.bought_by, supplier: v.supplier, has_receipt: v.receipt === '1' } });
      toast('Saved'); back();
    },
  });
}

async function saleForm(ctx: ViewCtx, id: number, reservation: number | null, back: () => void) {
  const d = await call<Detail>('pb_batch', { id });
  const b = d.batch;
  const r = reservation ? d.sales.find((x) => x.id === reservation) : undefined;
  const methods = await call<{ name: string; active: boolean }[]>('list_payment_methods').catch(() => [{ name: 'Cash', active: true }]);
  const free = b.alive - b.reserved_birds + (r ? n(r.birds) : 0);
  ask(ctx, {
    title: `Sell birds · ${b.name}`, lead: `${plural(free, 'bird')} free to sell${b.reserved_birds && !r ? ` (${b.reserved_birds} reserved)` : ''}.`, button: 'Save', back,
    sections: [
      { title: 'The sale', qs: [
        { name: 'status', label: 'Is this a sale now, or are you holding birds for a buyer?', type: 'chips', value: 'sold', options: r ? [['sold', 'Sold now']] : [['sold', 'Sold now'], ['reserved', 'Reserve for a buyer']] },
        { name: 'sale_date', label: 'Date', type: 'date', value: isoDate(new Date()) },
        { name: 'birds', label: 'How many birds?', type: 'number', value: r ? r.birds : '' },
        { name: 'price_per', label: 'Selling per bird or per kg?', type: 'chips', value: r ? s(r.price_per) : b.price_per, options: [['bird', 'Per bird'], ['kg', 'Per kg']] },
        { name: 'avg_kg', label: 'Average weight of one bird (kg)', type: 'number', value: r ? r.avg_kg : b.latest_avg_kg ?? '', when: (v) => v.price_per === 'kg' },
        { name: 'price', label: 'Selling price (per bird or per kg)', type: 'money', value: r ? r.price_kobo : b.money_hidden ? '' : b.expected_price_kobo },
      ] },
      { title: 'The buyer and payment', qs: [
        { name: 'buyer', label: 'Who bought the birds?', value: r ? r.buyer : '' },
        { name: 'phone', label: "Buyer's phone number", value: r ? r.phone : '' },
        { name: 'method', label: 'How did they pay?', type: 'chips', value: methods[0]?.name ?? 'Cash', options: [...methods.filter((m) => m.active !== false).map((m) => [m.name, m.name] as [string, string]), ['', 'Not paid yet']], when: (v) => v.status === 'sold' },
        { name: 'paid', label: 'How much did they pay now?', type: 'money', when: (v) => v.status === 'sold' && v.method !== '' },
        { name: 'evidence', label: 'Evidence of payment (transfer reference, etc.)', when: (v) => v.status === 'sold' },
        { name: 'delivery', label: 'How were the birds delivered?', type: 'select', value: r ? r.delivery : 'Buyer collected', options: [['Buyer collected', 'Buyer collected'], ['We delivered', 'We delivered'], ['Transport hired', 'Transport hired']] },
        { name: 'delivery_cost', label: 'Delivery cost', type: 'money', value: r ? r.delivery_cost_kobo : '' },
      ] },
    ],
    calc: (v) => {
      const birds = int(v.birds); const price = kobo(v.price);
      const total = v.price_per === 'kg' ? Math.round(birds * num(v.avg_kg) * price) : birds * price;
      const paidField = $<HTMLInputElement>('input[name=paid]');
      if (paidField && !paidField.dataset.touched) paidField.value = total ? nairaInput(total) : '';
      const paid = v.method === '' ? 0 : Math.min(total, kobo(v.paid));
      return `<h2>This sale</h2><div class="pl-group">${row('Free to sell now', qty(free))}${row('Selling', qty(birds))}${row('<b>Left after</b>', `<b class="${birds > free ? 'bad-text' : ''}">${qty(free - birds)}</b>`, 'pl-total')}</div>
        <div class="pl-group">${row('Total', `<b>${naira(total)}</b>`)}${v.status === 'sold' ? row('Paid now', naira(paid)) + (total - paid > 0 ? row('<b>Still owed</b>', `<b class="bad-text">${naira(total - paid)}</b>`) : '') : ''}</div>
        ${!b.eligible ? `<div class="callout callout-bad">${icon('alert', 16)}<span>Medicine withdrawal: no sale or slaughter before ${esc(dateLabel(b.withdrawal_until))}.</span></div>` : ''}`;
    },
    save: async (v) => {
      const res = await call<{ receipt_id: number | null; receipt_no: string }>('pb_save_sale', { form: { batch_id: id, reservation_id: reservation, status: v.status, sale_date: v.sale_date, birds: int(v.birds), price_per: v.price_per, avg_kg: num(v.avg_kg), price_kobo: kobo(v.price),
        paid_kobo: v.method === '' ? 0 : kobo(v.paid), method: v.method, buyer: v.buyer, phone: v.phone, delivery: v.delivery, delivery_cost_kobo: kobo(v.delivery_cost), evidence: v.evidence } });
      if (!res.receipt_id) { toast('Birds reserved'); back(); return; }
      ctx.header('Sale saved', `Receipt ${res.receipt_no}`);
      ctx.main.innerHTML = `<section class="panel done-panel"><div class="done-icon">${icon('check', 30)}</div><h2>Sale saved · receipt ${esc(res.receipt_no)}</h2><p>The birds are taken off the batch and the money is in the profit and loss.</p>
        <div class="done-actions"><button class="btn btn-ghost" data-p>${icon('print', 16)} Print receipt</button><button class="btn btn-ghost" data-w>${icon('chat', 16)} WhatsApp receipt</button><button class="btn btn-primary" data-b>${icon('bird', 16)} Back to the batch</button></div></section>`;
      const get = () => call<Receipt>('get_receipt', { receiptId: res.receipt_id });
      $('[data-p]', ctx.main)!.addEventListener('click', async () => printReceipt(await get()));
      $('[data-w]', ctx.main)!.addEventListener('click', async () => { const rc = await get(); await shareWhatsApp(rc.customer_phone, receiptText(rc)); });
      $('[data-b]', ctx.main)!.addEventListener('click', back);
    },
  });
  $<HTMLInputElement>('input[name=paid]')?.addEventListener('input', (e) => ((e.target as HTMLElement).dataset.touched = '1'));
}

function limitsForm(ctx: ViewCtx, l: Limits, back: () => void) {
  ask(ctx, {
    title: 'Alert limits', lead: 'When FarmBuddy should warn you. Only the owner can change these.', button: 'Save', back,
    sections: [{ title: 'Feed store', qs: [
      { name: 'warn_pct', label: '"Feed stock reducing" when the store is at or below (%)', type: 'number', value: l.warn_pct },
      { name: 'low_pct', label: '"Restocking required" at or below (%)', type: 'number', value: l.low_pct },
      { name: 'critical_days', label: '"Critical purchase reminder" when feed will finish within (days)', type: 'number', value: l.critical_days },
    ] }, { title: 'Birds', qs: [
      { name: 'mortality_pct', label: 'Warn me when deaths in one day are more than (% of birds)', type: 'number', value: l.mortality_pct || '', hint: 'Leave empty for no warning. Ask your vet what is normal for your farm.' },
      { name: 'weigh_days', label: 'Remind to weigh every (days)', type: 'number', value: l.weigh_days },
    ] }],
    save: async (v) => { await call('pb_save_limits', { limits: { warn_pct: num(v.warn_pct), low_pct: num(v.low_pct), critical_days: num(v.critical_days), mortality_pct: num(v.mortality_pct), weigh_days: int(v.weigh_days) || 7 } }); toast('Saved'); back(); },
  });
}
