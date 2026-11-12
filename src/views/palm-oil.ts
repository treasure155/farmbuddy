import { auth, call } from '../api';
import { $, $$, confirmDialog, emptyState, errText, esc, toast } from '../lib/dom';
import { dateLabel, isoDate, money, nairaInput, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { printReceipt } from '../lib/receipt';
import { receiptText, shareWhatsApp } from '../lib/whatsapp';
import { store } from '../store';
import type { Receipt } from '../types';
import type { ViewCtx } from './shell';

// ------------------------------------------------------------------ types (from palm_oil.rs)
interface Measure { unit: string; per_stock_unit: number; price_kobo: number; is_stock_unit: boolean }
interface ProductOption {
  key: string; label: string; kind: 'process' | 'harvest'; raw: string; raw_units: string[]; main: string; second: string | null;
  measures: Measure[]; second_measures: Measure[]; saved: boolean;
}
interface Qty { unit: string; quantity: number }
interface CostLine { category: string; label: string; detail: string; amount_kobo: number }
interface SaleLine { id: number; part: string; sale_date: string; unit: string; quantity: number; stock_qty: number; price_kobo: number; amount_kobo: number; transport_kobo: number; other_kobo: number; buyer: string; receipt_id: number | null; receipt_no: string }
interface Run {
  id: number; run_no: string; product: string; kind: 'process' | 'harvest'; product_label: string; raw_label: string; main_label: string; second_label: string;
  title: string; farm_id: number | null; farm_name: string; start_date: string; end_date: string; status: 'open' | 'closed';
  oil_unit: string; kernel_unit: string; expected_oil_price_kobo: number; expected_kernel_price_kobo: number; notes: string; is_demo: boolean;
  fruit: Qty[]; fruit_own: Qty[]; fruit_cost_kobo: number; avg_fruit_price_kobo: number | null; costs: CostLine[]; labour_workers: number; selling_costs_kobo: number; total_cost_kobo: number;
  oil_made: number; kernel_made: number; oil_sold: number; kernel_sold: number; oil_left: number; kernel_left: number;
  oil_sales_kobo: number; kernel_sales_kobo: number; total_sales_kobo: number; profit_kobo: number;
  unsold_value_kobo: number; projected_profit_kobo: number; cost_per_oil_unit_kobo: number | null; oil_per_fruit_unit: number | null; fruit_unit: string;
  sales: SaleLine[]; money_hidden?: boolean;
}
interface Named { name: string; amount_kobo: number }
interface Form {
  id?: number | null; product?: string; start_date?: string; farm_id?: number | null; source?: string; raw_unit?: string; raw_price_kobo?: number; bought_qty?: number; own_qty?: number;
  transport_kobo?: number; mill_name?: string; mill_fee_kobo?: number; workers?: Named[]; expenses?: Named[]; plot?: string;
  made_qty?: number; made_unit?: string; second_qty?: number; second_unit?: string; notes?: string;
}

let products: ProductOption[] | null = null;
const loadProducts = async (fresh = false) => (fresh || !products ? (products = await call<ProductOption[]>('process_products')) : products);

const plural = (u: string) => (/s$|\(ete\)$/.test(u) ? u : /(ch|sh)$/.test(u) ? `${u}es` : `${u}s`);
/** "4 bags", "1 cup", "2 × 25-litre keg". */
const amount = (n: number, unit: string) => (/^\d/.test(unit) ? `${qty(n)} × ${unit}` : `${qty(n)} ${n === 1 ? unit : plural(unit)}`);
const qtyList = (list: Qty[]) => list.map((q) => amount(q.quantity, q.unit)).join(' + ');
const naira = (kobo: number) => money(kobo, { whole: true });
const round = (n: number) => Math.round(n * 1000) / 1000;
const runIcon = (r: { product: string; kind: string }) => icon(r.product === 'palm_oil' ? 'drop' : r.kind === 'harvest' ? 'fruit' : 'mill', 20);

export async function render(ctx: ViewCtx) {
  if (ctx.params.sell !== undefined) return renderSale(ctx, Number(ctx.params.sell) || null);
  if (ctx.params.edit !== undefined) return renderForm(ctx, ctx.params.edit === 'new' ? null : Number(ctx.params.edit), String(ctx.params.product ?? ''), String(ctx.params.kind ?? 'process'));
  if (typeof ctx.params.open === 'number') return renderRun(ctx, ctx.params.open);
  return renderHome(ctx);
}

// ------------------------------------------------------------------ home: three plain choices and the barn

async function renderHome(ctx: ViewCtx) {
  const manager = auth.can('manager');
  const seller = auth.canSell();
  ctx.header('Runs and sales', 'Every run fills your barn. Every sale takes from it. When a run is sold out, its profit or loss is final.');
  const runs = (await call<Run[]>('list_palm_runs')).filter((r) => store.inFarm(r.farm_id));
  const open = runs.filter((r) => r.status === 'open');
  const done = runs.filter((r) => r.status === 'closed');
  const choice = (attr: string, ic: string, title: string, hint: string) => `<button type="button" class="big-choice" ${attr}><span>${icon(ic, 26)}</span><span class="bc-text"><b>${title}</b><small>${hint}</small></span></button>`;
  ctx.main.innerHTML = `
    <section class="big-choices">
      ${manager ? choice('data-start="process"', 'mill', 'Start a processing run', 'Palm oil, garri, fufu, cassava flour') : ''}
      ${manager ? choice('data-start="harvest"', 'fruit', 'Record a harvest', 'Cucumber, pepper, vegetables, melon, yam, ete') : ''}
      ${seller ? choice('data-sell', 'cash', 'Record a sale', open.length ? 'Sell from what is in your barn' : 'Start a run first') : ''}
    </section>
    <section class="stack"><h2 class="section-title">In your barn</h2>
      ${open.length ? `<div class="run-grid">${open.map(runCard).join('')}</div>`
        : emptyState('Your barn is empty', 'Start a processing run or record a harvest. What you get goes into your barn, ready to sell.')}
    </section>
    ${done.length ? `<section class="stack"><h2 class="section-title">Finished runs</h2><div class="run-grid">${done.map(runCard).join('')}</div></section>` : ''}`;
  ctx.main.onclick = (e) => {
    const el = e.target as HTMLElement;
    const start = el.closest<HTMLElement>('[data-start]');
    if (start) { ctx.go('palm-oil', { edit: 'new', kind: start.dataset.start! }); return; }
    if (el.closest('[data-sell]')) {
      if (!open.length) { toast('Nothing to sell yet. Start a run or record a harvest first.', 'error'); return; }
      ctx.go('palm-oil', { sell: '' }); return;
    }
    const o = el.closest<HTMLElement>('[data-run]');
    if (o) ctx.go('palm-oil', { open: Number(o.dataset.run) });
  };
}

function runCard(r: Run) {
  const showMoney = !r.money_hidden && auth.can('manager');
  const left = [r.oil_made ? amount(round(r.oil_left), r.oil_unit) : '', r.second_label && r.kernel_left > 1e-9 ? `${amount(round(r.kernel_left), r.kernel_unit)} kernel` : ''].filter(Boolean).join(' · ') || '—';
  const pl = r.status === 'closed' ? r.profit_kobo : r.projected_profit_kobo;
  return `<button type="button" class="run-card ${r.status === 'closed' ? 'is-closed' : ''}" data-run="${r.id}">
    <span class="run-head"><span class="run-icon">${runIcon(r)}</span>
      <span class="run-name"><b>${esc(r.product_label)}</b><small>${esc(r.run_no)} · ${esc(dateLabel(r.start_date))}</small></span>
      <span class="run-pills">${r.is_demo ? '<span class="pill pill-warn">Demo</span>' : ''}<span class="pill ${r.status === 'open' ? 'pill-ok' : 'pill-mute'}">${r.status === 'open' ? 'Selling' : 'Sold out'}</span></span></span>
    <span class="run-tiles">
      <span><small>Got</small><b>${r.oil_made ? esc(amount(round(r.oil_made), r.oil_unit)) : '—'}</b></span>
      <span><small>${r.status === 'open' ? 'Left in barn' : 'Sold'}</small><b>${esc(r.status === 'open' ? left : amount(round(r.oil_sold), r.oil_unit))}</b></span>
      ${showMoney ? `<span class="run-pl ${pl < 0 ? 'is-loss' : 'is-profit'}"><small>${r.status === 'closed' ? (pl < 0 ? 'Loss' : 'Profit') : pl < 0 ? 'Loss if all sold' : 'Profit if all sold'}</small><b>${naira(Math.abs(pl))}</b></span>` : ''}
    </span>
  </button>`;
}

// ------------------------------------------------------------------ the run form (questions about the run only)

async function renderForm(ctx: ViewCtx, id: number | null, productKey: string, kindParam: string) {
  const opts = await loadProducts(true);
  let f: Form = {};
  if (id) f = (await call<{ form: Form }>('get_palm_run', { id })).form;
  const wanted = opts.find((o) => o.key === (f.product || productKey));
  const kind = wanted?.kind ?? (kindParam === 'harvest' ? 'harvest' : 'process');
  const p = wanted ?? opts.find((o) => o.kind === kind)!;
  const harvest = p.kind === 'harvest';
  const isPalm = p.key === 'palm_oil';
  const raw = p.raw;
  ctx.header(id ? 'Change this run' : harvest ? 'Record a harvest' : 'Start a processing run',
    harvest ? 'A few questions about this harvest. Sales are recorded separately, each time you sell.' : 'Questions about this run only. Sales are recorded separately, each time you sell.');

  const kv = (k: number | undefined) => (k ? nairaInput(k) : '');
  const nv = (n: number | undefined) => (n ? String(n) : '');
  const moneyQ = (name: string, label: string, value: number | undefined, hint = '') => `<label class="q"><span class="q-text">${label}</span><span class="naira-input"><i>₦</i><input name="${name}" inputmode="decimal" value="${esc(kv(value))}" placeholder="0"></span>${hint ? `<small>${hint}</small>` : ''}</label>`;
  const number = (name: string, label: string, value: number | undefined, hint = '') => `<label class="q"><span class="q-text">${label}</span><input name="${name}" inputmode="decimal" value="${esc(nv(value))}" placeholder="0">${hint ? `<small>${hint}</small>` : ''}</label>`;
  const chips = (name: string, items: [string, string][], cur: string) => `<div class="chip-choice">${items.map(([k, l]) => `<label><input type="radio" name="${name}" value="${k}" ${cur === k ? 'checked' : ''}><span>${esc(l)}</span></label>`).join('')}</div>`;
  const rawUnit = f.raw_unit || p.raw_units[0] || '';
  const listRow = (kindName: 'workers' | 'expenses', x?: Named) => `<div class="list-row" data-list-row="${kindName}">
    <input data-name maxlength="80" placeholder="${kindName === 'workers' ? 'Name, e.g. Arit' : 'What, e.g. firewood'}" value="${esc(x?.name ?? '')}" aria-label="${kindName === 'workers' ? 'Worker name' : 'Expense'}">
    <span class="naira-input"><i>₦</i><input data-amount inputmode="decimal" placeholder="0" value="${esc(kv(x?.amount_kobo))}" aria-label="How much"></span>
    <button type="button" class="icon-btn" data-remove aria-label="Remove">${icon('close', 16)}</button></div>`;
  const listBlock = (kindName: 'workers' | 'expenses', title: string, hint: string, items: Named[] = []) => `<div class="q"><span class="q-text">${title}</span><small>${hint}</small>
    <div class="list-rows" data-list="${kindName}">${(items.length ? items : [undefined]).map((x) => listRow(kindName, x)).join('')}</div>
    <button type="button" class="btn btn-ghost btn-sm" data-add="${kindName}">${icon('plus', 14)} ${kindName === 'workers' ? 'Add a worker' : 'Add another expense'}</button></div>`;

  const measureTable = (part: 'main' | 'second', label: string, list: Measure[]) => {
    const stock = list.find((m) => m.is_stock_unit)?.unit ?? list[0]?.unit ?? '';
    return `<div class="measure-block" data-measures="${part}">
      <p class="q-text">How do you sell ${esc(label.toLowerCase())} in your area? <small>Tick every way you sell it and put today's price. Tell us the size once; FarmBuddy remembers it.</small></p>
      <label class="q q-inline"><span class="q-text">You count your ${esc(label.toLowerCase())} in</span><select data-stock-unit>${list.map((m) => `<option ${m.unit === stock ? 'selected' : ''}>${esc(m.unit)}</option>`).join('')}</select></label>
      <div class="measure-table" role="table">
        <div class="measure-head" role="row"><span>Sell by</span><span>How many make 1 <b data-stock-name>${esc(stock)}</b>?</span><span>Price of one now</span></div>
        ${list.map((m) => measureRow(m, p.saved || part === 'second' ? true : m.is_stock_unit || !!m.price_kobo)).join('')}
      </div>
      <div class="measure-add"><input data-new-measure maxlength="30" placeholder="Another measure, e.g. mudu" aria-label="Another measure"><button type="button" class="btn btn-ghost btn-sm" data-add-measure>${icon('plus', 14)} Add</button></div>
    </div>`;
  };
  const measureRow = (m: Measure, on: boolean) => `<div class="measure-row ${on ? '' : 'is-off'}" role="row" data-unit="${esc(m.unit)}">
    <label class="check"><input type="checkbox" data-on ${on ? 'checked' : ''}><span>${esc(m.unit)}</span></label>
    <span class="measure-size"><input data-per inputmode="decimal" placeholder="?" value="${m.per_stock_unit ? esc(String(m.per_stock_unit)) : ''}" aria-label="How many ${esc(m.unit)} make one"><i data-is-stock>= 1</i></span>
    <span class="naira-input"><i>₦</i><input data-price inputmode="decimal" placeholder="0" value="${esc(kv(m.price_kobo))}" aria-label="Price of one ${esc(m.unit)}"></span></div>`;
  const madeQ = (part: 'main' | 'second', label: string, q: number | undefined, unit: string | undefined) => `<div class="q-row">
      ${number(part === 'main' ? 'made_qty' : 'second_qty', harvest ? `How much ${esc(label.toLowerCase())} did you harvest?` : `How much ${esc(label.toLowerCase())} did you get?`, q)}
      <label class="q"><span class="q-text">Measured in</span><select name="${part === 'main' ? 'made_unit' : 'second_unit'}" data-unit-select="${part}" data-cur="${esc(unit ?? '')}"></select></label></div>`;

  let step = 0;
  const h = (t: string) => `<h2><span>${++step}</span>${t}</h2>`;
  const kindProducts = opts.filter((o) => o.kind === p.kind);
  ctx.main.innerHTML = `<form class="run-form" novalidate>
    <div class="run-form-main">
      <section class="qs">
        ${h(harvest ? 'What did you harvest?' : 'What are you making?')}
        ${chips('product_pick', kindProducts.map((o) => [o.key, o.label]), p.key)}
        <label class="q"><span class="q-text">${harvest ? 'When did you harvest?' : 'When did you start?'}</span><input type="date" name="start_date" value="${esc(f.start_date || isoDate(new Date()))}"></label>
        ${harvest ? `<label class="q"><span class="q-text">Which farm or plot? <em>(optional)</em></span><input name="plot" maxlength="120" value="${esc(f.plot ?? '')}" placeholder="e.g. Behind the house"></label>` : ''}
      </section>

      ${harvest ? '' : `<section class="qs">
        ${h(`The ${esc(raw)}`)}
        <div class="q"><span class="q-text">Did you buy the ${esc(raw)}, or take it from your farm?</span>${chips('source', [['bought', 'I bought it'], ['own', 'From my farm'], ['both', 'Both']], f.source || 'bought')}</div>
        <label class="q"><span class="q-text">How do you buy ${esc(raw)}?</span><select name="raw_unit">${p.raw_units.map((u) => `<option ${u === rawUnit ? 'selected' : ''}>${esc(u)}</option>`).join('')}</select></label>
        <div class="q-row" data-when="bought">
          ${moneyQ('raw_price_kobo', `How much is one <b data-ru1>${esc(rawUnit)}</b>?`, f.raw_price_kobo)}
          ${number('bought_qty', `How many <b data-ru>${esc(plural(rawUnit))}</b> did you buy?`, f.bought_qty)}
        </div>
        <p class="q-sum" data-when="bought" data-sum="buy"></p>
        <div class="q-row" data-when="own">${number('own_qty', `How many <b data-ru>${esc(plural(rawUnit))}</b> did you take from your farm?`, f.own_qty)}</div>
      </section>`}

      <section class="qs">
        ${h(harvest ? 'What it cost you' : 'Processing and other costs')}
        ${moneyQ('transport_kobo', harvest ? 'How much was transport?' : `How much was your transport?`, f.transport_kobo, harvest ? 'Carrying it home or to the market. Enter 0 if none.' : `Carrying the ${raw} to processing. Enter 0 if none.`)}
        ${harvest ? '' : `<div class="q-row">
          <label class="q"><span class="q-text">Which mill did the processing?</span><input name="mill_name" maxlength="120" value="${esc(f.mill_name ?? '')}" placeholder="e.g. Ikot Ekpene mill, or My own"></label>
          ${moneyQ('mill_fee_kobo', 'How much was the processing fee?', f.mill_fee_kobo, isPalm ? 'Mill or press fee.' : 'Peeling, grating, pressing, frying or milling.')}
        </div>`}
        ${listBlock('workers', 'Workers, if any', 'Each worker\'s name and how much you paid them. Leave empty if none.', f.workers)}
        ${listBlock('expenses', 'Any other expenses?', isPalm ? 'e.g. firewood, water, empty kegs.' : harvest ? 'e.g. bags, baskets, levy.' : 'e.g. firewood, palm oil for yellow garri, bags.', f.expenses)}
        <p class="q-sum" data-sum="lists"></p>
      </section>

      <section class="qs">
        ${h('How you sell it')}
        ${measureTable('main', p.main, p.measures)}
        ${p.second ? measureTable('second', p.second, p.second_measures) : ''}
      </section>

      <section class="qs">
        ${h(harvest ? 'How much you harvested' : 'What you got')}
        ${madeQ('main', p.main, f.made_qty, f.made_unit)}
        ${p.second ? madeQ('second', p.second, f.second_qty, f.second_unit) : ''}
        <p class="q-sum" data-sum="made"></p>
        <label class="q"><span class="q-text">Anything else to note? <em>(optional)</em></span><textarea name="notes" rows="2" maxlength="1000">${esc(f.notes ?? '')}</textarea></label>
      </section>
    </div>

    <aside class="run-form-result" aria-live="polite">
      <h2>This run</h2>
      <div data-result></div>
      <p class="form-error" role="alert" data-err></p>
      <button class="btn btn-primary btn-lg btn-block">${icon('check', 18)} ${id ? 'Save changes' : harvest ? 'Save harvest' : 'Save run'}</button>
      <button type="button" class="btn btn-ghost btn-block" data-cancel>Cancel</button>
      <p class="muted small">What you got goes into your barn. Record each sale with “Record a sale”.</p>
    </aside>
  </form>`;

  const form = $<HTMLFormElement>('.run-form', ctx.main)!;
  const val = (name: string) => (form.elements.namedItem(name) as HTMLInputElement | null)?.value ?? '';
  const num = (s: string) => Number(String(s).replace(/,/g, '')) || 0;
  const pick = (name: string) => (form.querySelector(`input[name=${name}]:checked`) as HTMLInputElement | null)?.value ?? '';
  const k = (name: string) => toKobo(val(name)) || 0;
  const list = (kindName: string): Named[] => $$<HTMLElement>(`[data-list="${kindName}"] .list-row`, form)
    .map((row) => ({ name: $<HTMLInputElement>('[data-name]', row)!.value.trim(), amount_kobo: toKobo($<HTMLInputElement>('[data-amount]', row)!.value) || 0 }))
    .filter((x) => x.name || x.amount_kobo);
  const readMeasures = (part: string): Measure[] => {
    const block = $<HTMLElement>(`[data-measures="${part}"]`, form);
    if (!block) return [];
    const stock = $<HTMLSelectElement>('[data-stock-unit]', block)!.value;
    return $$<HTMLElement>('.measure-row', block).filter((row) => $<HTMLInputElement>('[data-on]', row)!.checked || row.dataset.unit === stock).map((row) => ({
      unit: row.dataset.unit!, is_stock_unit: row.dataset.unit === stock,
      per_stock_unit: row.dataset.unit === stock ? 1 : num($<HTMLInputElement>('[data-per]', row)!.value),
      price_kobo: toKobo($<HTMLInputElement>('[data-price]', row)!.value) || 0,
    }));
  };
  const toStock = (ms: Measure[], unit: string, q: number) => { const m = ms.find((x) => x.unit === unit); return m && m.per_stock_unit ? q / m.per_stock_unit : 0; };

  const draw = () => {
    const source = pick('source');
    $$<HTMLElement>('[data-when]', form).forEach((el) => (el.hidden = !(source === 'both' || source === el.dataset.when)));
    $$('[data-ru]', form).forEach((el) => (el.textContent = plural(val('raw_unit'))));
    $$('[data-ru1]', form).forEach((el) => (el.textContent = val('raw_unit')));
    for (const part of ['main', 'second']) {
      const block = $<HTMLElement>(`[data-measures="${part}"]`, form);
      if (!block) continue;
      const sel = $<HTMLSelectElement>('[data-stock-unit]', block)!;
      const stock = sel.value;
      $$('[data-stock-name]', block).forEach((el) => (el.textContent = stock));
      const units = $$<HTMLElement>('.measure-row', block).map((row) => row.dataset.unit!);
      if (sel.options.length !== units.length) sel.innerHTML = units.map((u) => `<option ${u === stock ? 'selected' : ''}>${esc(u)}</option>`).join('');
      $$<HTMLElement>('.measure-row', block).forEach((row) => {
        const isStock = row.dataset.unit === stock;
        const on = $<HTMLInputElement>('[data-on]', row)!;
        if (isStock) on.checked = true;
        on.disabled = isStock;
        row.classList.toggle('is-off', !on.checked);
        row.classList.toggle('is-stock', isStock);
        $<HTMLInputElement>('[data-per]', row)!.hidden = isStock;
        $<HTMLElement>('[data-is-stock]', row)!.hidden = !isStock;
      });
      const us = $<HTMLSelectElement>(`[data-unit-select="${part}"]`, form)!;
      const ms = readMeasures(part);
      const cur = us.value || us.dataset.cur || stock;
      us.innerHTML = ms.map((m) => `<option ${m.unit === cur ? 'selected' : ''}>${esc(m.unit)}</option>`).join('');
    }
    const buy = harvest || source === 'own' ? 0 : num(val('bought_qty')) * k('raw_price_kobo');
    const workers = list('workers').reduce((s, x) => s + x.amount_kobo, 0);
    const other = list('expenses').reduce((s, x) => s + x.amount_kobo, 0);
    const cost = buy + k('transport_kobo') + (harvest ? 0 : k('mill_fee_kobo')) + workers + other;
    const ms = readMeasures('main');
    const stockUnit = ms.find((m) => m.is_stock_unit)?.unit ?? '';
    const made = toStock(ms, val('made_unit'), num(val('made_qty')));
    const ms2 = readMeasures('second');
    const made2 = p.second ? toStock(ms2, val('second_unit'), num(val('second_qty'))) : 0;
    const price = ms.find((m) => m.is_stock_unit)?.price_kobo ?? 0;
    const price2 = ms2.find((m) => m.is_stock_unit)?.price_kobo ?? 0;
    const worth = made * price + made2 * price2;
    const buyEl = $('[data-sum="buy"]', form);
    if (buyEl) buyEl.textContent = buy ? `You paid ${naira(buy)} for the ${raw}.` : '';
    $('[data-sum="lists"]', form)!.textContent = [workers ? `Workers: ${naira(workers)}` : '', other ? `Other expenses: ${naira(other)}` : ''].filter(Boolean).join(' · ');
    $('[data-sum="made"]', form)!.textContent = made && val('made_unit') !== stockUnit ? `That is ${amount(round(made), stockUnit)} in your barn.` : '';
    const row = (a: string, b: number) => (b ? `<div class="pl-row"><span>${esc(a)}</span><span>${naira(b)}</span></div>` : '');
    const missing = ms.filter((m) => !m.is_stock_unit && !m.per_stock_unit).map((m) => m.unit);
    $('[data-result]', form)!.innerHTML = `
      <div class="pl-group"><p class="pl-title">You spent</p>
        ${row(`Buying the ${raw}`, buy)}${row('Transport', k('transport_kobo'))}${harvest ? '' : row('Processing fee', k('mill_fee_kobo'))}${row('Workers', workers)}${row('Other expenses', other)}
        <div class="pl-row pl-total"><b>Total spent</b><b>${naira(cost)}</b></div></div>
      ${made ? `<div class="pl-group"><p class="pl-title">Into your barn</p>
        <div class="pl-row"><span>${esc(p.main)}</span><span>${esc(amount(round(made), stockUnit))}</span></div>
        ${made2 ? `<div class="pl-row"><span>${esc(p.second ?? '')}</span><span>${esc(amount(round(made2), ms2.find((m) => m.is_stock_unit)?.unit ?? ''))}</span></div>` : ''}
        ${cost ? `<div class="pl-row"><span>Each ${esc(stockUnit)} cost you</span><span>${naira(cost / made)}</span></div>` : ''}</div>` : ''}
      ${worth ? `<div class="pl-result ${worth - cost < 0 ? 'is-loss' : 'is-profit'}"><span>${worth - cost < 0 ? 'Loss' : 'Profit'} if you sell it all at today's price</span><b>${naira(Math.abs(worth - cost))}</b></div>
        <p class="muted small">The real result comes from your sales. It is final when everything is sold.</p>` : ''}
      ${missing.length ? `<div class="callout callout-warn">${icon('alert', 16)}<span>How many ${esc(missing.map(plural).join(', '))} make one ${esc(stockUnit)}? Fill the size, or untick it.</span></div>` : ''}`;
  };

  form.addEventListener('input', (e) => {
    // Typing a size or price for a measure ticks it.
    const t = e.target as HTMLInputElement;
    const row = t.closest('.measure-row');
    if (row && t.value && (t.matches('[data-per]') || t.matches('[data-price]'))) $<HTMLInputElement>('[data-on]', row as HTMLElement)!.checked = true;
    draw();
  });
  form.addEventListener('change', (e) => {
    const t = e.target as HTMLInputElement;
    if (t.name === 'product_pick' && t.value !== p.key) { ctx.go('palm-oil', { edit: id ? String(id) : 'new', product: t.value, kind: p.kind }); return; }
    draw();
  });
  form.addEventListener('click', (e) => {
    const el = e.target as HTMLElement;
    const add = el.closest<HTMLElement>('[data-add]');
    if (add) {
      const box = $(`[data-list="${add.dataset.add}"]`, form)!;
      box.insertAdjacentHTML('beforeend', listRow(add.dataset.add as 'workers' | 'expenses'));
      ($('.list-row:last-child [data-name]', box) as HTMLInputElement).focus();
      return;
    }
    const rm = el.closest('[data-remove]');
    if (rm) {
      const row = rm.closest('.list-row')!;
      const box = row.parentElement!;
      if (box.children.length > 1) row.remove(); else row.querySelectorAll('input').forEach((i) => (i.value = ''));
      draw(); return;
    }
    const addM = el.closest('[data-add-measure]');
    if (addM) {
      const block = addM.closest<HTMLElement>('[data-measures]')!;
      const input = $<HTMLInputElement>('[data-new-measure]', block)!;
      const unit = input.value.trim().toLowerCase();
      if (!unit) { input.focus(); return; }
      if ($$<HTMLElement>('.measure-row', block).some((r) => r.dataset.unit!.toLowerCase() === unit)) { toast('That measure is already in the list'); return; }
      $('.measure-table', block)!.insertAdjacentHTML('beforeend', measureRow({ unit, per_stock_unit: 0, price_kobo: 0, is_stock_unit: false }, true));
      input.value = '';
      draw();
    }
  });
  draw();
  $('[data-cancel]', form)?.addEventListener('click', () => (id ? ctx.go('palm-oil', { open: id }) : ctx.go('palm-oil')));
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const err = $('[data-err]', form)!;
    err.textContent = '';
    const source = harvest ? '' : pick('source');
    const input = {
      id, product: p.key, start_date: val('start_date'), farm_id: id ? (f.farm_id ?? null) : (store.farmId ?? null), source,
      raw_unit: harvest ? '' : val('raw_unit'), raw_price_kobo: source === 'own' || harvest ? 0 : k('raw_price_kobo'),
      bought_qty: source === 'own' || harvest ? 0 : num(val('bought_qty')), own_qty: source === 'bought' || harvest ? 0 : num(val('own_qty')),
      transport_kobo: k('transport_kobo'), mill_name: harvest ? '' : val('mill_name'), mill_fee_kobo: harvest ? 0 : k('mill_fee_kobo'),
      workers: list('workers'), expenses: list('expenses'), plot: harvest ? val('plot') : '',
      made_qty: num(val('made_qty')), made_unit: val('made_unit'), second_qty: p.second ? num(val('second_qty')) : 0, second_unit: p.second ? val('second_unit') : '',
      measures: readMeasures('main'), second_measures: p.second ? readMeasures('second') : [], notes: val('notes'),
    };
    const btn = $<HTMLButtonElement>('button.btn-primary', form)!;
    btn.disabled = true;
    try {
      const saved = await call<number>('save_process_run', { form: input });
      products = null;
      toast(harvest ? 'Harvest saved. It is in your barn.' : 'Run saved. What you got is in your barn.');
      ctx.go('palm-oil', { open: saved });
    } catch (ex) { err.textContent = errText(ex); btn.disabled = false; }
  });
}

