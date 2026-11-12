import { auth, call } from '../api';
import { exportCsv } from '../lib/csv';
import { $, confirmDialog, errText, esc, toast } from '../lib/dom';
import { dateLabel, isoDate, money, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { t } from '../lib/i18n';
import { LGAS, STATES } from '../lib/content/nigeria';
import { fmtNum, formModal, num, pill, tableHtml, tabsHtml } from '../lib/ui';
import { store } from '../store';
import type { ViewCtx } from './shell';

interface Weather { id: number; farm_id: number | null; record_date: string; rainfall_mm: number | null; temp_min_c: number | null; temp_max_c: number | null; conditions: string; notes: string; user_name: string }
interface MonthRain { month: string; rainfall_mm: number; rain_days: number; records: number }
interface Outlook { id: number; year: number; state: string; lga: string; onset_date: string; cessation_date: string; season_days: number | null; dry_spell_notes: string; notes: string; source: string; published_date: string; imported_at: string }
interface Price { id: number; product: string; market: string; state: string; lga: string; price_date: string; unit: string; wholesale_kobo: number | null; retail_kobo: number | null; buyer_offer_kobo: number | null; quantity_available: string; source: string; notes: string; is_demo: boolean; user_name: string }
interface PriceSummary { product: string; unit: string; market: string; observations: number; latest_date: string; latest_wholesale_kobo: number | null; latest_retail_kobo: number | null; min_kobo: number | null; max_kobo: number | null; demo_only: boolean }

let tab = 'weather';
const m = (k: number | null) => (k === null || k === undefined ? '—' : money(k, { whole: true }));
const stateOpts = (): Array<[string, string]> => [['', '—'], ...STATES.map((s): [string, string] => [s, s])];

/** Weather and rainfall you record, NiMet seasonal outlooks you enter, and market prices you observe. */
export async function render(ctx: ViewCtx) {
  if (typeof ctx.params.tab === 'string') tab = ctx.params.tab;
  ctx.header(t('nav.weather', {}, 'Weather and markets'), 'Rainfall on your farm, the season outlook, and market prices with their history.');
  ctx.main.innerHTML = `${tabsHtml([['weather', 'Weather and rainfall'], ['outlook', 'Season outlook (NiMet)'], ['market', 'Market prices']], tab)}<div data-body class="stack"></div>`;
  const body = $('[data-body]', ctx.main)!;
  const show = async () => {
    try { if (tab === 'weather') await weather(body); else if (tab === 'outlook') await outlooks(body); else await market(body); }
    catch (x) { body.innerHTML = `<p class="form-error">${esc(errText(x))}</p>`; }
  };
  ctx.main.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-tab]');
    if (!b) return;
    tab = b.dataset.tab!;
    ctx.main.querySelectorAll('[data-tab]').forEach((c) => c.classList.toggle('is-on', c === b));
    void show();
  });
  await show();
}

