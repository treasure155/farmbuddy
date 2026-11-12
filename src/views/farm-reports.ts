import { auth, call } from '../api';
import { exportCsv } from '../lib/csv';
import { $, confirmDialog, errText, esc, toast } from '../lib/dom';
import { dateLabel, isoDate, money, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { t, tl } from '../lib/i18n';
import { savePdfReport, type ReportSection } from '../lib/report-pdf';
import { fmtNum, formModal, monthRange, pill, tableHtml, tabsHtml } from '../lib/ui';
import { store } from '../store';
import type { ViewCtx } from './shell';

interface Line { label: string; amount_kobo: number }
interface PL { from: string; to: string; include_demo: boolean; revenue: Line[]; revenue_kobo: number; cost_of_sales_kobo: number; production: Line[]; production_kobo: number; capitalised_to_stock_kobo: number; gross_profit_kobo: number;
  expenses: Line[]; expenses_kobo: number; net_profit_kobo: number; cash_in: Line[]; cash_in_kobo: number; cash_out: Line[]; cash_out_kobo: number; net_cash_kobo: number;
  owed_by_customers_kobo: number; owed_to_suppliers_kobo: number; inventory_value_kobo: number; rules: string[] }
interface Activity { kind: string; id: number; name: string; detail: string; status: string; started: string; revenue_kobo: number; cost_kobo: number; profit_kobo: number; unit_cost: string; is_demo: boolean }
type J = Record<string, unknown> & { is_demo?: boolean };
interface Production { from: string; to: string; livestock: J[]; crops: J[]; processing: J[]; vaccinations: J[]; health: J[]; weights: J[]; feed: J[] }
interface FinanceItem { id: number; kind: 'asset' | 'liability'; name: string; value_kobo: number; as_of: string; notes: string }
interface Summary { generated_at: string; business: Record<string, string | null>; farms: Array<{ name: string; state: string; lga: string; activities: string }>; profit_loss: PL; activities: Activity[]; assets: FinanceItem[]; liabilities: FinanceItem[]; assets_kobo: number; liabilities_kobo: number; production: Record<string, unknown>; disclaimer: string }

let tab = '';
let range = monthRange();
let includeDemo = false;
const s = (v: unknown) => (v === null || v === undefined ? '' : String(v));
const n = (v: unknown) => (typeof v === 'number' ? v : 0);
const lines = (ls: Line[]) => ls.map((l) => [esc(l.label), money(l.amount_kobo)]);

/** Farm reports: profit and loss, cash flow, profit by activity, production registers and a finance summary for loans and grants. */
export async function render(ctx: ViewCtx) {
  const manager = auth.can('manager');
  const owner = auth.can('owner');
  const tabs: Array<[string, string]> = [...(manager ? [['pl', 'Profit and loss'], ['cash', 'Cash flow'], ['activity', 'Profit by activity']] as Array<[string, string]> : []), ['production', 'Production records'], ...(owner ? [['finance', 'Finance summary']] as Array<[string, string]> : [])];
  if (typeof ctx.params.tab === 'string') tab = ctx.params.tab;
  if (!tabs.some((x) => x[0] === tab)) tab = tabs[0][0];
  ctx.header(t('nav.farm_reports', {}, 'Farm reports'), 'Whole-farm results from your records, ready to print, save as PDF or export.');
  ctx.main.innerHTML = `${tabsHtml(tabs, tab)}
    <form class="toolbar" data-range><label class="field"><span>From</span><input type="date" name="from" value="${range.from}"></label><label class="field"><span>To</span><input type="date" name="to" value="${range.to}"></label>
      ${manager ? `<label class="check inline"><input type="checkbox" name="demo" ${includeDemo ? 'checked' : ''}><span>Include demo records</span></label>` : ''}<button class="btn btn-ghost">Show</button>
      <span class="grow"></span><button type="button" class="btn btn-ghost" data-pdf>${icon('download', 16)} Save PDF</button>${manager ? `<button type="button" class="btn btn-ghost" data-csv>${icon('download', 16)} CSV</button>` : ''}<button type="button" class="btn btn-ghost" data-print>${icon('print', 16)} Print</button></form>
    <div data-body class="stack report-body"></div>`;
  const body = $('[data-body]', ctx.main)!;
  let exportFn: { pdf: () => void; csv: () => void } = { pdf: () => undefined, csv: () => undefined };
  const show = async () => {
    body.innerHTML = '<div class="loading"><span></span><span></span><span></span></div>';
    try {
      if (tab === 'pl' || tab === 'cash') exportFn = await profitLoss(body, tab);
      else if (tab === 'activity') exportFn = await activities(body);
      else if (tab === 'production') exportFn = await production(body);
      else exportFn = await finance(body);
    } catch (x) { body.innerHTML = `<p class="form-error">${esc(errText(x))}</p>`; }
  };
  const form = $<HTMLFormElement>('[data-range]', ctx.main)!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    range = { from: (form.elements.namedItem('from') as HTMLInputElement).value, to: (form.elements.namedItem('to') as HTMLInputElement).value };
    includeDemo = !!(form.elements.namedItem('demo') as HTMLInputElement | null)?.checked;
    void show();
  });
  ctx.main.addEventListener('click', (e) => {
    const el = e.target as HTMLElement;
    const b = el.closest<HTMLElement>('[data-tab]');
    if (b) { tab = b.dataset.tab!; ctx.main.querySelectorAll('[data-tab]').forEach((c) => c.classList.toggle('is-on', c === b)); void show(); }
    if (el.closest('[data-pdf]')) exportFn.pdf();
    if (el.closest('[data-csv]')) exportFn.csv();
    if (el.closest('[data-print]')) call('print_window').catch(() => window.print());
  });
  await show();
}