// ------------------------------------------------------------------ the sale form (questions about the sale only)

interface PayMethod { id: number; name: string; active: boolean }

async function renderSale(ctx: ViewCtx, runId: number | null) {
  const [runs, opts, methods] = await Promise.all([call<Run[]>('list_palm_runs'), loadProducts(true), call<PayMethod[]>('list_payment_methods').catch(() => [] as PayMethod[])]);
  const open = runs.filter((r) => r.status === 'open' && store.inFarm(r.farm_id) && (r.oil_left > 1e-9 || r.kernel_left > 1e-9));
  ctx.header('Record a sale', 'Questions about this sale only. FarmBuddy takes it out of the right run and makes a receipt.');
  if (!open.length) {
    ctx.main.innerHTML = emptyState('Nothing in your barn to sell', 'Start a processing run or record a harvest first.');
    return;
  }
  const first = open.find((r) => r.id === runId) ?? open[0];
  const pays = methods.filter((m) => m.active !== false).map((m) => m.name);
  const payList = pays.length ? pays : ['Cash'];
  ctx.main.innerHTML = `<form class="run-form sale-form" novalidate>
    <div class="run-form-main">
      <section class="qs">
        <h2><span>1</span>What are you selling?</h2>
        <label class="q"><span class="q-text">From which run?</span><select name="run_id">${open.map((r) => `<option value="${r.id}" ${r.id === first.id ? 'selected' : ''}>${esc(r.product_label)} · ${esc(r.run_no)} · ${esc(amount(round(r.oil_left), r.oil_unit))} left${r.second_label && r.kernel_left ? ` + ${esc(amount(round(r.kernel_left), r.kernel_unit))} kernel` : ''}</option>`).join('')}</select>
          <small>Choose the run this ${'produce'} came from. Many runs can be selling at once.</small></label>
        <div class="q" data-part-q><span class="q-text">Which product?</span><div class="chip-choice" data-parts></div></div>
        <label class="q"><span class="q-text">Date of the sale</span><input type="date" name="sale_date" value="${isoDate(new Date())}"></label>
      </section>
      <section class="qs">
        <h2><span>2</span>How much did you sell?</h2>
        <div class="q"><span class="q-text">How did you sell it?</span><div class="chip-choice" data-units></div></div>
        <div class="q-row">
          <label class="q"><span class="q-text">How many <b data-unit-p>?</b> did you sell?</span><input name="quantity" inputmode="decimal" placeholder="0"></label>
          <label class="q"><span class="q-text">Price of one <b data-unit-1>?</b></span><span class="naira-input"><i>₦</i><input name="price" inputmode="decimal" placeholder="0"></span></label>
        </div>
        <p class="q-sum" data-sum="sale"></p>
      </section>
      <section class="qs">
        <h2><span>3</span>The buyer and payment</h2>
        <div class="q-row">
          <label class="q"><span class="q-text">Buyer's name <em>(needed if they owe you)</em></span><input name="buyer" maxlength="120" placeholder="e.g. Mma Ekaette"></label>
          <label class="q"><span class="q-text">Buyer's phone <em>(optional)</em></span><input name="buyer_phone" inputmode="tel" maxlength="40" placeholder="for WhatsApp receipt"></label>
        </div>
        <div class="q"><span class="q-text">How did they pay?</span><div class="chip-choice">${[...payList, 'Not paid yet'].map((m, i) => `<label><input type="radio" name="method" value="${esc(m)}" ${i === 0 ? 'checked' : ''}><span>${esc(m)}</span></label>`).join('')}</div></div>
        <label class="q" data-paid-q><span class="q-text">How much did they pay now?</span><span class="naira-input"><i>₦</i><input name="paid" inputmode="decimal" placeholder="0"></span><small>If they paid part, the rest is recorded as owed.</small></label>
      </section>
      <section class="qs">
        <h2><span>4</span>Costs of this sale</h2>
        <div class="q-row">
          <label class="q"><span class="q-text">Transportation</span><span class="naira-input"><i>₦</i><input name="transport" inputmode="decimal" placeholder="0"></span><small>Carrying it to the buyer or market. 0 if none.</small></label>
          <label class="q"><span class="q-text">Miscellaneous</span><span class="naira-input"><i>₦</i><input name="other" inputmode="decimal" placeholder="0"></span><small>Market levy, loading, bags…</small></label>
        </div>
      </section>
    </div>
    <aside class="run-form-result" aria-live="polite">
      <h2>This sale</h2>
      <div data-result></div>
      <p class="form-error" role="alert" data-err></p>
      <button class="btn btn-primary btn-lg btn-block">${icon('check', 18)} Save sale and make receipt</button>
      <button type="button" class="btn btn-ghost btn-block" data-cancel>Cancel</button>
    </aside>
  </form>`;

  const form = $<HTMLFormElement>('.sale-form', ctx.main)!;
  const val = (name: string) => (form.elements.namedItem(name) as HTMLInputElement | null)?.value ?? '';
  const num = (s: string) => Number(String(s).replace(/,/g, '')) || 0;
  const pick = (name: string) => (form.querySelector(`input[name=${name}]:checked`) as HTMLInputElement | null)?.value ?? '';
  let paidTouched = false;
  let priceFor = '';

  const current = () => {
    const run = open.find((r) => r.id === Number(val('run_id')))!;
    const opt = opts.find((o) => o.key === run.product)!;
    const part = run.second_label && pick('part') === 'second' ? 'second' : 'main';
    const ms = (part === 'second' ? opt.second_measures : opt.measures).filter((m) => m.is_stock_unit || m.per_stock_unit > 0);
    const left = part === 'second' ? run.kernel_left : run.oil_left;
    const stockUnit = part === 'second' ? run.kernel_unit : run.oil_unit;
    return { run, opt, part, ms, left, stockUnit };
  };
  const drawChoices = () => {
    const { run, ms } = current();
    const parts = $('[data-parts]', form)!;
    $<HTMLElement>('[data-part-q]', form)!.hidden = !run.second_label;
    const partNow = pick('part') || 'main';
    parts.innerHTML = run.second_label ? [['main', run.main_label, run.oil_left], ['second', run.second_label, run.kernel_left]]
      .map(([k, l, left]) => `<label><input type="radio" name="part" value="${k}" ${k === partNow ? 'checked' : ''} ${Number(left) > 1e-9 ? '' : 'disabled'}><span>${esc(String(l))}</span></label>`).join('') : '';
    const unitNow = pick('unit');
    const keep = ms.some((m) => m.unit === unitNow) ? unitNow : ms[0]?.unit;
    $('[data-units]', form)!.innerHTML = ms.map((m) => `<label><input type="radio" name="unit" value="${esc(m.unit)}" ${m.unit === keep ? 'checked' : ''}><span>${esc(m.unit)}</span></label>`).join('');
  };
  const draw = () => {
    const { ms, left, stockUnit } = current();
    const unit = pick('unit');
    const m = ms.find((x) => x.unit === unit);
    $$('[data-unit-p]', form).forEach((el) => (el.textContent = plural(unit)));
    $$('[data-unit-1]', form).forEach((el) => (el.textContent = unit));
    const priceInput = form.elements.namedItem('price') as HTMLInputElement;
    if (m && priceFor !== unit) { priceInput.value = m.price_kobo ? nairaInput(m.price_kobo) : ''; priceFor = unit; }
    const q = num(val('quantity'));
    const total = Math.round(q * (toKobo(val('price')) || 0));
    const paidInput = form.elements.namedItem('paid') as HTMLInputElement;
    const notPaid = pick('method') === 'Not paid yet';
    $<HTMLElement>('[data-paid-q]', form)!.hidden = notPaid;
    if (!paidTouched) paidInput.value = total ? nairaInput(total) : '';
    const paid = notPaid ? 0 : Math.min(total, toKobo(paidInput.value) || 0);
    const stockQty = m ? q / (m.is_stock_unit ? 1 : m.per_stock_unit) : 0;
    const costs = (toKobo(val('transport')) || 0) + (toKobo(val('other')) || 0);
    $('[data-sum="sale"]', form)!.textContent = q && unit !== stockUnit ? `${amount(q, unit)} = ${amount(round(stockQty), stockUnit)} from your barn.` : '';
    const row = (a: string, b: string, cls = '') => `<div class="pl-row ${cls}"><span>${a}</span><span>${b}</span></div>`;
    const over = stockQty > left + 1e-6;
    $('[data-result]', form)!.innerHTML = `
      <div class="pl-group">
        ${row('In the barn now', esc(amount(round(left), stockUnit)))}
        ${row('You are selling', q ? esc(amount(round(stockQty), stockUnit)) : '—')}
        ${row('<b>Left after this sale</b>', `<b>${esc(amount(round(Math.max(0, left - stockQty)), stockUnit))}</b>`, 'pl-total')}</div>
      <div class="pl-group">
        ${row('Total for this sale', naira(total))}${row('Paid now', naira(paid))}${total - paid > 0 ? row('<b>Still owed</b>', `<b class="bad-text">${naira(total - paid)}</b>`) : ''}
        ${costs ? row('Selling costs', naira(costs)) : ''}</div>
      ${over ? `<div class="callout callout-warn">${icon('alert', 16)}<span>This run has only ${esc(amount(round(left), stockUnit))} left.</span></div>` : ''}
      ${stockQty && Math.abs(stockQty - left) < 1e-6 ? `<div class="callout">${icon('check', 16)}<span>This sells everything from the run. Its profit or loss will be final.</span></div>` : ''}`;
  };
  drawChoices(); draw();
  form.addEventListener('input', (e) => { if ((e.target as HTMLInputElement).name === 'paid') paidTouched = true; draw(); });
  form.addEventListener('change', (e) => {
    const t = e.target as HTMLInputElement;
    if (t.name === 'run_id' || t.name === 'part') { priceFor = ''; drawChoices(); }
    draw();
  });
  $('[data-cancel]', form)?.addEventListener('click', () => (runId ? ctx.go('palm-oil', { open: runId }) : ctx.go('palm-oil')));
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const err = $('[data-err]', form)!;
    err.textContent = '';
    const { run, part } = current();
    const q = num(val('quantity'));
    const price = toKobo(val('price')) || 0;
    const notPaid = pick('method') === 'Not paid yet';
    const total = Math.round(q * price);
    const sale = {
      run_id: run.id, part, sale_date: val('sale_date'), unit: pick('unit'), quantity: q, price_kobo: price,
      buyer: val('buyer').trim(), buyer_phone: val('buyer_phone').trim(), method: notPaid ? '' : pick('method'),
      paid_kobo: notPaid ? 0 : Math.min(total, toKobo(val('paid')) || 0), transport_kobo: toKobo(val('transport')) || 0, other_kobo: toKobo(val('other')) || 0,
    };
    const btn = $<HTMLButtonElement>('button.btn-primary', form)!;
    btn.disabled = true;
    try {
      const res = await call<{ receipt_id: number; receipt_no: string; run_finished: boolean }>('record_run_sale', { sale });
      showDone(ctx, run, res);
    } catch (ex) { err.textContent = errText(ex); btn.disabled = false; }
  });
}