async function weather(body: HTMLElement) {
  const year = new Date().getFullYear();
  const [list, recorded] = await Promise.all([call<Weather[]>('list_weather', { from: `${year - 1}-01-01`, to: isoDate(new Date()) }), call<MonthRain[]>('rainfall_by_month', { year })]);
  const months: MonthRain[] = Array.from({ length: 12 }, (_, i) => {
    const key = `${year}-${String(i + 1).padStart(2, '0')}`;
    return recorded.find((m) => m.month === key) ?? { month: key, rainfall_mm: 0, rain_days: 0, records: 0 };
  });
  const total = months.reduce((s, x) => s + x.rainfall_mm, 0);
  const max = Math.max(1, ...months.map((x) => x.rainfall_mm));
  body.innerHTML = `<div class="row-actions"><button class="btn btn-primary" data-add>${icon('plus', 16)} Record today's weather</button><button class="btn btn-ghost" data-csv>${icon('download', 16)} Export CSV</button></div>
    <section class="panel"><header class="panel-head"><h2>Rainfall in ${year}</h2><p class="muted small">${fmtNum(total)} mm recorded. Days without a reading are not counted as dry.</p></header>
      <div class="bar-chart" role="img" aria-label="Rainfall by month">${months.map((x) => `<div class="bar-col" title="${esc(x.month)}: ${fmtNum(x.rainfall_mm)} mm, ${x.rain_days} rain days"><span class="bar" style="height:${Math.round((x.rainfall_mm / max) * 100)}%"></span><small>${esc(x.month.slice(5))}</small></div>`).join('')}</div>
      ${tableHtml(['Month', 'Rainfall (mm)#', 'Rain days#', 'Readings#'], months.filter((x) => x.records).map((x) => [esc(x.month), fmtNum(x.rainfall_mm), String(x.rain_days), String(x.records)]), 'No readings this year yet.')}</section>
    <section class="panel"><header class="panel-head"><h2>Readings</h2></header>
      ${tableHtml(['Date', 'Rain (mm)#', 'Min °C#', 'Max °C#', 'Conditions', 'By', ''], list.map((w) => [esc(dateLabel(w.record_date)), w.rainfall_mm === null ? '—' : fmtNum(w.rainfall_mm), fmtNum(w.temp_min_c), fmtNum(w.temp_max_c), esc([w.conditions, w.notes].filter(Boolean).join(' · ')), esc(w.user_name),
        auth.can('manager') ? `<button class="icon-btn" data-del="${w.id}" aria-label="Delete">${icon('trash', 15)}</button>` : '']))}</section>`;
  body.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-add]')) formModal({ title: 'Record weather', intro: 'Leave a reading empty if you did not measure it.', fields: [
      { name: 'date', label: 'Date', type: 'date', value: isoDate(new Date()), required: true, pair: true },
      { name: 'rain', label: 'Rainfall (mm)', type: 'number', help: 'From a rain gauge' },
      { name: 'min', label: 'Lowest temperature (°C)', type: 'number', pair: true },
      { name: 'max', label: 'Highest temperature (°C)', type: 'number' },
      { name: 'conditions', label: 'Conditions', list: ['Sunny', 'Cloudy', 'Light rain', 'Heavy rain', 'Storm', 'Harmattan haze', 'Windy'] },
      { name: 'notes', label: 'Notes', type: 'textarea' },
    ], done: 'Weather saved', submit: async (v) => {
      await call('add_weather', { input: { record_date: v.date, rainfall_mm: num(v.rain), temp_min_c: num(v.min), temp_max_c: num(v.max), conditions: v.conditions, notes: v.notes } });
      void weather(body);
    } });
    const d = el.closest<HTMLElement>('[data-del]');
    if (d && await confirmDialog({ title: 'Delete this reading?', message: 'It is removed from the rainfall totals.', confirm: 'Delete', danger: true }) !== null) { try { await call('delete_weather', { id: Number(d.dataset.del) }); void weather(body); } catch (x) { toast(errText(x), 'error'); } }
    if (el.closest('[data-csv]')) void exportCsv('weather.csv', ['Date', 'Rainfall mm', 'Min C', 'Max C', 'Conditions', 'Notes', 'By'], list.map((w) => [w.record_date, w.rainfall_mm ?? '', w.temp_min_c ?? '', w.temp_max_c ?? '', w.conditions, w.notes, w.user_name]));
  };
}

