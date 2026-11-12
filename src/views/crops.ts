import { auth, call } from '../api';
import { exportCsv, nairaPlain } from '../lib/csv';
import { $, $$, confirmDialog, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { dateLabel, isoDate, money, nairaInput, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { t, tl } from '../lib/i18n';
import { ACTIVITY_TYPES, CROPS, HARVEST_UNITS } from '../lib/content/farm';
import { AREA_UNITS } from '../lib/content/nigeria';
import { store } from '../store';
import type { CropActivity, CropCycle, Farm, Harvest, Plot } from '../types';
import type { ViewCtx } from './shell';
import { calendar, cropHealth, fields, varieties } from './crops-extra';

type Tab = 'crops' | 'plots' | 'calendar' | 'health' | 'varieties' | 'fields';
let tab: Tab = 'crops';
let showClosed = false;
const cur = () => esc(store.settings.currency_symbol || '₦');
const unitName = (u: string) => u.replace(/_/g, ' ');

/** Crop farming: plots, crops planted on them, field work and harvests. */
export async function render(ctx: ViewCtx) {
  const manager = auth.can('manager');
  if (ctx.params.tab) tab = ctx.params.tab as Tab;
  ctx.header(t('nav.crops'), 'Each planting from land preparation to harvest, with costs, yield and losses.', manager ? `
    <button class="btn btn-ghost" data-new-plot>${icon('plus', 16)} Plot</button><button class="btn btn-primary" data-new-crop>${icon('plus', 16)} Plant a crop</button>` : '');
  ctx.main.innerHTML = `<div class="tabs" role="tablist"><button role="tab" class="tab" data-tab="crops">Crops</button><button role="tab" class="tab" data-tab="plots">Plots</button><button role="tab" class="tab" data-tab="calendar">Calendar</button><button role="tab" class="tab" data-tab="health">Crop health</button><button role="tab" class="tab" data-tab="fields">Fields and history</button><button role="tab" class="tab" data-tab="varieties">Varieties</button></div>
    <div class="toolbar"><label class="check inline"><input type="checkbox" data-closed ${showClosed ? 'checked' : ''}><span>${esc(t('common.show_closed'))}</span></label></div><div data-pane></div>`;
  const pane = $('[data-pane]', ctx.main)!;
  let plots: Plot[] = [];

  const show = async () => {
    $$('[data-tab]', ctx.main).forEach((b) => b.classList.toggle('is-on', b.dataset.tab === tab));
    const extra: Record<string, (p: HTMLElement) => Promise<void>> = { calendar, health: cropHealth, varieties, fields };
    ($('[data-closed]', ctx.main)!.closest('.toolbar') as HTMLElement).hidden = tab in extra;
    if (tab in extra) { pane.onclick = null; pane.onchange = null; await extra[tab](pane); return; }
    pane.onclick = null; pane.onchange = null;
    plots = (await call<Plot[]>('list_plots', { includeArchived: showClosed })).filter((p) => store.inFarm(p.farm_id));
    if (tab === 'plots') {
      pane.innerHTML = plots.length ? `<div class="table-scroll"><table class="table"><thead><tr><th>Plot</th><th class="num">Area</th><th>Growing now</th><th>Location</th><th class="num">Crops so far</th><th></th></tr></thead><tbody>
        ${plots.map((p) => `<tr class="${p.status === 'archived' ? 'is-muted' : ''}"><td><b>${esc(p.code)}</b>${p.name ? ` ${esc(p.name)}` : ''}${p.is_demo ? ` <span class="pill pill-warn">${esc(t('common.demo'))}</span>` : ''}${p.farm_name ? `<small class="muted block">${esc(p.farm_name)}</small>` : ''}</td>
          <td class="num">${p.area ? `${qty(p.area)} ${esc(unitName(p.area_unit))}` : '—'}</td><td>${esc(p.current_crop || '—')}</td><td>${esc(p.location || '—')}</td><td class="num">${p.cycles}</td>
          <td class="actions-col">${manager ? `<button class="icon-btn" data-edit-plot="${p.id}" aria-label="Edit">${icon('edit', 15)}</button><button class="icon-btn" data-archive-plot="${p.id}" data-restore="${p.status === 'archived' ? 1 : 0}" aria-label="${p.status === 'archived' ? 'Restore' : 'Archive'}">${icon(p.status === 'archived' ? 'undo' : 'box', 15)}</button>` : ''}</td></tr>`).join('')}
        </tbody></table></div>` : emptyState('No plots yet', 'Add each field or plot with its size. Sizes can be in hectares, acres, plots or your own unit.', manager ? `<button class="btn btn-primary" data-new-plot>${icon('plus', 16)} Add plot</button>` : '');
      return;
    }
    const plotIds = new Set(plots.map((p) => p.id));
    const cycles = (await call<CropCycle[]>('list_crop_cycles', { includeClosed: showClosed })).filter((c) => !store.farmId || plotIds.has(c.plot_id));
    pane.innerHTML = cycles.length ? `<div class="flock-grid">${cycles.map((c) => `<button class="flock-card crop-card ${c.status === 'closed' ? 'is-closed' : ''}" data-open="${c.id}">
        <span class="flock-top"><b>${esc(c.crop)}${c.variety ? ` <small class="muted">${esc(c.variety)}</small>` : ''}</b><span>${c.is_demo ? `<span class="pill pill-warn">${esc(t('common.demo'))}</span> ` : ''}<span class="pill ${c.status === 'growing' ? 'pill-ok' : 'pill-mute'}">${c.status === 'growing' ? `Day ${c.days_since_planting}` : 'Closed'}</span></span></span>
        <span class="muted small">${esc(c.plot_code)}${c.area ? ` · ${qty(c.area)} ${esc(unitName(c.area_unit))}` : ''} · planted ${esc(dateLabel(c.planting_date))}${c.expected_harvest_from ? ` · harvest from ${esc(dateLabel(c.expected_harvest_from))}` : ''}</span>
        <span class="flock-figs">
          ${auth.can('manager') ? `<span><small>Costs</small><b>${money(c.total_cost_kobo, { whole: true })}</b></span>` : ''}
          <span><small>Harvested</small><b>${c.harvests[0] ? `${qty(c.harvests[0].quantity)} ${esc(c.harvests[0].unit)}` : '—'}</b></span>
          ${auth.can('manager') ? `<span><small>Received</small><b>${money(c.revenue_kobo, { whole: true })}</b></span>` : ''}
          <span><small>Last work</small><b>${c.last_activity_date ? esc(dateLabel(c.last_activity_date)) : '—'}</b></span>
        </span></button>`).join('')}</div>`
      : emptyState('No crops recorded', plots.length ? 'Record each planting: the crop, the plot and the date. Then add field work and harvests.' : 'Add a plot first, then record what you planted on it.', manager ? `<button class="btn btn-primary" data-${plots.length ? 'new-crop' : 'new-plot'}>${icon('plus', 16)} ${plots.length ? 'Plant a crop' : 'Add plot'}</button>` : '');
  };

  const plotsReady = async () => { if (!plots.length) plots = await call<Plot[]>('list_plots', {}); return plots; };
  const onClick = async (e: Event) => {
    const el = e.target as HTMLElement;
    const tb = el.closest<HTMLElement>('[data-tab]');
    if (tb) { tab = tb.dataset.tab as Tab; void show(); return; }
    if (el.closest('[data-new-plot]')) { plotForm(null, show); return; }
    if (el.closest('[data-new-crop]')) { const p = await plotsReady(); if (!p.length) { toast('Add a plot first', 'info'); plotForm(null, show); } else cycleForm(p, null, show); return; }
    const ep = el.closest<HTMLElement>('[data-edit-plot]');
    if (ep) { plotForm(plots.find((p) => p.id === Number(ep.dataset.editPlot)) ?? null, show); return; }
    const ap = el.closest<HTMLElement>('[data-archive-plot]');
    if (ap) {
      const restore = ap.dataset.restore === '1';
      try { await call('archive_plot', { id: Number(ap.dataset.archivePlot), restore }); toast(restore ? 'Plot restored' : 'Plot archived'); void show(); } catch (x) { toast(errText(x), 'error'); }
      return;
    }
    const o = el.closest<HTMLElement>('[data-open]');
    if (o) void openCycle(Number(o.dataset.open), show);
  };
  ctx.main.addEventListener('click', onClick);
  $<HTMLElement>('[data-actions]')!.onclick = onClick;
  ctx.main.addEventListener('change', (e) => { if ((e.target as HTMLElement).matches('[data-closed]')) { showClosed = (e.target as HTMLInputElement).checked; void show(); } });
  await show();
  if (typeof ctx.params.open === 'number') void openCycle(ctx.params.open, show);
}

async function plotForm(p: Plot | null, done: () => void) {
  const farms = await call<Farm[]>('list_farms').catch(() => [] as Farm[]);
  const m = modal({ title: p ? `Edit ${p.code}` : 'Add a plot or field', size: 'md', body: `<form class="stack">
    <div class="grid-2"><label class="field"><span>Plot code</span><input name="code" required maxlength="30" value="${esc(p?.code ?? '')}" placeholder="e.g. PLOT-A" autofocus></label>
      <label class="field"><span>Name</span><input name="name" maxlength="80" value="${esc(p?.name ?? '')}" placeholder="e.g. Behind the house"></label></div>
    <div class="grid-2"><label class="field"><span>Area</span><input name="area" inputmode="decimal" value="${p?.area ?? ''}"></label>
      <label class="field"><span>Unit</span><input name="area_unit" list="area-units" required maxlength="30" value="${esc(p?.area_unit ?? 'hectare')}"><datalist id="area-units">${AREA_UNITS.map((u) => `<option value="${u}">`).join('')}</datalist><small>A local “plot” differs by place: say what yours means in the notes.</small></label></div>
    <div class="grid-2"><label class="field"><span>Location</span><input name="location" maxlength="200" value="${esc(p?.location ?? '')}"></label>
      <label class="field"><span>Farm</span><select name="farm_id"><option value="">—</option>${farms.map((f) => `<option value="${f.id}" ${p?.farm_id === f.id ? 'selected' : ''}>${esc(f.name)}</option>`).join('')}</select></label></div>
    <div class="grid-2"><label class="field"><span>Soil observations</span><input name="soil_notes" maxlength="500" value="${esc(p?.soil_notes ?? '')}"></label>
      <label class="field"><span>Irrigation</span><input name="irrigation" maxlength="100" value="${esc(p?.irrigation ?? '')}" placeholder="e.g. Rain-fed"></label></div>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('save_plot', { input: { id: p?.id ?? null, farm_id: v.farm_id ? Number(v.farm_id) : null, code: v.code, name: v.name, area: Number(v.area) || 0, area_unit: v.area_unit, location: v.location, soil_notes: v.soil_notes, irrigation: v.irrigation } }); }
      catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Plot saved'); m.close(); done();
    });
  });
}

function cycleForm(plots: Plot[], c: CropCycle | null, done: () => void) {
  const m = modal({ title: c ? `Edit ${c.crop} on ${c.plot_code}` : 'Plant a crop', size: 'md', body: `<form class="stack">
    <div class="grid-2"><label class="field"><span>Crop</span><input name="crop" required list="crop-list" maxlength="60" value="${esc(c?.crop ?? '')}" autofocus><datalist id="crop-list">${CROPS.map((x) => `<option value="${esc(x)}">`).join('')}</datalist></label>
      <label class="field"><span>Variety</span><input name="variety" maxlength="80" value="${esc(c?.variety ?? '')}"></label></div>
    <div class="grid-2"><label class="field"><span>Plot</span><select name="plot_id" required>${plots.filter((p) => p.status === 'active' || p.id === c?.plot_id).map((p) => `<option value="${p.id}" ${c?.plot_id === p.id ? 'selected' : ''}>${esc(p.code)}${p.name ? ` · ${esc(p.name)}` : ''}${p.area ? ` (${qty(p.area)} ${esc(unitName(p.area_unit))})` : ''}</option>`).join('')}</select></label>
      <label class="field"><span>Planting date</span><input type="date" name="planting_date" required value="${esc(c?.planting_date ?? isoDate(new Date()))}"></label></div>
    <div class="grid-3"><label class="field"><span>Area planted</span><input name="area_planted" inputmode="decimal" value="${c?.area ?? ''}"><small>Leave empty to use the plot size.</small></label>
      <label class="field"><span>Quantity planted</span><input name="quantity_planted" inputmode="decimal" value="${c?.quantity_planted || ''}"></label>
      <label class="field"><span>Unit</span><input name="planting_unit" maxlength="30" value="${esc(c?.planting_unit ?? '')}" placeholder="e.g. bundle, kg, sucker"></label></div>
    <div class="grid-2"><label class="field"><span>Seed or planting material source</span><input name="material_source" maxlength="150" value="${esc(c?.material_source ?? '')}"></label>
      <label class="field"><span>Cost of planting material</span><span class="input-affix"><span class="affix">${cur()}</span><input name="planting_cost" inputmode="decimal" value="${nairaInput(c?.planting_cost_kobo ?? 0)}"></span></label></div>
    <div class="grid-2"><label class="field"><span>Expected harvest from</span><input type="date" name="expected_harvest_from" value="${esc(c?.expected_harvest_from ?? '')}"></label>
      <label class="field"><span>Expected harvest until</span><input type="date" name="expected_harvest_to" value="${esc(c?.expected_harvest_to ?? '')}"></label></div>
    <div class="grid-2"><label class="field"><span>Planting method</span><input name="method" maxlength="80" value="${esc(c?.method ?? '')}" placeholder="e.g. Ridges, mounds, flat"></label>
      <label class="field"><span>Purpose</span><input name="purpose" maxlength="80" value="${esc(c?.purpose ?? '')}" placeholder="e.g. Sale, processing, home use"></label></div>
    <label class="field"><span>Notes</span><textarea name="notes" rows="2" maxlength="1000">${esc(c?.notes ?? '')}</textarea></label>
    <p class="muted small">Use your own planting date and expected harvest. Timing differs by state, variety and season; NiMet’s seasonal forecast can help you plan.</p>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('save_crop_cycle', { input: { id: c?.id ?? null, plot_id: Number(v.plot_id), crop: v.crop, variety: v.variety, planting_date: v.planting_date, area_planted: Number(v.area_planted) || 0,
          quantity_planted: Number(v.quantity_planted) || 0, planting_unit: v.planting_unit, material_source: v.material_source, planting_cost_kobo: toKobo(v.planting_cost), method: v.method,
          expected_harvest_from: v.expected_harvest_from, expected_harvest_to: v.expected_harvest_to, purpose: v.purpose, notes: v.notes } });
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Crop saved'); m.close(); done();
    });
  });
}

