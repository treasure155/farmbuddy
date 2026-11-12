import { auth, call } from '../api';
import { exportCsv } from '../lib/csv';
import { $, confirmDialog, errText, esc, toast } from '../lib/dom';
import { money, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { t, tl } from '../lib/i18n';
import { SPECIES } from '../lib/content/farm';
import { fmtNum, formModal, num, pill, reviewPill, tableHtml, tabsHtml } from '../lib/ui';
import type { Flock } from '../types';
import type { ViewCtx } from './shell';

interface Guide { id: number | null; species: string; age_from_day: number; age_to_day: number; grams_per_animal_day: number; source: string; review_status: string; reviewer: string; notes: string }
interface DayFeed { date: string; age_day: number; actual_kg: number; wasted_kg: number; expected_kg: number | null; guide_source: string; guide_reviewed: boolean }
interface BatchFeed { flock_id: number; code: string; species: string; status: string; age_days: number; alive: number; feed_kg: number; wasted_kg: number; wastage_pct: number; feed_per_animal_kg: number; feed_cost_kobo: number; feed_cost_per_animal_kobo: number; fcr: number; days: DayFeed[] }
interface Item { ingredient: string; quantity_kg: number; unit_cost_kobo: number; protein_pct: number | null; energy_kcal_kg: number | null }
interface Result { total_kg: number; total_cost_kobo: number; cost_per_kg_kobo: number; shares: Array<[string, number]>; protein_pct: number | null; energy_kcal_kg: number | null; warning: string }
interface Formulation { id: number; name: string; species: string; notes: string; user_name: string; created_at: string; items: Item[]; result: Result }

let tab = 'batches';
const speciesOpts = (): Array<[string, string]> => SPECIES.map((s) => [s, tl('species', s)]);

/** Feed: expected versus actual use per batch, wastage, comparison across batches, feed guides and a formulation calculator. */
export async function render(ctx: ViewCtx) {
  if (typeof ctx.params.tab === 'string') tab = ctx.params.tab;
  ctx.header(t('nav.feed', {}, 'Feed'), 'Feed used against your guides, wastage, cost per animal and feed mixes.');
  ctx.main.innerHTML = `${tabsHtml([['batches', 'By batch'], ['compare', 'Compare batches'], ['guides', 'Feed guides'], ['mix', 'Feed mix calculator']], tab)}<div data-body class="stack"></div>`;
  const body = $('[data-body]', ctx.main)!;
  const show = async () => {
    body.innerHTML = '<div class="loading"><span></span><span></span><span></span></div>';
    try {
      if (tab === 'batches') await batches(body);
      else if (tab === 'compare') await compare(body);
      else if (tab === 'guides') await guides(body);
      else await mixer(body);
    } catch (x) { body.innerHTML = `<p class="form-error">${esc(errText(x))}</p>`; }
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

async function batches(body: HTMLElement) {
  const flocks = await call<Flock[]>('list_flocks', { includeClosed: true, group: null });
  if (!flocks.length) { body.innerHTML = '<p class="muted">Start a livestock batch first. Feed is recorded in its daily records.</p>'; return; }
  body.innerHTML = `<label class="field"><span>Batch</span><select data-flock>${flocks.map((f) => `<option value="${f.id}">${esc(f.code)} · ${esc(tl('species', f.species))}${f.status === 'closed' ? ' (closed)' : ''}</option>`).join('')}</select></label><div data-feed></div>`;
  const out = $('[data-feed]', body)!;
  const load = async () => {
    const id = Number(($('[data-flock]', body) as HTMLSelectElement).value);
    const f = await call<BatchFeed>('get_batch_feed', { flockId: id });
    const anyGuide = f.days.some((d) => d.expected_kg !== null);
    const exp = f.days.reduce((s, d) => s + (d.expected_kg ?? 0), 0);
    const act = f.days.filter((d) => d.expected_kg !== null).reduce((s, d) => s + d.actual_kg, 0);
    out.innerHTML = `<div class="figures">
        <div><span>Feed used</span><b>${fmtNum(f.feed_kg)} kg</b><small class="muted">${fmtNum(f.feed_per_animal_kg, 2)} kg per animal</small></div>
        <div><span>Wasted</span><b class="${f.wastage_pct > 5 ? 'bad-text' : ''}">${fmtNum(f.wasted_kg)} kg</b><small class="muted">${fmtNum(f.wastage_pct)}% of feed given</small></div>
        <div><span>FCR</span><b>${f.fcr ? f.fcr.toFixed(2) : '—'}</b><small class="muted">feed ÷ live weight produced</small></div>
        ${auth.can('manager') ? `<div><span>Feed cost</span><b>${money(f.feed_cost_kobo, { whole: true })}</b><small class="muted">${money(f.feed_cost_per_animal_kobo)} per animal</small></div>` : ''}
      </div>
      ${anyGuide ? `<p class="small">Where a guide covers the age: expected <b>${fmtNum(exp)} kg</b>, actual <b>${fmtNum(act)} kg</b> (${exp ? `${fmtNum(((act - exp) / exp) * 100)}%` : '—'}).</p>` : '<div class="callout callout-info"><div><b>No feed guide covers this batch yet</b><span>Add the amount per animal per day from your feed bag label or supplier under Feed guides. FarmBuddy does not assume one amount for every breed.</span></div></div>'}
      ${tableHtml(['Date', 'Age (days)#', 'Given (kg)#', 'Wasted (kg)#', 'Expected (kg)#', 'Guide'], f.days.map((d) => [esc(d.date), String(d.age_day), fmtNum(d.actual_kg), d.wasted_kg ? fmtNum(d.wasted_kg) : '', d.expected_kg === null ? '—' : fmtNum(d.expected_kg), d.guide_source ? `${esc(d.guide_source)} ${d.guide_reviewed ? '' : pill('not reviewed', 'warn')}` : '']), 'No feed recorded yet. Enter feed in the daily record of the batch.')}`;
  };
  $('[data-flock]', body)!.addEventListener('change', () => void load().catch((x) => toast(errText(x), 'error')));
  await load();
}

async function compare(body: HTMLElement) {
  body.innerHTML = `<label class="field"><span>Type</span><select data-sp><option value="">All types</option>${speciesOpts().map(([k, l]) => `<option value="${k}">${esc(l)}</option>`).join('')}</select></label><div data-out></div>`;
  const load = async () => {
    const sp = ($('[data-sp]', body) as HTMLSelectElement).value;
    const rows = await call<BatchFeed[]>('compare_batch_feed', { species: sp || null });
    const money_ = auth.can('manager');
    $('[data-out]', body)!.innerHTML = tableHtml(['Batch', 'Type', 'Status', 'Age#', 'Feed (kg)#', 'Per animal (kg)#', 'Wastage %#', 'FCR#', ...(money_ ? ['Feed cost per animal#'] : [])],
      rows.map((r) => [esc(r.code), esc(tl('species', r.species)), esc(r.status), String(r.age_days), fmtNum(r.feed_kg), fmtNum(r.feed_per_animal_kg, 2), fmtNum(r.wastage_pct), r.fcr ? r.fcr.toFixed(2) : '—', ...(money_ ? [money(r.feed_cost_per_animal_kobo)] : [])]), 'No batches to compare yet.');
  };
  $('[data-sp]', body)!.addEventListener('change', () => void load());
  await load();
}

async function guides(body: HTMLElement) {
  const list = await call<Guide[]>('list_feed_guides');
  const manager = auth.can('manager');
  body.innerHTML = `<div class="callout callout-info"><div><b>Feed guides come from you</b><span>Enter the amount per animal per day for an age range, from the feed bag label, your feed supplier or a nutritionist, and name that source. A guide counts as reviewed only after an owner or adviser approves it.</span></div></div>
    ${manager ? `<div><button class="btn btn-primary" data-add>${icon('plus', 16)} Add feed guide</button></div>` : ''}
    ${tableHtml(['Type', 'Age (days)', 'Grams per animal per day#', 'Source', 'Status', ''], list.map((g) => [esc(tl('species', g.species)), `${g.age_from_day}–${g.age_to_day}`, fmtNum(g.grams_per_animal_day), esc(g.source), `${reviewPill(g.review_status)}${g.reviewer ? ` <small class="muted">${esc(g.reviewer)}</small>` : ''}`,
      manager ? `<button class="icon-btn" data-edit="${g.id}" aria-label="Edit">${icon('edit', 15)}</button><button class="icon-btn" data-del="${g.id}" aria-label="Delete">${icon('trash', 15)}</button>` : '']), 'No feed guides yet.')}`;
  const form = (g: Guide | null) => formModal({ title: g ? 'Edit feed guide' : 'Add feed guide', fields: [
    { name: 'species', label: 'Type', type: 'select', options: speciesOpts(), value: g?.species ?? 'broiler', pair: true },
    { name: 'grams', label: 'Grams per animal per day', type: 'number', value: g?.grams_per_animal_day ?? '', required: true },
    { name: 'from', label: 'From age (day)', type: 'number', value: g?.age_from_day ?? 0, required: true, pair: true },
    { name: 'to', label: 'To age (day)', type: 'number', value: g?.age_to_day ?? '', required: true },
    { name: 'source', label: 'Source', value: g?.source ?? '', required: true, placeholder: 'e.g. Feed bag label, supplier name' },
    ...(auth.can('owner') || auth.session?.role === 'adviser' ? [{ name: 'approved', label: 'Reviewed and approved', type: 'checkbox' as const, value: g?.review_status === 'approved', help: 'Your name is saved as the reviewer.' }] : []),
    { name: 'notes', label: 'Notes', type: 'textarea', value: g?.notes ?? '' },
  ], done: 'Feed guide saved', submit: async (v) => {
    await call('save_feed_guide', { input: { id: g?.id ?? null, species: v.species, age_from_day: Math.round(num(v.from) ?? 0), age_to_day: Math.round(num(v.to) ?? 0), grams_per_animal_day: num(v.grams) ?? 0, source: v.source, review_status: v.approved ? 'approved' : 'pending', reviewer: '', notes: v.notes } });
    void guides(body);
  } });
  body.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-add]')) form(null);
    const ed = el.closest<HTMLElement>('[data-edit]');
    if (ed) form(list.find((g) => g.id === Number(ed.dataset.edit)) ?? null);
    const d = el.closest<HTMLElement>('[data-del]');
    if (d && await confirmDialog({ title: 'Delete this feed guide?', message: 'Expected feed will no longer be shown for these ages.', confirm: 'Delete', danger: true }) !== null) {
      try { await call('delete_feed_guide', { id: Number(d.dataset.del) }); void guides(body); } catch (x) { toast(errText(x), 'error'); }
    }
  };
}

async function mixer(body: HTMLElement) {
  const saved = await call<Formulation[]>('list_formulations');
  const rowHtml = (i?: Item) => `<tr data-row><td><input name="ingredient" maxlength="80" value="${esc(i?.ingredient ?? '')}" placeholder="e.g. Maize"></td><td><input name="kg" inputmode="decimal" value="${i?.quantity_kg ?? ''}"></td>
    <td><input name="cost" inputmode="decimal" value="${i ? (i.unit_cost_kobo / 100) || '' : ''}" placeholder="per kg"></td><td><input name="protein" inputmode="decimal" value="${i?.protein_pct ?? ''}"></td><td><input name="energy" inputmode="decimal" value="${i?.energy_kcal_kg ?? ''}"></td>
    <td><button type="button" class="icon-btn" data-rm aria-label="Remove">${icon('trash', 15)}</button></td></tr>`;
  body.innerHTML = `<div class="callout callout-warn"><div><b>A calculator, not a recommendation</b><span>It adds up the quantities, cost and nutrients you enter. Use ingredient values from your supplier's analysis, and have a nutritionist check any ration before you feed it.</span></div></div>
    <div class="table-scroll"><table class="table compact form-table"><thead><tr><th>Ingredient</th><th>Quantity (kg)</th><th>Cost per kg</th><th>Crude protein %</th><th>Energy kcal/kg</th><th></th></tr></thead><tbody data-rows>${rowHtml()}${rowHtml()}${rowHtml()}</tbody></table></div>
    <div class="row-actions"><button class="btn btn-ghost" data-more>${icon('plus', 16)} Add ingredient</button><button class="btn btn-primary" data-calc>Calculate</button><button class="btn btn-ghost" data-save>Save this mix</button></div>
    <div data-result></div>
    <section class="panel"><header class="panel-head"><h2>Saved mixes</h2></header>${tableHtml(['Name', 'Type', 'Total (kg)#', 'Cost per kg#', 'Protein %#', 'Energy#', 'Saved by', ''], saved.map((f) => [esc(f.name), esc(f.species ? tl('species', f.species) : ''), fmtNum(f.result.total_kg), money(f.result.cost_per_kg_kobo), fmtNum(f.result.protein_pct), fmtNum(f.result.energy_kcal_kg, 0), esc(f.user_name),
      `<button class="icon-btn" data-load="${f.id}" aria-label="Open">${icon('edit', 15)}</button><button class="icon-btn" data-del="${f.id}" aria-label="Delete">${icon('trash', 15)}</button>`]), 'No saved mixes yet.')}
      ${saved.length ? `<button class="btn btn-ghost btn-sm" data-csv>${icon('download', 14)} Export CSV</button>` : ''}</section>`;
  const rows = $('[data-rows]', body)!;
  const items = (): Item[] => [...rows.querySelectorAll<HTMLTableRowElement>('[data-row]')].map((r) => {
    const g = (n: string) => (r.querySelector(`[name=${n}]`) as HTMLInputElement).value;
    return { ingredient: g('ingredient').trim(), quantity_kg: num(g('kg')) ?? 0, unit_cost_kobo: toKobo(g('cost') || '0'), protein_pct: num(g('protein')), energy_kcal_kg: num(g('energy')) };
  }).filter((i) => i.ingredient || i.quantity_kg);
  const result = $('[data-result]', body)!;
  const calc = async () => {
    try {
      const r = await call<Result>('calculate_formulation', { items: items() });
      result.innerHTML = `<div class="figures"><div><span>Total</span><b>${fmtNum(r.total_kg)} kg</b></div><div><span>Cost</span><b>${money(r.total_cost_kobo)}</b><small class="muted">${money(r.cost_per_kg_kobo)} per kg</small></div>
        <div><span>Crude protein</span><b>${r.protein_pct === null ? '—' : `${fmtNum(r.protein_pct)}%`}</b></div><div><span>Energy</span><b>${r.energy_kcal_kg === null ? '—' : `${fmtNum(r.energy_kcal_kg, 0)} kcal/kg`}</b></div></div>
        ${r.warning ? `<p class="warn-text small">${esc(r.warning)}</p>` : ''}
        ${tableHtml(['Ingredient', 'Share of mix %#'], r.shares.map(([n, s]) => [esc(n), fmtNum(s)]))}`;
    } catch (x) { result.innerHTML = `<p class="form-error">${esc(errText(x))}</p>`; }
  };
  body.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-more]')) rows.insertAdjacentHTML('beforeend', rowHtml());
    if (el.closest('[data-rm]')) el.closest('tr')?.remove();
    if (el.closest('[data-calc]')) void calc();
    if (el.closest('[data-save]')) {
      const list = items();
      formModal({ title: 'Save this mix', fields: [{ name: 'name', label: 'Name', required: true }, { name: 'species', label: 'For', type: 'select', options: [['', '—'], ...speciesOpts()] }, { name: 'notes', label: 'Notes', type: 'textarea' }], done: 'Mix saved', submit: async (v) => {
        await call('save_formulation', { name: v.name, species: v.species, notes: v.notes, items: list });
        void mixer(body);
      } });
    }
    const ld = el.closest<HTMLElement>('[data-load]');
    if (ld) { const f = saved.find((x) => x.id === Number(ld.dataset.load)); if (f) { rows.innerHTML = f.items.map(rowHtml).join(''); void calc(); } }
    const d = el.closest<HTMLElement>('[data-del]');
    if (d && await confirmDialog({ title: 'Delete this mix?', message: 'The saved mix is removed.', confirm: 'Delete', danger: true }) !== null) { await call('delete_formulation', { id: Number(d.dataset.del) }); void mixer(body); }
    if (el.closest('[data-csv]')) void exportCsv('feed-mixes.csv', ['Name', 'Ingredient', 'Kg', 'Cost per kg (NGN)', 'Protein %', 'Energy kcal/kg'], saved.flatMap((f) => f.items.map((i) => [f.name, i.ingredient, i.quantity_kg, i.unit_cost_kobo / 100, i.protein_pct ?? '', i.energy_kcal_kg ?? ''])));
  };
}