async function outlooks(body: HTMLElement) {
  const list = await call<Outlook[]>('list_outlooks');
  const canEdit = auth.can('manager') || auth.session?.role === 'adviser';
  body.innerHTML = `<div class="callout callout-info"><div><b>From NiMet's Seasonal Climate Prediction</b><span>Enter the expected onset and end of the rains for your state or LGA from the official NiMet publication, or import them in a FarmBuddy content update. FarmBuddy never invents these dates. The crop calendar shows them.</span></div></div>
    ${canEdit ? `<div><button class="btn btn-primary" data-add>${icon('plus', 16)} Add outlook</button></div>` : ''}
    ${tableHtml(['Year', 'State', 'LGA', 'Onset', 'Cessation', 'Season (days)#', 'Dry spells', 'Source', ''], list.map((o) => [String(o.year), esc(o.state), esc(o.lga || 'Whole state'), esc(o.onset_date ? dateLabel(o.onset_date) : '—'), esc(o.cessation_date ? dateLabel(o.cessation_date) : '—'), o.season_days === null ? '—' : String(o.season_days), esc(o.dry_spell_notes), `${esc(o.source)}${o.published_date ? ` <small class="muted">${esc(o.published_date)}</small>` : ''}`,
      canEdit ? `<button class="icon-btn" data-edit="${o.id}" aria-label="Edit">${icon('edit', 15)}</button><button class="icon-btn" data-del="${o.id}" aria-label="Delete">${icon('trash', 15)}</button>` : '']), 'No outlook saved yet.')}`;
  const form = (o: Outlook | null) => formModal({ title: o ? 'Edit outlook' : 'Add season outlook', fields: [
    { name: 'year', label: 'Year', type: 'number', value: o?.year ?? new Date().getFullYear(), required: true, pair: true },
    { name: 'state', label: 'State', type: 'select', options: stateOpts(), value: o?.state ?? (store.settings.business_state || ''), required: true },
    { name: 'lga', label: 'LGA (leave empty for the whole state)', list: LGAS[o?.state ?? ''] ?? [], value: o?.lga ?? '' },
    { name: 'onset', label: 'Expected onset of rains', type: 'date', value: o?.onset_date ?? '', pair: true },
    { name: 'cessation', label: 'Expected end of rains', type: 'date', value: o?.cessation_date ?? '' },
    { name: 'days', label: 'Length of season (days)', type: 'number', value: o?.season_days ?? '', pair: true },
    { name: 'published', label: 'Published date', type: 'date', value: o?.published_date ?? '' },
    { name: 'dry', label: 'Dry spell notes', type: 'textarea', value: o?.dry_spell_notes ?? '' },
    { name: 'source', label: 'Source', value: o?.source ?? 'NiMet Seasonal Climate Prediction', required: true },
    { name: 'notes', label: 'Notes', type: 'textarea', value: o?.notes ?? '' },
  ], done: 'Outlook saved', submit: async (v) => {
    await call('save_outlook', { input: { id: o?.id ?? 0, year: Math.round(num(v.year) ?? 0), state: v.state, lga: v.lga, onset_date: v.onset, cessation_date: v.cessation, season_days: num(v.days), dry_spell_notes: v.dry, notes: v.notes, source: v.source, published_date: v.published } });
    void outlooks(body);
  } });
  body.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-add]')) form(null);
    const ed = el.closest<HTMLElement>('[data-edit]');
    if (ed) form(list.find((o) => o.id === Number(ed.dataset.edit)) ?? null);
    const d = el.closest<HTMLElement>('[data-del]');
    if (d && await confirmDialog({ title: 'Delete this outlook?', message: 'The crop calendar will stop showing these dates.', confirm: 'Delete', danger: true }) !== null) { try { await call('delete_outlook', { id: Number(d.dataset.del) }); void outlooks(body); } catch (x) { toast(errText(x), 'error'); } }
  };
}