function showDone(ctx: ViewCtx, run: Run, res: { receipt_id: number; receipt_no: string; run_finished: boolean }) {
  ctx.header('Sale saved', `Receipt ${res.receipt_no}`);
  ctx.main.innerHTML = `<section class="panel done-panel">
    <div class="done-icon">${icon('check', 30)}</div>
    <h2>Sale saved · receipt ${esc(res.receipt_no)}</h2>
    <p>${res.run_finished ? `Everything from ${esc(run.run_no)} is sold. Its result is now final.` : `It has been taken out of ${esc(run.run_no)} in your barn.`}</p>
    <div class="done-actions">
      <button class="btn btn-ghost" data-print>${icon('print', 16)} Print receipt</button>
      <button class="btn btn-ghost" data-wa>${icon('chat', 16)} WhatsApp receipt</button>
      <button class="btn btn-ghost" data-again>${icon('plus', 16)} Record another sale</button>
      <button class="btn btn-primary" data-run>${icon(res.run_finished ? 'chart' : 'box', 16)} ${res.run_finished ? 'See the result' : 'Open the run'}</button>
    </div></section>`;
  const receipt = () => call<Receipt>('get_receipt', { receiptId: res.receipt_id });
  $('[data-print]', ctx.main)!.addEventListener('click', async () => { try { printReceipt(await receipt()); } catch (ex) { toast(errText(ex), 'error'); } });
  $('[data-wa]', ctx.main)!.addEventListener('click', async () => { try { const r = await receipt(); await shareWhatsApp(r.customer_phone, receiptText(r)); } catch (ex) { toast(errText(ex), 'error'); } });
  $('[data-again]', ctx.main)!.addEventListener('click', () => ctx.go('palm-oil', { sell: '' }));
  $('[data-run]', ctx.main)!.addEventListener('click', () => ctx.go('palm-oil', { open: run.id }));
}