const period = () => `${dateLabel(range.from)} to ${dateLabel(range.to)}`;
const biz = () => store.settings.business_name || 'My Farm';

async function profitLoss(body: HTMLElement, which: string) {
  const p = await call<PL>('farm_profit_loss', { from: range.from, to: range.to, includeDemo });
  const demoNote = p.include_demo ? `<div class="callout callout-warn"><div><b>Includes demo records</b><span>Untick "Include demo records" for your real figures.</span></div></div>` : '';
  if (which === 'pl') {
    body.innerHTML = `${demoNote}<div class="figures">
        <div><span>Revenue</span><b>${money(p.revenue_kobo, { whole: true })}</b></div>
        <div><span>Gross profit</span><b class="${p.gross_profit_kobo < 0 ? 'bad-text' : ''}">${money(p.gross_profit_kobo, { whole: true })}</b></div>
        <div><span>Expenses</span><b>${money(p.expenses_kobo, { whole: true })}</b></div>
        <div><span>Net profit</span><b class="${p.net_profit_kobo < 0 ? 'bad-text' : 'ok-text'}">${money(p.net_profit_kobo, { whole: true })}</b></div></div>
      <section class="panel"><header class="panel-head"><h2>Profit and loss</h2><p class="muted small">${esc(period())}</p></header>
        ${tableHtml(['Revenue', 'Amount#'], [...lines(p.revenue), ['<b>Total revenue</b>', `<b>${money(p.revenue_kobo)}</b>`]])}
        ${tableHtml(['Cost of sales and production', 'Amount#'], [['Cost of goods sold (stock items at cost)', money(p.cost_of_sales_kobo)], ...lines(p.production),
          ...(p.capitalised_to_stock_kobo ? [['Less: processing cost carried into stock (counted when sold)', `−${money(p.capitalised_to_stock_kobo)}`]] : []), ['<b>Gross profit</b>', `<b>${money(p.gross_profit_kobo)}</b>`]])}
        ${tableHtml(['Operating expenses', 'Amount#'], [...lines(p.expenses), ['<b>Total expenses</b>', `<b>${money(p.expenses_kobo)}</b>`], ['<b>Net profit</b>', `<b>${money(p.net_profit_kobo)}</b>`]])}
      </section>
      <section class="panel"><header class="panel-head"><h2>Position today</h2></header>${tableHtml(['', 'Amount#'], [['Inventory value at cost', money(p.inventory_value_kobo)], ['Owed to you by customers', money(p.owed_by_customers_kobo)], ['You owe suppliers', money(p.owed_to_suppliers_kobo)]])}</section>
      <details class="panel"><summary><b>How each naira is counted once</b></summary><ul class="small">${p.rules.map((r) => `<li>${esc(r)}</li>`).join('')}</ul></details>`;
  } else {
    body.innerHTML = `${demoNote}<div class="figures"><div><span>Cash in</span><b>${money(p.cash_in_kobo, { whole: true })}</b></div><div><span>Cash out</span><b>${money(p.cash_out_kobo, { whole: true })}</b></div>
        <div><span>Net cash</span><b class="${p.net_cash_kobo < 0 ? 'bad-text' : 'ok-text'}">${money(p.net_cash_kobo, { whole: true })}</b></div></div>
      <p class="muted small">Cash flow counts money actually received and paid in the period. Credit sales count when the customer pays; purchases count when you pay the supplier.</p>
      ${tableHtml(['Cash in', 'Amount#'], [...lines(p.cash_in), ['<b>Total</b>', `<b>${money(p.cash_in_kobo)}</b>`]])}
      ${tableHtml(['Cash out', 'Amount#'], [...lines(p.cash_out), ['<b>Total</b>', `<b>${money(p.cash_out_kobo)}</b>`]])}`;
  }
  const n2 = (k: number) => money(k);
  const sections: ReportSection[] = which === 'pl' ? [
    { title: 'Revenue', headers: ['Line', 'Amount'], numeric: [false, true], rows: [...p.revenue.map((l) => [l.label, n2(l.amount_kobo)]), ['Total revenue', n2(p.revenue_kobo)]] },
    { title: 'Cost of sales and production', headers: ['Line', 'Amount'], numeric: [false, true], rows: [['Cost of goods sold', n2(p.cost_of_sales_kobo)], ...p.production.map((l) => [l.label, n2(l.amount_kobo)]), ['Less: processing cost carried into stock', `-${n2(p.capitalised_to_stock_kobo)}`], ['Gross profit', n2(p.gross_profit_kobo)]] },
    { title: 'Operating expenses', headers: ['Line', 'Amount'], numeric: [false, true], rows: [...p.expenses.map((l) => [l.label, n2(l.amount_kobo)]), ['Total expenses', n2(p.expenses_kobo)], ['NET PROFIT', n2(p.net_profit_kobo)]] },
    { title: 'Position', headers: ['', 'Amount'], numeric: [false, true], rows: [['Inventory at cost', n2(p.inventory_value_kobo)], ['Owed by customers', n2(p.owed_by_customers_kobo)], ['Owed to suppliers', n2(p.owed_to_suppliers_kobo)]] },
    { title: 'Counting rules', notes: p.rules },
  ] : [
    { title: 'Cash in', headers: ['Line', 'Amount'], numeric: [false, true], rows: [...p.cash_in.map((l) => [l.label, n2(l.amount_kobo)]), ['Total', n2(p.cash_in_kobo)]] },
    { title: 'Cash out', headers: ['Line', 'Amount'], numeric: [false, true], rows: [...p.cash_out.map((l) => [l.label, n2(l.amount_kobo)]), ['Total', n2(p.cash_out_kobo)], ['NET CASH', n2(p.net_cash_kobo)]] },
  ];
  const title = which === 'pl' ? 'Profit and loss' : 'Cash flow';
  return {
    pdf: () => void savePdfReport(`${which === 'pl' ? 'profit-and-loss' : 'cash-flow'}-${range.from}-${range.to}.pdf`, `${biz()}: ${title}`, [period(), p.include_demo ? 'Includes demo records' : 'Demo records excluded'], sections),
    csv: () => void exportCsv(`${which === 'pl' ? 'profit-and-loss' : 'cash-flow'}-${range.from}-${range.to}.csv`, ['Section', 'Line', 'Amount (NGN)'], sections.flatMap((x) => (x.rows ?? []).map((r) => [x.title, r[0], r[1].replace(/[^\d.-]/g, '')]))),
  };
}