async function market(body: HTMLElement, product = '') {
  const [summary, list] = await Promise.all([call<PriceSummary[]>('market_price_summary'), call<Price[]>('list_market_prices', { product: product || null })]);
  body.innerHTML = `<div class="callout callout-warn"><div><b>Prices you or your team observed</b><span>Prices change by date, place, quality and quantity. Each entry keeps its date, market and source; FarmBuddy shows history, not a current price for everyone. Demo prices are illustrative only.</span></div></div>
    <div class="row-actions"><button class="btn btn-primary" data-add>${icon('plus', 16)} Record a price</button><button class="btn btn-ghost" data-csv>${icon('download', 16)} Export CSV</button></div>
    <section class="panel"><header class="panel-head"><h2>Latest by product and market</h2></header>
      ${tableHtml(['Product', 'Unit', 'Market', 'Latest', 'Wholesale#', 'Retail#', 'Lowest seen#', 'Highest seen#', 'Entries#', ''], summary.map((s) => [`<button class="link-btn" data-product="${esc(s.product)}">${esc(s.product)}</button>${s.demo_only ? ` ${pill('Demo', 'warn')}` : ''}`, esc(s.unit), esc(s.market), esc(dateLabel(s.latest_date)), m(s.latest_wholesale_kobo), m(s.latest_retail_kobo), m(s.min_kobo), m(s.max_kobo), String(s.observations), '']), 'No prices recorded yet.')}</section>
    <section class="panel"><header class="panel-head"><h2>${product ? `History: ${esc(product)}` : 'All entries'}</h2>${product ? '<button class="btn btn-ghost btn-sm" data-all>Show all</button>' : ''}</header>
      ${tableHtml(['Date', 'Product', 'Market', 'Unit', 'Wholesale#', 'Retail#', 'Buyer offer#', 'Source', ''], list.map((p) => [esc(dateLabel(p.price_date)), `${esc(p.product)}${p.is_demo ? ` ${pill('Demo', 'warn')}` : ''}`, esc([p.market, p.lga, p.state].filter(Boolean).join(', ')), esc(p.unit), m(p.wholesale_kobo), m(p.retail_kobo), m(p.buyer_offer_kobo), esc(p.source),
        auth.can('manager') ? `<button class="icon-btn" data-del="${p.id}" aria-label="Delete">${icon('trash', 15)}</button>` : '']))}</section>`;
  body.onclick = async (e) => {
    const el = e.target as HTMLElement;
    const pr = el.closest<HTMLElement>('[data-product]');
    if (pr) { void market(body, pr.dataset.product!); return; }
    if (el.closest('[data-all]')) { void market(body); return; }
    if (el.closest('[data-add]')) formModal({ title: 'Record a market price', intro: 'Enter at least one price.', fields: [
      { name: 'product', label: 'Product', required: true, list: [...new Set(summary.map((s) => s.product))], value: product, pair: true },
      { name: 'unit', label: 'Unit', required: true, placeholder: 'e.g. 50kg bag, 25 litres, crate of 30', list: [...new Set(summary.map((s) => s.unit))] },
      { name: 'date', label: 'Date', type: 'date', value: isoDate(new Date()), required: true, pair: true },
      { name: 'market', label: 'Market', placeholder: 'e.g. Itam Market' },
      { name: 'state', label: 'State', type: 'select', options: stateOpts(), value: store.settings.business_state || '', pair: true },
      { name: 'lga', label: 'LGA' },
      { name: 'wholesale', label: 'Wholesale price', type: 'money', pair: true },
      { name: 'retail', label: 'Retail price', type: 'money' },
      { name: 'offer', label: 'Buyer offer', type: 'money', pair: true },
      { name: 'available', label: 'Quantity available' },
      { name: 'source', label: 'Source', required: true, placeholder: 'e.g. Visited market, buyer phone call' },
      { name: 'notes', label: 'Notes (quality, grade)', type: 'textarea' },
    ], done: 'Price saved', submit: async (v) => {
      const k = (s: string) => (s.trim() ? toKobo(s) : null);
      await call('add_market_price', { input: { product: v.product, market: v.market, state: v.state, lga: v.lga, price_date: v.date, unit: v.unit, wholesale_kobo: k(v.wholesale), retail_kobo: k(v.retail), buyer_offer_kobo: k(v.offer), quantity_available: v.available, source: v.source, notes: v.notes } });
      void market(body, product);
    } });
    const d = el.closest<HTMLElement>('[data-del]');
    if (d && await confirmDialog({ title: 'Delete this price?', message: 'It is removed from the history.', confirm: 'Delete', danger: true }) !== null) { try { await call('delete_market_price', { id: Number(d.dataset.del) }); void market(body, product); } catch (x) { toast(errText(x), 'error'); } }
    if (el.closest('[data-csv]')) void exportCsv('market-prices.csv', ['Date', 'Product', 'Market', 'LGA', 'State', 'Unit', 'Wholesale NGN', 'Retail NGN', 'Buyer offer NGN', 'Source', 'Demo'], list.map((p) => [p.price_date, p.product, p.market, p.lga, p.state, p.unit, p.wholesale_kobo === null ? '' : p.wholesale_kobo / 100, p.retail_kobo === null ? '' : p.retail_kobo / 100, p.buyer_offer_kobo === null ? '' : p.buyer_offer_kobo / 100, p.source, p.is_demo ? 'yes' : '']));
  };
}