async function openCycle(id: number, onChange: () => void) {
  const { cycle: c, activities, harvests } = await call<{ cycle: CropCycle; activities: CropActivity[]; harvests: Harvest[] }>('get_crop_cycle', { id });
  const manager = auth.can('manager');
  const growing = c.status === 'growing';
  const main = c.harvests[0];
  const m = modal({ title: `${c.crop}${c.variety ? ` (${c.variety})` : ''} on ${c.plot_code}`, subtitle: `Planted ${dateLabel(c.planting_date)} · day ${c.days_since_planting}${c.area ? ` · ${qty(c.area)} ${unitName(c.area_unit)}` : ''}${growing ? '' : ` · closed ${dateLabel(c.closed_date)}`}`, size: 'lg', body: `<div class="stack">
    ${c.is_demo ? `<div class="callout callout-warn"><div><b>${esc(t('common.demo_data'))}</b><span>This crop is fictional demonstration data.</span></div></div>` : ''}
    <div class="figures">${auth.can('manager') ? `<div><span>Cost of production</span><b>${money(c.total_cost_kobo)}</b><small class="muted">Planting ${money(c.planting_cost_kobo, { whole: true })} · work ${money(c.activity_cost_kobo, { whole: true })} · harvest ${money(c.harvest_cost_kobo, { whole: true })}</small></div>` : ''}
      <div><span>Harvested</span><b>${main ? `${qty(main.quantity)} ${esc(main.unit)}` : '—'}</b><small class="muted">${main ? `${qty(main.stored)} stored · ${qty(main.sold)} sold` : 'No harvest yet'}${c.harvests.length > 1 ? ` · +${c.harvests.length - 1} other unit(s)` : ''}</small></div>
      <div><span>Yield</span><b>${c.yield_per_area ? `${qty(Math.round(c.yield_per_area * 100) / 100)} ${esc(main?.unit ?? '')}/${esc(unitName(c.area_unit))}` : '—'}</b><small class="muted">Loss ${c.loss_pct ? `${c.loss_pct.toFixed(1)}% damaged` : '—'}</small></div>
      ${auth.can('manager') ? `<div><span>Received from sales</span><b>${money(c.revenue_kobo)}</b><small class="muted">Margin so far <span class="${c.margin_kobo < 0 ? 'bad-text' : 'ok-text'}">${money(c.margin_kobo, { whole: true })}</span>. Produce in store is not counted as income.</small></div>` : ''}</div>
    <div class="row-actions">${growing ? `<button class="btn btn-primary" data-act>${icon('calendar', 16)} Record field work</button><button class="btn btn-ghost" data-harvest>${icon('box', 16)} Record harvest</button>` : ''}</div>
    <h3 class="section-title">Field work</h3>
    <div class="table-scroll"><table class="table compact"><thead><tr><th>Date</th><th>Activity</th><th>Details</th><th class="num">Input</th><th class="num">Cost</th>${manager ? '<th></th>' : ''}</tr></thead><tbody>
      ${activities.map((a) => `<tr><td>${esc(dateLabel(a.activity_date))}</td><td>${esc(tl('act', a.activity_type))}</td><td>${esc(a.description || '—')}${a.area_done ? ` <small class="muted">${qty(a.area_done)} ${esc(unitName(c.area_unit))}</small>` : ''}<small class="muted block">${esc(a.user_name)}</small></td>
        <td class="num">${a.product_name ? `${qty(a.input_qty)} ${esc(a.input_unit)} ${esc(a.product_name)}` : ''}</td><td class="num">${money(a.labour_kobo + a.other_cost_kobo + a.input_cost_kobo)}</td>
        ${manager ? `<td class="actions-col"><button class="icon-btn" data-del="activity:${a.id}" aria-label="Delete">${icon('trash', 15)}</button></td>` : ''}</tr>`).join('') || `<tr><td colspan="6" class="muted">No field work recorded yet.</td></tr>`}
    </tbody></table></div>
    <h3 class="section-title">Harvests</h3>
    <div class="table-scroll"><table class="table compact"><thead><tr><th>Date</th><th class="num">Harvested</th><th class="num">Damaged</th><th class="num">Stored</th><th class="num">Sold</th><th class="num">Received</th>${manager ? '<th></th>' : ''}</tr></thead><tbody>
      ${harvests.map((h) => `<tr><td>${esc(dateLabel(h.harvest_date))}${h.grade ? `<small class="muted block">Grade ${esc(h.grade)}</small>` : ''}</td><td class="num">${qty(h.quantity)} ${esc(h.unit)}</td><td class="num">${qty(h.damaged_qty)}</td>
        <td class="num">${qty(h.stored_qty)}${h.product_name ? `<small class="muted block">→ ${esc(h.product_name)}</small>` : ''}</td><td class="num">${qty(h.sold_qty)}${h.buyer ? `<small class="muted block">${esc(h.buyer)}</small>` : ''}</td><td class="num">${money(h.sale_amount_kobo)}</td>
        ${manager ? `<td class="actions-col"><button class="icon-btn" data-del="harvest:${h.id}" aria-label="Delete">${icon('trash', 15)}</button></td>` : ''}</tr>`).join('') || `<tr><td colspan="7" class="muted">No harvest recorded yet.</td></tr>`}
    </tbody></table></div>
    <p class="muted small">${esc(t('notice.no_chemicals'))}</p>
    <div class="form-actions">${manager ? `<button class="btn btn-ghost" data-edit>${icon('edit', 16)} Edit</button><button class="btn btn-ghost" data-export>${icon('download', 16)} Export</button><button class="btn ${growing ? 'btn-ghost' : 'btn-primary'}" data-toggle>${growing ? 'Close crop' : 'Reopen crop'}</button>` : ''}</div>
  </div>` });
  const refresh = () => { m.close(); onChange(); void openCycle(id, onChange); };
  m.el.addEventListener('click', async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-act]')) void activityForm(c, refresh);
    if (el.closest('[data-harvest]')) void harvestForm(c, refresh);
    if (el.closest('[data-edit]')) { const plots = await call<Plot[]>('list_plots', { includeArchived: true }); m.close(); cycleForm(plots, c, () => { onChange(); void openCycle(id, onChange); }); }
    const del = el.closest<HTMLElement>('[data-del]');
    if (del) {
      const [kind, eid] = del.dataset.del!.split(':');
      if (await confirmDialog({ title: 'Delete this entry?', message: 'Stock already issued or received is not changed back. Use a stock adjustment if needed.', confirm: 'Delete', danger: true }) === null) return;
      try { await call('delete_crop_entry', { kind, id: Number(eid) }); toast('Deleted'); refresh(); } catch (x) { toast(errText(x), 'error'); }
    }
    if (el.closest('[data-toggle]')) {
      if (growing && await confirmDialog({ title: 'Close this crop?', message: 'Close it when the harvest is finished. You can reopen it later.', confirm: 'Close crop' }) === null) return;
      try { await call('close_crop_cycle', { id, closedDate: isoDate(new Date()), reopen: !growing }); toast(growing ? 'Crop closed' : 'Crop reopened'); refresh(); } catch (x) { toast(errText(x), 'error'); }
    }
    if (el.closest('[data-export]')) exportCsv(`crop-${c.crop}-${c.plot_code}.csv`, ['Date', 'Type', 'Activity / unit', 'Details', 'Quantity', 'Cost or amount'], [
      ...activities.map((a) => [a.activity_date, 'Field work', tl('act', a.activity_type), a.description, a.product_name ? `${a.input_qty} ${a.input_unit} ${a.product_name}` : '', nairaPlain(a.labour_kobo + a.other_cost_kobo + a.input_cost_kobo)]),
      ...harvests.map((h) => [h.harvest_date, 'Harvest', h.unit, `damaged ${h.damaged_qty}, stored ${h.stored_qty}, sold ${h.sold_qty}`, h.quantity, nairaPlain(h.sale_amount_kobo)]),
    ]);
  });
}