// ------------------------------------------------------------------ one run: the result in plain words

async function renderRun(ctx: ViewCtx, id: number) {
  const manager = auth.can('manager');
  const d = await call<{ run: Run } | Run>('get_palm_run', { id });
  const r = ('run' in d ? d.run : d) as Run;
  const showMoney = manager && !r.money_hidden;
  const harvest = r.kind === 'harvest';
  ctx.header(`${r.product_label} · ${r.run_no}`, `${r.farm_name ? `${r.farm_name} · ` : ''}${harvest ? 'harvested' : 'started'} ${dateLabel(r.start_date)}${r.status === 'closed' ? ` · sold out${r.end_date ? ` ${dateLabel(r.end_date)}` : ''}` : ''}`,
    `<button class="btn btn-ghost" data-all>${icon('back', 16)} All runs</button>${r.status === 'open' && auth.canSell() ? `<button class="btn btn-primary" data-sell>${icon('cash', 16)} Record a sale</button>` : ''}${manager ? `<button class="btn btn-ghost" data-edit>${icon('edit', 16)} Change answers</button>` : ''}${auth.can('owner') && !r.is_demo && !r.sales.length ? `<button class="btn btn-ghost" data-delete>${icon('trash', 16)} Delete</button>` : ''}`);
  const row = (a: string, b: string, cls = '') => `<div class="pl-row ${cls}"><span>${a}</span><span>${b}</span></div>`;
  const raw = qtyList(r.fruit);
  const own = qtyList(r.fruit_own);
  const final = r.status === 'closed';
  const mainSales = r.sales.filter((s) => s.part !== 'second');
  const secondSales = r.sales.filter((s) => s.part === 'second');
  ctx.main.innerHTML = `
    ${r.is_demo ? `<div class="callout callout-warn">${icon('info', 18)}<span>Demo run with illustrative figures. They are not market prices or yields.</span></div>` : ''}
    <div class="run-layout">
      ${showMoney ? `<section class="panel pl-card">
        <header class="panel-head"><h2>${final ? 'Final result' : 'Profit and loss so far'}</h2></header>
        <div class="pl-group"><p class="pl-title">You spent</p>
          ${r.costs.filter((c) => c.amount_kobo).map((c) => row(c.category === 'fruit'
            ? `Buying the ${esc(r.raw_label)} <small>${esc(raw)}${r.avg_fruit_price_kobo ? ` at ${naira(r.avg_fruit_price_kobo)} each` : ''}</small>`
            : `${esc(c.label)}${c.detail ? ` <small>${esc(c.detail)}</small>` : ''}`, naira(c.amount_kobo))).join('')}
          ${row('<b>Total spent</b>', `<b>${naira(r.total_cost_kobo)}</b>`, 'pl-total')}</div>
        <div class="pl-group"><p class="pl-title">You received</p>
          ${row(`${esc(r.main_label)} sold <small>${r.oil_sold ? esc(amount(round(r.oil_sold), r.oil_unit)) : 'none yet'}</small>`, naira(r.oil_sales_kobo))}
          ${r.second_label ? row(`${esc(r.second_label)} sold <small>${r.kernel_sold ? esc(amount(round(r.kernel_sold), r.kernel_unit)) : 'none yet'}</small>`, naira(r.kernel_sales_kobo)) : ''}
          ${row('<b>Total received</b>', `<b>${naira(r.total_sales_kobo)}</b>`, 'pl-total')}</div>
        <div class="pl-result ${r.profit_kobo < 0 ? 'is-loss' : 'is-profit'}"><span>${final ? (r.profit_kobo < 0 ? 'Loss' : 'Profit') : r.profit_kobo < 0 ? 'Not yet covered' : 'Profit so far'}</span><b>${naira(Math.abs(r.profit_kobo))}</b></div>
        ${!final && r.unsold_value_kobo ? `<div class="pl-unsold">${row('Still in the barn, at today\'s price', naira(r.unsold_value_kobo))}${row('<b>If you sell it all at today\'s price</b>', `<b class="${r.projected_profit_kobo < 0 ? 'bad-text' : 'ok-text'}">${r.projected_profit_kobo < 0 ? 'Loss ' : 'Profit '}${naira(Math.abs(r.projected_profit_kobo))}</b>`, 'pl-total')}</div>` : ''}
      </section>` : ''}
      <section class="panel">
        <header class="panel-head"><h2>From start to finish</h2></header>
        <ol class="timeline">
          ${raw ? `<li><b>Bought ${esc(raw)} of ${esc(r.raw_label)}</b></li>` : ''}
          ${own ? `<li><b>${esc(own)} of ${esc(r.raw_label)} from the farm</b></li>` : ''}
          ${harvest ? `<li><b>Harvested ${r.oil_made ? esc(amount(round(r.oil_made), r.oil_unit)) : ''}${r.labour_workers ? ` with ${r.labour_workers} worker${r.labour_workers === 1 ? '' : 's'}` : ''}</b></li>`
            : `<li><b>Processed${r.labour_workers ? ` with ${r.labour_workers} worker${r.labour_workers === 1 ? '' : 's'}` : ''}</b></li>
          <li><b>Got ${r.oil_made ? esc(amount(round(r.oil_made), r.oil_unit)) : 'no'} of ${esc(r.main_label.toLowerCase())}${r.second_label && r.kernel_made ? ` and ${esc(amount(round(r.kernel_made), r.kernel_unit))} of ${esc(r.second_label.toLowerCase())}` : ''}</b></li>`}
          ${showMoney && r.cost_per_oil_unit_kobo ? `<li><b>Into the barn</b><small>Each ${esc(r.oil_unit)} cost you ${naira(r.cost_per_oil_unit_kobo)}</small></li>` : '<li><b>Into the barn</b></li>'}
          <li><b>${r.sales.length} sale${r.sales.length === 1 ? '' : 's'}: ${r.oil_sold ? esc(amount(round(r.oil_sold), r.oil_unit)) : 'nothing yet'}${r.second_label && r.kernel_sold ? ` and ${esc(amount(round(r.kernel_sold), r.kernel_unit))} of ${esc(r.second_label.toLowerCase())}` : ''}</b></li>
          <li class="${final ? '' : 'is-open'}"><b>${final ? 'Everything sold. The result is final.' : `Left in the barn: ${[r.oil_left > 1e-9 ? `${amount(round(r.oil_left), r.oil_unit)} of ${r.main_label.toLowerCase()}` : '', r.kernel_left > 1e-9 ? `${amount(round(r.kernel_left), r.kernel_unit)} of ${r.second_label.toLowerCase()}` : ''].filter(Boolean).map(esc).join(', ')}`}</b></li>
        </ol>
        ${r.notes ? `<p class="muted">${esc(r.notes)}</p>` : ''}
      </section>
    </div>
    <section class="panel">
      <header class="panel-head"><h2>Sales from this run</h2></header>
      ${r.sales.length ? `<div class="table-wrap"><table class="table">
        <thead><tr><th>Date</th><th>What</th><th>Sold</th>${showMoney ? '<th class="num">Price of one</th><th class="num">Amount</th><th class="num">Selling costs</th>' : ''}<th>Buyer</th><th>Receipt</th></tr></thead>
        <tbody>${[...mainSales, ...secondSales].map((s) => `<tr><td>${esc(dateLabel(s.sale_date))}</td><td>${esc(s.part === 'second' ? r.second_label : r.main_label)}</td>
          <td>${esc(amount(s.quantity, s.unit))}${s.unit !== (s.part === 'second' ? r.kernel_unit : r.oil_unit) ? ` <small class="muted">(${esc(amount(round(s.stock_qty), s.part === 'second' ? r.kernel_unit : r.oil_unit))})</small>` : ''}</td>
          ${showMoney ? `<td class="num">${naira(s.price_kobo)}</td><td class="num">${naira(s.amount_kobo)}</td><td class="num">${s.transport_kobo + s.other_kobo ? naira(s.transport_kobo + s.other_kobo) : '—'}</td>` : ''}
          <td>${esc(s.buyer || '—')}</td><td>${esc(s.receipt_no || '—')}</td></tr>`).join('')}</tbody></table></div>`
        : `<p class="muted">No sales yet.${r.status === 'open' && auth.canSell() ? ' Use “Record a sale” when you sell.' : ''}</p>`}
      ${r.sales.length ? '<p class="muted small">To undo a sale, void its receipt in Receipts. The quantity goes back to this run.</p>' : ''}
    </section>`;
  document.querySelector('[data-all]')?.addEventListener('click', () => ctx.go('palm-oil'));
  document.querySelector('[data-sell]')?.addEventListener('click', () => ctx.go('palm-oil', { sell: String(id) }));
  document.querySelector('[data-edit]')?.addEventListener('click', () => ctx.go('palm-oil', { edit: String(id) }));
  document.querySelector('[data-delete]')?.addEventListener('click', async () => {
    if (!(await confirmDialog({ title: `Delete ${r.run_no}?`, message: 'What this run put into your barn is taken back out.', confirm: 'Delete', danger: true }))) return;
    try { await call('delete_palm_run', { id }); toast('Run deleted'); ctx.go('palm-oil'); } catch (ex) { toast(errText(ex), 'error'); }
  });
}