async function activities(body: HTMLElement) {
  const list = await call<Activity[]>('profit_by_activity', { from: range.from, to: range.to, includeDemo });
  const kindName: Record<string, string> = { livestock: 'Livestock batch', crop: 'Crop', processing: 'Processing batch' };
  const total = (k: keyof Activity) => list.reduce((a, x) => a + n(x[k]), 0);
  body.innerHTML = `<p class="muted small">Each batch, crop and processing run started in the period, with its own revenue and costs. Costs are only those recorded against that activity.</p>
    ${tableHtml(['Activity', 'Name', 'Started', 'Status', 'Revenue#', 'Costs#', 'Profit#', 'Unit cost'], [...list.map((a) => [esc(kindName[a.kind] ?? a.kind), `${esc(a.name)} <small class="muted">${esc(a.detail)}</small>${a.is_demo ? ` ${pill('Demo', 'warn')}` : ''}`, esc(dateLabel(a.started)), esc(a.status), money(a.revenue_kobo), money(a.cost_kobo), `<b class="${a.profit_kobo < 0 ? 'bad-text' : 'ok-text'}">${money(a.profit_kobo)}</b>`, esc(a.unit_cost)]),
      ...(list.length ? [['<b>Total</b>', '', '', '', `<b>${money(total('revenue_kobo'))}</b>`, `<b>${money(total('cost_kobo'))}</b>`, `<b>${money(total('profit_kobo'))}</b>`, '']] : [])], 'No activities started in this period.')}`;
  const rows = list.map((a) => [kindName[a.kind] ?? a.kind, a.name, a.started, a.status, money(a.revenue_kobo), money(a.cost_kobo), money(a.profit_kobo), a.unit_cost]);
  return {
    pdf: () => void savePdfReport(`profit-by-activity-${range.from}-${range.to}.pdf`, `${biz()}: Profit by activity`, [period()], [{ title: 'Activities', headers: ['Activity', 'Name', 'Started', 'Status', 'Revenue', 'Costs', 'Profit', 'Unit cost'], numeric: [false, false, false, false, true, true, true, false], rows }]),
    csv: () => void exportCsv(`profit-by-activity-${range.from}-${range.to}.csv`, ['Activity', 'Name', 'Detail', 'Started', 'Status', 'Revenue NGN', 'Costs NGN', 'Profit NGN', 'Unit cost', 'Demo'], list.map((a) => [kindName[a.kind] ?? a.kind, a.name, a.detail, a.started, a.status, a.revenue_kobo / 100, a.cost_kobo / 100, a.profit_kobo / 100, a.unit_cost, a.is_demo ? 'yes' : ''])),
  };
}