async function activityForm(c: CropCycle, done: () => void) {
  const manager = auth.can('manager');
  if (manager) await store.loadProducts();
  const inputs = store.products.filter((p) => p.active && p.track_stock && p.kind === 'material');
  const m = modal({ title: 'Record field work', subtitle: `${c.crop} on ${c.plot_code}`, size: 'md', body: `<form class="stack">
    <div class="grid-2"><label class="field"><span>Date</span><input type="date" name="date" required value="${isoDate(new Date())}" min="${esc(c.planting_date)}"></label>
      <label class="field"><span>Activity</span><select name="type">${ACTIVITY_TYPES.map((a) => `<option value="${a}">${esc(tl('act', a))}</option>`).join('')}</select></label></div>
    <div class="grid-2"><label class="field"><span>Details</span><input name="description" maxlength="300" placeholder="What was done, pests seen, weather"></label>
      <label class="field"><span>Area covered (${esc(unitName(c.area_unit))})</span><input name="area_done" inputmode="decimal" value="0"></label></div>
    ${manager ? `<div class="grid-2"><label class="field"><span>Labour cost</span><span class="input-affix"><span class="affix">${cur()}</span><input name="labour" inputmode="decimal"></span></label>
      <label class="field"><span>Other cost</span><span class="input-affix"><span class="affix">${cur()}</span><input name="other" inputmode="decimal"></span><small>e.g. tractor hire, fuel</small></label></div>
    <div class="grid-2"><label class="field"><span>Input taken from stock <em class="muted">(${esc(t('common.optional'))})</em></span><select name="product_id"><option value="">None</option>${inputs.map((p) => `<option value="${p.id}">${esc(p.name)}: ${qty(p.stock_qty)} ${esc(p.unit)}</option>`).join('')}</select><small>Seed, fertiliser, herbicide… costed at average cost.</small></label>
      <label class="field"><span>Quantity used</span><input name="input_qty" inputmode="decimal" value="0"></label></div>` : '<p class="muted small">A manager can add costs and inputs used.</p>'}
    <p class="muted small">${esc(t('notice.no_chemicals'))}</p>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('add_crop_activity', { input: { cycle_id: c.id, activity_date: v.date, activity_type: v.type, description: v.description, area_done: Number(v.area_done) || 0,
          labour_kobo: toKobo(v.labour ?? ''), other_cost_kobo: toKobo(v.other ?? ''), product_id: v.product_id ? Number(v.product_id) : null, input_qty: v.product_id ? Number(v.input_qty) || 0 : 0 } });
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast(t('common.saved')); m.close(); done();
    });
  });
}

async function harvestForm(c: CropCycle, done: () => void) {
  const manager = auth.can('manager');
  await store.loadProducts();
  const produce = store.products.filter((p) => p.active && p.track_stock && p.kind !== 'service');
  const m = modal({ title: 'Record harvest', subtitle: `${c.crop} on ${c.plot_code}`, size: 'md', body: `<form class="stack">
    <div class="grid-3"><label class="field"><span>Date</span><input type="date" name="date" required value="${isoDate(new Date())}" min="${esc(c.planting_date)}"></label>
      <label class="field"><span>Quantity harvested</span><input name="quantity" inputmode="decimal" required autofocus></label>
      <label class="field"><span>Unit</span><input name="unit" required list="h-units" maxlength="30" value="${esc(c.harvests[0]?.unit ?? 'bag')}"><datalist id="h-units">${HARVEST_UNITS.map((u) => `<option value="${u}">`).join('')}</datalist></label></div>
    <div class="grid-3"><label class="field"><span>Damaged or lost</span><input name="damaged" inputmode="decimal" value="0"></label>
      <label class="field"><span>Put in store</span><input name="stored" inputmode="decimal" value="0"></label>
      <label class="field"><span>Sold at harvest</span><input name="sold" inputmode="decimal" value="0"></label></div>
    <div class="grid-2"><label class="field"><span>Add the stored quantity to stock as <em class="muted">(${esc(t('common.optional'))})</em></span><select name="product_id"><option value="">Do not add to stock</option>${produce.map((p) => `<option value="${p.id}" data-unit="${esc(p.unit)}">${esc(p.name)} (${esc(p.unit)})</option>`).join('')}</select><small>The stock item must use the same unit.</small></label>
      <label class="field"><span>Grade or quality</span><input name="grade" maxlength="40"></label></div>
    ${manager ? `<div class="grid-2"><label class="field"><span>Amount received for what was sold</span><span class="input-affix"><span class="affix">${cur()}</span><input name="amount" inputmode="decimal"></span></label>
      <label class="field"><span>Buyer</span><input name="buyer" maxlength="120"></label></div>
    <div class="grid-3"><label class="field"><span>Harvest labour</span><span class="input-affix"><span class="affix">${cur()}</span><input name="labour" inputmode="decimal"></span></label>
      <label class="field"><span>Transport</span><span class="input-affix"><span class="affix">${cur()}</span><input name="transport" inputmode="decimal"></span></label>
      <label class="field"><span>Other cost</span><span class="input-affix"><span class="affix">${cur()}</span><input name="other" inputmode="decimal"></span></label></div>` : ''}
    <label class="field"><span>Notes</span><input name="notes" maxlength="500"></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  (form.elements.namedItem('product_id') as HTMLSelectElement).addEventListener('change', (e) => {
    const unit = (e.target as HTMLSelectElement).selectedOptions[0]?.dataset.unit;
    if (unit) (form.elements.namedItem('unit') as HTMLInputElement).value = unit;
  });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const n = (k: string) => Number((v[k] ?? '').replace(/,/g, '')) || 0;
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('add_harvest', { input: { cycle_id: c.id, harvest_date: v.date, quantity: n('quantity'), unit: v.unit, grade: v.grade, damaged_qty: n('damaged'), product_id: v.product_id ? Number(v.product_id) : null,
          stored_qty: n('stored'), sold_qty: n('sold'), sale_amount_kobo: toKobo(v.amount ?? ''), buyer: v.buyer ?? '', labour_kobo: toKobo(v.labour ?? ''), transport_kobo: toKobo(v.transport ?? ''), other_cost_kobo: toKobo(v.other ?? ''), notes: v.notes } });
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Harvest saved'); m.close(); done();
    });
  });
}
