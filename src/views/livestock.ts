import { auth, call } from '../api';
import { exportCsv, nairaPlain } from '../lib/csv';
import { $, confirmDialog, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { dateLabel, isoDate, money, nairaInput, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { t, tl } from '../lib/i18n';
import { codePrefix, countNoun, FISH, LIVESTOCK, LIVESTOCK_COSTS, POULTRY } from '../lib/content/farm';
import { store } from '../store';
import type { Farm, Flock, FlockEntry } from '../types';
import { animalsModal, breedingModal, eggLine, eggsForm, pondsModal, receiveForm, transferForm } from './livestock-extra';
import type { ViewCtx } from './shell';

let showClosed = false;
let group = '';
const cur = () => esc(store.settings.currency_symbol || '₦');
const EGGS = ['layer', 'noiler', 'duck', 'turkey', 'guinea_fowl', 'goose', 'other_poultry'];

/**
 * Livestock batches: poultry, other livestock and fish. Each batch runs from
 * placement to sale with deaths, culls, feed, water, eggs and weights recorded
 * as separate events, so no animal is counted twice.
 */
export async function render(ctx: ViewCtx) {
  const manager = auth.can('manager');
  if (typeof ctx.params.group === 'string') group = ctx.params.group;
  ctx.header(t('nav.livestock'), 'Every batch from placement to sale: deaths, feed, weight, eggs, costs and profit.', `<button class="btn btn-ghost" data-ponds>${icon('layers', 16)} Fish ponds</button><button class="btn btn-ghost" data-feed>${icon('chart', 16)} Feed</button>${manager ? `<button class="btn btn-primary" data-new>${icon('plus', 16)} New batch</button>` : ''}`);
  ctx.main.innerHTML = `<div class="toolbar">
      <div class="chips" role="group" aria-label="Type">${['', 'poultry', 'livestock', 'fish'].map((g) => `<button class="chip ${group === g ? 'is-on' : ''}" data-group="${g}">${esc(g ? t(`group.${g}`) : t('common.all'))}</button>`).join('')}</div>
      <label class="check inline"><input type="checkbox" data-closed ${showClosed ? 'checked' : ''}><span>${esc(t('common.show_closed'))}</span></label>
    </div><div data-list></div>`;
  const list = $('[data-list]', ctx.main)!;
  const load = async () => {
    const rows = (await call<Flock[]>('list_flocks', { includeClosed: showClosed, group: group || null })).filter((f) => store.inFarm(f.farm_id));
    list.innerHTML = rows.length ? `<div class="flock-grid">${rows.map((f) => `<button class="flock-card group-${f.group} ${f.status === 'closed' ? 'is-closed' : ''}" data-open="${f.id}">
        <span class="flock-top"><b>${esc(f.code)}</b><span>${f.is_demo ? `<span class="pill pill-warn">${esc(t('common.demo'))}</span> ` : ''}<span class="pill ${f.status === 'active' ? 'pill-ok' : 'pill-mute'}">${f.status === 'active' ? `Day ${f.age_days}` : 'Closed'}</span></span></span>
        <span class="muted small">${esc(tl('species', f.species))}${f.breed ? ` · ${esc(f.breed)}` : ''}${f.house ? ` · ${esc(f.house)}` : ''} · placed ${esc(dateLabel(f.placed_date))}</span>
        <span class="flock-figs">
          <span><small>Alive</small><b>${qty(f.alive)}</b></span>
          <span><small>Mortality</small><b class="${f.mortality_pct > 5 ? 'bad-text' : ''}">${f.mortality_pct.toFixed(1)}%</b></span>
          ${EGGS.includes(f.species) && f.eggs ? `<span><small>Eggs</small><b>${qty(f.eggs)}</b></span>` : `<span><small>FCR</small><b>${f.fcr ? f.fcr.toFixed(2) : '—'}</b></span>`}
          ${auth.can('manager') ? `<span><small>Profit</small><b class="${f.profit_kobo < 0 ? 'bad-text' : 'ok-text'}">${money(f.profit_kobo, { whole: true })}</b></span>` : ''}
        </span></button>`).join('')}</div>`
      : emptyState('No batches yet', manager ? 'Start a batch when birds, animals or fingerlings arrive. Then record deaths, feed and weights each day.' : 'Ask a manager to start a batch.', manager ? `<button class="btn btn-primary" data-new-inline>${icon('plus', 16)} New batch</button>` : '');
  };
  await load();
  ctx.main.addEventListener('change', (e) => { if ((e.target as HTMLElement).matches('[data-closed]')) { showClosed = (e.target as HTMLInputElement).checked; void load(); } });
  ctx.main.addEventListener('click', (e) => {
    const el = e.target as HTMLElement;
    const g = el.closest<HTMLElement>('[data-group]');
    if (g) { group = g.dataset.group ?? ''; ctx.main.querySelectorAll('[data-group]').forEach((c) => c.classList.toggle('is-on', c === g)); void load(); return; }
    const o = el.closest<HTMLElement>('[data-open]');
    if (o) void openBatch(Number(o.dataset.open), load, ctx);
    if (el.closest('[data-new-inline]')) void batchForm(null, load);
  });
  $('[data-new]')?.addEventListener('click', () => void batchForm(null, load));
  $('[data-ponds]')?.addEventListener('click', () => void pondsModal(() => void load()));
  $('[data-feed]')?.addEventListener('click', () => ctx.go('feed'));
  if (typeof ctx.params.open === 'number') void openBatch(ctx.params.open, load, ctx);
}

async function batchForm(f: Flock | null, done: () => void) {
  const [farms, ponds] = await Promise.all([call<Farm[]>('list_farms').catch(() => [] as Farm[]), call<Array<{ id: number; code: string; status: string }>>('list_ponds', { includeArchived: false }).catch(() => [])]);
  const species = f?.species ?? (group === 'fish' ? 'catfish' : group === 'livestock' ? 'goat' : 'broiler');
  const optgroup = (label: string, list: string[]) => `<optgroup label="${esc(label)}">${list.map((s) => `<option value="${s}" ${s === species ? 'selected' : ''}>${esc(tl('species', s))}</option>`).join('')}</optgroup>`;
  const defaultCode = (sp: string) => `${codePrefix[sp] ?? 'LV'}-${isoDate(new Date()).slice(0, 7)}`;
  const m = modal({ title: f ? `Edit ${f.code}` : 'New livestock batch', size: 'md', body: `<form class="stack">
    <div class="grid-2"><label class="field"><span>Type</span><select name="species">${optgroup(t('group.poultry'), POULTRY)}${optgroup(t('group.livestock'), LIVESTOCK)}${optgroup(t('group.fish'), FISH)}</select></label>
      <label class="field"><span>Batch code</span><input name="code" required maxlength="40" value="${esc(f?.code ?? defaultCode(species))}"></label></div>
    <div class="grid-2"><label class="field"><span>Date placed or bought</span><input type="date" name="placed_date" required value="${esc(f?.placed_date ?? isoDate(new Date()))}"></label>
      <label class="field"><span data-count-label>Number placed (${esc(countNoun(species))})</span><input name="count" inputmode="numeric" required value="${f?.chicks_placed ?? ''}"></label></div>
    <div class="grid-2"><label class="field"><span>Purchase cost (total)</span><span class="input-affix"><span class="affix">${cur()}</span><input name="cost" inputmode="decimal" value="${nairaInput(f?.chick_cost_kobo ?? 0)}"></span><small>Transport and other costs can be added as batch costs.</small></label>
      <label class="field"><span>Production purpose</span><input name="purpose" maxlength="60" list="purposes" value="${esc(f?.purpose ?? '')}" placeholder="e.g. Meat, Eggs, Breeding"><datalist id="purposes"><option value="Meat"><option value="Eggs"><option value="Breeding"><option value="Table fish"><option value="Fattening"></datalist></label></div>
    <div class="grid-2"><label class="field"><span>Breed or strain</span><input name="breed" maxlength="60" value="${esc(f?.breed ?? '')}"></label>
      <label class="field"><span>House, pen or pond</span><input name="house" maxlength="60" value="${esc(f?.house ?? '')}"></label></div>
    <div class="grid-2"><label class="field"><span>Source (hatchery, seller)</span><input name="source" maxlength="120" value="${esc(f?.source ?? '')}"></label>
      <label class="field"><span>Farm</span><select name="farm_id"><option value="">—</option>${farms.map((x) => `<option value="${x.id}" ${f?.farm_id === x.id ? 'selected' : ''}>${esc(x.name)}</option>`).join('')}</select></label></div>
    <label class="field" data-pond-field ${FISH.includes(species) ? '' : 'hidden'}><span>Pond</span><select name="pond_id"><option value="">—</option>${ponds.map((p) => `<option value="${p.id}" ${f?.pond_id === p.id ? 'selected' : ''}>${esc(p.code)}</option>`).join('')}</select><small>Add ponds under Fish ponds.</small></label>
    <label class="field"><span>Notes</span><textarea name="notes" rows="2" maxlength="500">${esc(f?.notes ?? '')}</textarea></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  (form.elements.namedItem('species') as HTMLSelectElement).addEventListener('change', (e) => {
    const sp = (e.target as HTMLSelectElement).value;
    $('[data-count-label]', form)!.textContent = `Number placed (${countNoun(sp)})`;
    $('[data-pond-field]', form)!.hidden = !FISH.includes(sp);
    const code = form.elements.namedItem('code') as HTMLInputElement;
    if (!f && /^[A-Z]{2}-\d{4}-\d{2}$/.test(code.value)) code.value = defaultCode(sp);
  });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('save_flock', { input: { id: f?.id ?? null, code: v.code, species: v.species, breed: v.breed, house: v.house, placed_date: v.placed_date, chicks_placed: Math.round(Number(v.count) || 0),
          chick_cost_kobo: toKobo(v.cost), purpose: v.purpose, source: v.source, farm_id: v.farm_id ? Number(v.farm_id) : null, pond_id: v.pond_id && FISH.includes(v.species) ? Number(v.pond_id) : null, notes: v.notes } });
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Batch saved'); m.close(); done();
    });
  });
}

async function openBatch(id: number, onChange: () => void, ctx: ViewCtx) {
  const { flock: f, entries } = await call<{ flock: Flock; entries: FlockEntry[] }>('get_flock', { id });
  const manager = auth.can('manager');
  const active = f.status === 'active';
  const noun = countNoun(f.species);
  const eggs = EGGS.includes(f.species);
  const KIND: Record<string, string> = { log: 'Daily record', cost: 'Cost', sale: `${noun[0].toUpperCase()}${noun.slice(1)} sold`, receive: 'Received', transfer_out: 'Moved out', transfer_in: 'Moved in', eggs: 'Eggs', birth: 'Birth' };
  const breeds = f.group === 'livestock';
  const m = modal({ title: `${f.code} · ${tl('species', f.species)}`, subtitle: `${[f.breed, f.house, f.farm_name].filter(Boolean).join(' · ')}${f.breed || f.house || f.farm_name ? ' · ' : ''}placed ${dateLabel(f.placed_date)}${active ? ` · day ${f.age_days}` : ` · closed ${dateLabel(f.closed_date)}`}`, size: 'lg', body: `<div class="stack">
    ${f.is_demo ? `<div class="callout callout-warn"><div><b>${esc(t('common.demo_data'))}</b><span>This batch is fictional demonstration data with illustrative figures.</span></div></div>` : ''}
    <div class="figures"><div><span>${esc(noun[0].toUpperCase() + noun.slice(1))} alive</span><b>${qty(f.alive)}</b><small class="muted">of ${qty(f.total_in)} ${f.total_in === f.chicks_placed ? 'placed' : 'in total'} · ${f.survival_pct.toFixed(1)}% survival</small></div>
      <div><span>Mortality</span><b class="${f.mortality_pct > 5 ? 'bad-text' : ''}">${f.mortality_pct.toFixed(1)}%</b><small class="muted">${f.dead} dead ÷ ${qty(f.total_in)} ${f.total_in === f.chicks_placed ? 'placed' : 'in total'} · ${f.culled} culled</small></div>
      <div><span>Feed used</span><b>${qty(Math.round(f.feed_kg))} kg</b><small class="muted">FCR ${f.fcr ? f.fcr.toFixed(2) : '— (needs a weighing)'} · avg ${f.latest_weight_kg ? `${f.latest_weight_kg} kg` : '—'}</small></div>
      ${auth.can('manager') ? `<div><span>Profit so far</span><b class="${f.profit_kobo < 0 ? 'bad-text' : 'ok-text'}">${money(f.profit_kobo)}</b><small class="muted">Sales ${money(f.revenue_kobo, { whole: true })} − costs ${money(f.costs_kobo, { whole: true })}</small></div>` : ''}</div>
    <p class="muted small">${auth.can('manager') ? `Cost per ${esc(noun === 'fish' ? 'fish' : noun.replace(/s$/, ''))} ${money(f.cost_per_bird_kobo)}${f.cost_per_kg_kobo ? ` · cost per kg live weight ${money(f.cost_per_kg_kobo)}` : ''} · ` : ''}${qty(f.sold)} sold${f.sold_weight_kg ? ` (${qty(f.sold_weight_kg)} kg)` : ''}${eggs ? ` · ${qty(f.eggs)} eggs collected, ${qty(f.eggs_damaged)} damaged` : ''}${f.water_l ? ` · ${qty(f.water_l)} L water` : ''}.
      FCR = feed used ÷ live weight produced (sold weight + animals alive × latest average weight).${f.feed_wasted_kg ? ` ${qty(f.feed_wasted_kg)} kg feed wasted.` : ''}${f.pond_code ? ` Pond ${esc(f.pond_code)}.` : ''}
      ${f.total_in !== f.chicks_placed ? ` In total ${qty(f.total_in)} came in: ${qty(f.chicks_placed)} placed${f.received ? `, ${qty(f.received)} received` : ''}${f.transferred_in ? `, ${qty(f.transferred_in)} moved in` : ''}${f.born ? `, ${qty(f.born)} born` : ''}.` : ''}${f.transferred_out ? ` ${qty(f.transferred_out)} moved to other batches.` : ''}</p>
    ${eggs && (f.eggs || f.eggs_sold) ? `<p class="small"><b>Eggs:</b> ${eggLine(f)}</p>` : ''}
    <div class="row-actions">${active ? `<button class="btn btn-primary" data-entry="log">${icon('calendar', 16)} Daily record</button>${manager ? `<button class="btn btn-ghost" data-entry="cost">${icon('wallet', 16)} Add cost</button>` : ''}${auth.canSell() ? `<button class="btn btn-ghost" data-entry="sale">${icon('cash', 16)} ${esc(KIND.sale)}</button>` : ''}` : ''}
      <button class="btn btn-ghost" data-health>${icon('shield', 16)} Health and vaccination</button></div>
    <div class="row-actions">${active ? `<button class="btn btn-ghost btn-sm" data-x="receive">${icon('plus', 14)} Add animals</button><button class="btn btn-ghost btn-sm" data-x="transfer">${icon('truck', 14)} Move to another batch</button>${eggs ? `<button class="btn btn-ghost btn-sm" data-x="eggs">${icon('box', 14)} Eggs sold / used / lost</button>` : ''}` : ''}
      ${f.group !== 'fish' && f.group !== 'poultry' ? `<button class="btn btn-ghost btn-sm" data-x="animals">${icon('tag', 14)} Animals (${qty(f.animals_registered)})</button>` : ''}${breeds ? `<button class="btn btn-ghost btn-sm" data-x="breeding">${icon('people', 14)} Breeding and births</button>` : ''}
      <button class="btn btn-ghost btn-sm" data-x="feed">${icon('chart', 14)} Feed against guide</button></div>
    <div class="table-scroll"><table class="table compact"><thead><tr><th>Date</th><th>Entry</th><th>Details</th><th class="num">Deaths</th><th class="num">Feed</th>${eggs ? '<th class="num">Eggs</th>' : ''}<th class="num">Amount</th>${manager ? '<th></th>' : ''}</tr></thead><tbody>
      ${entries.map((e) => `<tr><td>${esc(dateLabel(e.date))}</td><td>${KIND[e.kind]}</td><td>${esc(e.description || '—')}${e.kind === 'log' && e.avg_weight_kg ? ` <small class="muted">avg ${e.avg_weight_kg} kg</small>` : ''}${e.kind === 'log' && e.water_l ? ` <small class="muted">${qty(e.water_l)} L water</small>` : ''}${e.kind === 'sale' && e.quantity ? ` <small class="muted">${qty(e.quantity)} kg</small>` : ''}<small class="muted block">${esc(e.user_name)}</small></td>
        <td class="num">${e.mortality || e.culls ? `${e.mortality}${e.culls ? ` + ${e.culls} culled` : ''}` : ''}</td><td class="num">${e.feed_kg ? `${qty(e.feed_kg)} kg` : ''}</td>${eggs ? `<td class="num">${e.eggs ? `${qty(e.eggs)}${e.eggs_damaged ? ` (${e.eggs_damaged} dmg)` : ''}` : ''}</td>` : ''}<td class="num">${e.amount_kobo ? money(e.amount_kobo) : ''}</td>
        ${manager ? `<td class="actions-col"><button class="icon-btn" data-del="${e.kind}:${e.id}" aria-label="Delete entry">${icon('trash', 15)}</button></td>` : ''}</tr>`).join('') || `<tr><td colspan="8" class="muted">No entries yet. Add the first daily record.</td></tr>`}
    </tbody></table></div>
    <div class="form-actions">
      ${auth.can('owner') ? `<button class="btn btn-ghost btn-danger-text push-left" data-delete-batch>${icon('trash', 16)} Delete batch</button>` : ''}
      ${manager ? `<button class="btn btn-ghost" data-edit>${icon('edit', 16)} Edit</button><button class="btn btn-ghost" data-export>${icon('download', 16)} Export</button><button class="btn ${active ? 'btn-ghost' : 'btn-primary'}" data-toggle>${active ? 'Close batch' : 'Reopen batch'}</button>` : ''}
    </div></div>` });
  const refresh = () => { m.close(); onChange(); void openBatch(id, onChange, ctx); };
  m.el.addEventListener('click', async (e) => {
    const el = e.target as HTMLElement;
    const en = el.closest<HTMLElement>('[data-entry]');
    if (en) void entryForm(f, en.dataset.entry as 'log' | 'cost' | 'sale', refresh);
    if (el.closest('[data-health]')) { m.close(); ctx.go('health', { flock: f.id }); return; }
    const x = el.closest<HTMLElement>('[data-x]');
    if (x) {
      const k = x.dataset.x;
      if (k === 'receive') receiveForm(f, refresh);
      if (k === 'transfer') void transferForm(f, refresh);
      if (k === 'eggs') eggsForm(f, refresh);
      if (k === 'animals') { m.close(); void animalsModal(f, () => { onChange(); void openBatch(id, onChange, ctx); }); }
      if (k === 'breeding') { m.close(); void breedingModal(f, () => { onChange(); void openBatch(id, onChange, ctx); }); }
      if (k === 'feed') { m.close(); ctx.go('feed', { tab: 'batches' }); }
      return;
    }
    const del = el.closest<HTMLElement>('[data-del]');
    if (del) {
      const [kind, eid] = del.dataset.del!.split(':');
      if (await confirmDialog({ title: 'Delete this entry?', message: kind === 'cost' ? 'Stock issued with this cost is not put back. Use a stock adjustment if needed.' : 'The batch figures will be recalculated.', confirm: 'Delete', danger: true }) === null) return;
      try { await call('delete_flock_entry', { kind, id: Number(eid) }); toast('Entry deleted'); refresh(); } catch (x) { toast(errText(x), 'error'); }
    }
    if (el.closest('[data-edit]')) { m.close(); void batchForm(f, () => { onChange(); void openBatch(id, onChange, ctx); }); }
    if (el.closest('[data-delete-batch]')) {
      if (await confirmDialog({ title: `Delete batch ${f.code}?`, message: 'All its daily records, costs, sales, health and vaccination records are deleted. Stock already issued to it is not put back. This cannot be undone. To keep the history, close the batch instead.', confirm: 'Delete batch', danger: true }) === null) return;
      try { await call('delete_flock', { id }); toast('Batch deleted'); m.close(); onChange(); } catch (x) { toast(errText(x), 'error'); }
    }
    if (el.closest('[data-toggle]')) {
      if (active && await confirmDialog({ title: `Close batch ${f.code}?`, message: 'Close it when the last animals are sold. You can reopen it later.', confirm: 'Close batch' }) === null) return;
      try { await call('close_flock', { id, closedDate: isoDate(new Date()), reopen: !active }); toast(active ? 'Batch closed' : 'Batch reopened'); refresh(); } catch (x) { toast(errText(x), 'error'); }
    }
    if (el.closest('[data-export]')) exportCsv(`batch-${f.code}.csv`, ['Date', 'Entry', 'Details', 'Deaths', 'Culls', 'Feed kg', 'Avg weight kg', 'Eggs', 'Eggs damaged', 'Water L', 'Amount', 'By'],
      entries.map((x) => [x.date, KIND[x.kind], x.description, x.mortality, x.culls, x.feed_kg, x.avg_weight_kg, x.eggs, x.eggs_damaged, x.water_l, nairaPlain(x.amount_kobo), x.user_name]));
  });
}

async function entryForm(f: Flock, kind: 'log' | 'cost' | 'sale', done: () => void) {
  if (kind === 'cost') await store.loadProducts();
  const materials = store.products.filter((p) => p.active && p.track_stock);
  const today = isoDate(new Date());
  const noun = countNoun(f.species);
  const eggs = EGGS.includes(f.species);
  const body = kind === 'log' ? `
      <div class="grid-2"><label class="field"><span>Deaths today</span><input name="mortality" inputmode="numeric" value="0" autofocus></label><label class="field"><span>Culled</span><input name="culls" inputmode="numeric" value="0"></label></div>
      <div class="grid-2"><label class="field"><span>Feed given (kg)</span><input name="feed_kg" inputmode="decimal" value="0"><small>Leave 0 if you issue feed from stock as a cost.</small></label><label class="field"><span>Average weight (kg)</span><input name="avg_weight_kg" inputmode="decimal" value="0"><small>From a sample weighing, if done.</small></label></div>
      ${eggs ? `<div class="grid-2"><label class="field"><span>Eggs collected</span><input name="eggs_collected" inputmode="numeric" value="0"></label><label class="field"><span>Eggs broken, cracked or dirty</span><input name="eggs_damaged" inputmode="numeric" value="0"></label></div>` : ''}
      <div class="grid-2"><label class="field"><span>Feed wasted (kg) <em class="muted">(${esc(t('common.optional'))})</em></span><input name="feed_wasted_kg" inputmode="decimal" value="0"><small>Spilled, wet or mouldy feed.</small></label>
      <label class="field"><span>Water used (litres) <em class="muted">(${esc(t('common.optional'))})</em></span><input name="water_l" inputmode="decimal" value="0"></label></div>
      <label class="field"><span>Notes and observations</span><input name="description" maxlength="300" placeholder="e.g. Animals active, weather hot"></label>
      <p class="muted small">Write unusual signs in Health records too. ${esc(t('notice.no_diagnosis'))}</p>`
    : kind === 'cost' ? `
      <label class="field"><span>Category</span><input name="category" required list="fc-cats" value="Feed"><datalist id="fc-cats">${LIVESTOCK_COSTS.map((c) => `<option value="${esc(c)}">`).join('')}</datalist></label>
      <div class="grid-2"><label class="field"><span>Take from stock <em class="muted">(${esc(t('common.optional'))})</em></span><select name="product_id"><option value="">Not from stock (paid directly)</option>${materials.map((p) => `<option value="${p.id}">${esc(p.name)}: ${qty(p.stock_qty)} ${esc(p.unit)}</option>`).join('')}</select></label>
      <label class="field"><span>Quantity from stock</span><input name="quantity" inputmode="decimal" value="0"><small>Feed issued in kg also counts toward FCR.</small></label></div>
      <label class="field"><span>Amount</span><span class="input-affix"><span class="affix">${cur()}</span><input name="amount" inputmode="decimal" placeholder="Worked out from the stock cost if left empty"></span></label>
      <label class="field"><span>Details</span><input name="description" maxlength="300"></label>`
    : `
      <div class="grid-2"><label class="field"><span>Number sold (${esc(noun)})</span><input name="birds" inputmode="numeric" required autofocus><small>${qty(f.alive)} alive now.</small></label><label class="field"><span>Total live weight (kg)</span><input name="live_weight_kg" inputmode="decimal" value="0"></label></div>
      <div class="grid-2"><label class="field"><span>Amount received</span><span class="input-affix"><span class="affix">${cur()}</span><input name="amount" inputmode="decimal" required></span></label><label class="field"><span>Receipt no. <em class="muted">(${esc(t('common.optional'))})</em></span><input name="receipt_no" maxlength="40"></label></div>
      <label class="field"><span>Buyer</span><input name="buyer" maxlength="120"></label>
      <p class="muted small">If you also gave a receipt for this sale, enter its number here. Batch profit is worked out from this record.</p>`;
  const title = { log: 'Daily record', cost: 'Add batch cost', sale: 'Record a sale' }[kind];
  const m = modal({ title, subtitle: `${f.code} · ${tl('species', f.species)}`, size: 'md', body: `<form class="stack">
    <label class="field"><span>Date</span><input type="date" name="date" value="${today}" max="${today}" min="${esc(f.placed_date)}" required></label>
    ${body}
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const n = (k: string) => Number((v[k] ?? '').replace(',', '.')) || 0;
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('add_flock_entry', { input: {
          flock_id: f.id, kind, date: v.date, mortality: Math.round(n('mortality')), culls: Math.round(n('culls')), feed_kg: n('feed_kg'), avg_weight_kg: n('avg_weight_kg'),
          eggs_collected: Math.round(n('eggs_collected')), eggs_damaged: Math.round(n('eggs_damaged')), water_l: n('water_l'), feed_wasted_kg: n('feed_wasted_kg'),
          category: v.category ?? '', description: v.description ?? '', product_id: v.product_id ? Number(v.product_id) : null, quantity: n('quantity'),
          amount_kobo: v.amount ? toKobo(v.amount) : 0, birds: Math.round(n('birds')), live_weight_kg: n('live_weight_kg'), buyer: v.buyer ?? '', receipt_no: v.receipt_no ?? '',
        } });
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast(t('common.saved')); m.close(); done();
    });
  });
}