async function production(body: HTMLElement) {
  const r = await call<Production>('get_production_report', { from: range.from, to: range.to });
  const withMoney = auth.can('manager');
  const demo = (x: J) => (x.is_demo ? ` ${pill('Demo', 'warn')}` : '');
  const L = r.livestock.map((x) => [esc(s(x.code)) + demo(x), esc(tl('species', s(x.species))), esc(s(x.placed_date)), qty(n(x.total_in)), qty(n(x.alive)), `${n(x.dead)} (${fmtNum(n(x.mortality_pct))}%)`, String(n(x.deaths_in_period)), fmtNum(n(x.feed_in_period_kg)), n(x.fcr) ? n(x.fcr).toFixed(2) : '—', n(x.eggs) ? qty(n(x.eggs)) : '', ...(withMoney ? [money(n(x.profit_kobo))] : [])]);
  const C = r.crops.map((x) => [esc(s(x.crop)) + demo(x), esc(s(x.variety)), esc(s(x.plot)), `${fmtNum(n(x.area))} ${esc(s(x.area_unit))}`, esc(s(x.planted)), esc(s(x.status)), String(n(x.activities)), String(n(x.harvests)), fmtNum(n(x.yield_per_area)), `${fmtNum(n(x.loss_pct))}%`, ...(withMoney ? [money(n(x.margin_kobo))] : [])]);
  const P = r.processing.map((x) => [esc(s(x.batch_no)) + demo(x), esc(s(x.type).replace(/_/g, ' ')), esc(s(x.started)), esc(s(x.status)), `${fmtNum(n(x.raw_qty))} ${esc(s(x.raw_unit))}`, esc((x.outputs as J[]).map((o) => `${fmtNum(n(o.quantity))} ${s(o.unit)} ${s(o.name)}`).join(', ')), `${fmtNum(n(x.yield_pct))}%`, `${fmtNum(n(x.waste_pct))}%`, ...(withMoney ? [money(n(x.cost_kobo))] : [])]);
  const V = r.vaccinations.map((x) => [esc(s(x.code)), esc(s(x.vaccine)), esc(s(x.disease)), esc(s(x.due)), esc(s(x.status)), esc(s(x.given)), esc(s(x.by)), s(x.confirmed_by) ? pill('Vet confirmed', 'ok') : pill('Not confirmed', 'warn')]);
  const H = r.health.map((x) => [esc(s(x.code)), esc(s(x.date)), esc(s(x.issue)), String(n(x.affected)), esc(s(x.action)), esc(s(x.vet))]);
  const F = r.feed.map((x) => [esc(s(x.code)), fmtNum(n(x.feed_kg)), fmtNum(n(x.wasted_kg)), `${fmtNum(n(x.wastage_pct))}%`, fmtNum(n(x.per_animal_kg), 2), n(x.fcr) ? n(x.fcr).toFixed(2) : '—']);
  const W = r.weights.map((x) => [esc(s(x.code)), esc(s(x.date)), fmtNum(n(x.avg_weight_kg), 2)]);
  const LH = ['Batch', 'Type', 'Placed', 'In#', 'Alive#', 'Dead#', 'Deaths in period#', 'Feed in period (kg)#', 'FCR#', 'Eggs#', ...(withMoney ? ['Profit#'] : [])];
  const CH = ['Crop', 'Variety', 'Plot', 'Area', 'Planted', 'Status', 'Activities#', 'Harvests#', 'Yield per area#', 'Loss#', ...(withMoney ? ['Margin#'] : [])];
  const PH = ['Batch', 'Process', 'Started', 'Status', 'Raw material', 'Outputs', 'Yield#', 'Waste#', ...(withMoney ? ['Cost#'] : [])];
  body.innerHTML = `<section class="panel"><header class="panel-head"><h2>Livestock</h2></header>${tableHtml(LH, L, 'No batches in this period.')}</section>
    <section class="panel"><header class="panel-head"><h2>Feed</h2></header>${tableHtml(['Batch', 'Feed (kg)#', 'Wasted (kg)#', 'Wastage#', 'Per animal (kg)#', 'FCR#'], F)}</section>
    <section class="panel"><header class="panel-head"><h2>Weights</h2></header>${tableHtml(['Batch', 'Date', 'Average (kg)#'], W, 'No weighings in this period.')}</section>
    <section class="panel"><header class="panel-head"><h2>Vaccinations</h2></header>${tableHtml(['Batch', 'Vaccine', 'Disease', 'Due', 'Status', 'Given', 'By', 'Review'], V, 'No vaccinations in this period.')}</section>
    <section class="panel"><header class="panel-head"><h2>Health</h2></header>${tableHtml(['Batch', 'Date', 'Issue', 'Affected#', 'Action', 'Vet'], H, 'No health records in this period.')}</section>
    <section class="panel"><header class="panel-head"><h2>Crops</h2></header>${tableHtml(CH, C, 'No crops in this period.')}</section>
    <section class="panel"><header class="panel-head"><h2>Processing</h2></header>${tableHtml(PH, P, 'No processing batches in this period.')}</section>`;
  const plain = (rows: string[][]) => rows.map((row) => row.map((c) => c.replace(/<[^>]+>/g, '').replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'")));
  const hdr = (h: string[]) => h.map((x) => x.replace(/#$/, ''));
  const numeric = (h: string[]) => h.map((x) => x.endsWith('#'));
  const sections: ReportSection[] = [
    { title: 'Livestock', headers: hdr(LH), numeric: numeric(LH), rows: plain(L) },
    { title: 'Feed', headers: ['Batch', 'Feed kg', 'Wasted kg', 'Wastage', 'Per animal kg', 'FCR'], numeric: [false, true, true, true, true, true], rows: plain(F) },
    { title: 'Vaccinations', headers: ['Batch', 'Vaccine', 'Disease', 'Due', 'Status', 'Given', 'By', 'Review'], rows: plain(V) },
    { title: 'Health', headers: ['Batch', 'Date', 'Issue', 'Affected', 'Action', 'Vet'], rows: plain(H) },
    { title: 'Crops', headers: hdr(CH), numeric: numeric(CH), rows: plain(C) },
    { title: 'Processing', headers: hdr(PH), numeric: numeric(PH), rows: plain(P) },
  ];
  return {
    pdf: () => void savePdfReport(`production-${range.from}-${range.to}.pdf`, `${biz()}: Production records`, [period()], sections),
    csv: () => void exportCsv(`production-${range.from}-${range.to}.csv`, ['Section', 'Columns', 'Values'], sections.flatMap((x) => (x.rows ?? []).map((row) => [x.title, (x.headers ?? []).join(' | '), row.join(' | ')]))),
  };
}

async function finance(body: HTMLElement) {
  const [sum, items] = await Promise.all([call<Summary>('farm_finance_summary', { from: range.from, to: range.to }), call<FinanceItem[]>('list_finance_items')]);
  const p = sum.profit_loss;
  body.innerHTML = `<div class="callout callout-info"><div><b>For loan, grant and cooperative applications</b><span>${esc(sum.disclaimer)}</span></div></div>
    <section class="panel"><header class="panel-head"><h2>${esc(s(sum.business.name) || biz())}</h2><p class="muted small">${esc([sum.business.address, sum.business.phone, sum.business.email].filter(Boolean).join(' · '))}</p></header>
      ${tableHtml(['Farm', 'Location', 'Activities'], sum.farms.map((f) => [esc(f.name), esc([f.lga, f.state].filter(Boolean).join(', ')), esc(f.activities.replace(/,/g, ', ').replace(/_/g, ' '))]), 'No farm profile yet.')}</section>
    <div class="figures"><div><span>Revenue (${esc(period())})</span><b>${money(p.revenue_kobo, { whole: true })}</b></div><div><span>Net profit</span><b class="${p.net_profit_kobo < 0 ? 'bad-text' : 'ok-text'}">${money(p.net_profit_kobo, { whole: true })}</b></div>
      <div><span>Assets</span><b>${money(sum.assets_kobo + p.inventory_value_kobo + p.owed_by_customers_kobo, { whole: true })}</b><small class="muted">incl. stock and money owed to you</small></div><div><span>Liabilities</span><b>${money(sum.liabilities_kobo + p.owed_to_suppliers_kobo, { whole: true })}</b><small class="muted">incl. owed to suppliers</small></div></div>
    <section class="panel"><header class="panel-head"><h2>Assets and liabilities</h2><button class="btn btn-ghost btn-sm" data-add-item>${icon('plus', 14)} Add</button></header>
      ${tableHtml(['Type', 'Name', 'Value#', 'As of', 'Notes', ''], items.map((i) => [i.kind === 'asset' ? 'Asset' : 'Liability', esc(i.name), money(i.value_kobo), esc(dateLabel(i.as_of)), esc(i.notes), `<button class="icon-btn" data-edit-item="${i.id}" aria-label="Edit">${icon('edit', 15)}</button><button class="icon-btn" data-del-item="${i.id}" aria-label="Delete">${icon('trash', 15)}</button>`]), 'Add equipment, land, buildings and vehicles you own, and loans or debts you owe.')}</section>
    <section class="panel"><header class="panel-head"><h2>Results by activity</h2></header>${tableHtml(['Activity', 'Revenue#', 'Costs#', 'Profit#'], sum.activities.map((a) => [esc(a.name), money(a.revenue_kobo), money(a.cost_kobo), money(a.profit_kobo)]), 'No activities in this period.')}</section>`;
  const itemForm = (i: FinanceItem | null) => formModal({ title: i ? 'Edit' : 'Add asset or liability', fields: [
    { name: 'kind', label: 'Type', type: 'select', options: [['asset', 'Asset (what the farm owns)'], ['liability', 'Liability (what the farm owes)']], value: i?.kind ?? 'asset', pair: true },
    { name: 'value', label: 'Value', type: 'money', value: i?.value_kobo ?? '', required: true },
    { name: 'name', label: 'Name', value: i?.name ?? '', required: true, placeholder: 'e.g. Poultry house, Motorcycle, Bank of Agriculture loan', pair: true },
    { name: 'as_of', label: 'Value as of', type: 'date', value: i?.as_of ?? isoDate(new Date()), required: true },
    { name: 'notes', label: 'Notes', type: 'textarea', value: i?.notes ?? '' },
  ], done: 'Saved', submit: async (v) => {
    await call('save_finance_item', { input: { id: i?.id ?? 0, kind: v.kind, name: v.name, value_kobo: toKobo(v.value), as_of: v.as_of, notes: v.notes } });
    void finance(body);
  } });
  body.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-add-item]')) itemForm(null);
    const ed = el.closest<HTMLElement>('[data-edit-item]');
    if (ed) itemForm(items.find((x) => x.id === Number(ed.dataset.editItem)) ?? null);
    const d = el.closest<HTMLElement>('[data-del-item]');
    if (d && await confirmDialog({ title: 'Delete this entry?', message: 'It is removed from the finance summary.', confirm: 'Delete', danger: true }) !== null) { try { await call('delete_finance_item', { id: Number(d.dataset.delItem) }); void finance(body); } catch (x) { toast(errText(x), 'error'); } }
  };
  const sections: ReportSection[] = [
    { title: 'Business', notes: [s(sum.business.name), s(sum.business.address), s(sum.business.phone), s(sum.business.email)].filter(Boolean) },
    { title: 'Farms', headers: ['Farm', 'Location', 'Activities'], rows: sum.farms.map((f) => [f.name, [f.lga, f.state].filter(Boolean).join(', '), f.activities.replace(/,/g, ', ')]) },
    { title: `Results ${period()}`, headers: ['', 'Amount'], numeric: [false, true], rows: [['Revenue', money(p.revenue_kobo)], ['Gross profit', money(p.gross_profit_kobo)], ['Expenses', money(p.expenses_kobo)], ['Net profit', money(p.net_profit_kobo)], ['Cash in', money(p.cash_in_kobo)], ['Cash out', money(p.cash_out_kobo)]] },
    { title: 'Assets', headers: ['Name', 'As of', 'Value'], numeric: [false, false, true], rows: [...sum.assets.map((a) => [a.name, a.as_of, money(a.value_kobo)]), ['Stock at cost', '', money(p.inventory_value_kobo)], ['Owed by customers', '', money(p.owed_by_customers_kobo)]] },
    { title: 'Liabilities', headers: ['Name', 'As of', 'Value'], numeric: [false, false, true], rows: [...sum.liabilities.map((a) => [a.name, a.as_of, money(a.value_kobo)]), ['Owed to suppliers', '', money(p.owed_to_suppliers_kobo)]] },
    { title: 'Results by activity', headers: ['Activity', 'Revenue', 'Costs', 'Profit'], numeric: [false, true, true, true], rows: sum.activities.map((a) => [a.name, money(a.revenue_kobo), money(a.cost_kobo), money(a.profit_kobo)]) },
    { title: 'Production now', notes: [`Live animals: ${s(sum.production.live_animals)} in ${s(sum.production.active_batches)} batches`, `Active plots: ${s(sum.production.active_plots)}; growing crops: ${s(sum.production.growing_crops)}`, `Open processing batches: ${s(sum.production.open_processing)}`] },
    { title: 'Note', notes: [sum.disclaimer, `Prepared ${sum.generated_at}`] },
  ];
  return {
    pdf: () => void savePdfReport(`finance-summary-${range.to}.pdf`, `${biz()}: Farm finance summary`, [period()], sections),
    csv: () => void exportCsv(`finance-summary-${range.to}.csv`, ['Section', 'Line', 'Detail', 'Value'], sections.flatMap((x) => (x.rows ?? []).map((r) => [x.title, ...r]))),
  };
}
