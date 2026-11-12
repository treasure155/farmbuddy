import { auth, call } from '../api';
import { confirmDialog, errText, esc, modal, toast } from '../lib/dom';
import { dateLabel, isoDate, money, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { CROPS } from '../lib/content/farm';
import { AREA_UNITS } from '../lib/content/nigeria';
import { fmtNum, formModal, num, pill, reviewPill, tableHtml } from '../lib/ui';
import type { CropCycle, Farm, Plot } from '../types';

interface CalEvent { date: string; end_date: string; kind: string; title: string; cycle_id: number | null; source: string }
interface Health { id: number | null; cycle_id: number; crop: string; plot_code: string; observed_date: string; kind: string; observation: string; affected_area: number; action_taken: string; product_applied: string; quantity_applied: string; cost_kobo: number; advice_source: string; review_status: string; reviewer: string; notes: string; user_name: string }
interface Variety { id: number | null; crop: string; variety: string; maturity_days: number | null; notes: string }
interface Field { id: number | null; farm_id: number | null; name: string; area: number; area_unit: string; notes: string; plots: number }

const KIND_TONE: Record<string, 'ok' | 'warn' | 'info' | 'mute' | 'bad'> = { planting: 'ok', harvest_window: 'warn', harvest: 'ok', activity: 'mute', crop_health: 'bad', reminder: 'info', rain_onset: 'info', rain_end: 'info' };
const KIND_LABEL: Record<string, string> = { planting: 'Planted', harvest_window: 'Harvest window', harvest: 'Harvest', activity: 'Field work', crop_health: 'Crop health', reminder: 'Reminder', rain_onset: 'Rains expected', rain_end: 'Rains end' };
let month = isoDate(new Date()).slice(0, 7);

/** A month view of planting, field work, harvests, crop health, reminders and the season outlook. */
export async function calendar(pane: HTMLElement) {
  const [y, mo] = month.split('-').map(Number);
  const first = new Date(y, mo - 1, 1);
  const last = new Date(y, mo, 0);
  const events = await call<CalEvent[]>('crop_calendar', { from: isoDate(first), to: isoDate(last) });
  const byDay = new Map<number, CalEvent[]>();
  for (const e of events) {
    const start = new Date(`${e.date}T00:00`);
    const end = e.end_date ? new Date(`${e.end_date}T00:00`) : start;
    for (let d = new Date(Math.max(start.getTime(), first.getTime())); d <= end && d <= last; d.setDate(d.getDate() + 1)) {
      const k = d.getDate();
      byDay.set(k, [...(byDay.get(k) ?? []), e]);
    }
  }
  const lead = (first.getDay() + 6) % 7;
  const cells: string[] = [];
  for (let i = 0; i < lead; i++) cells.push('<div class="cal-cell is-empty"></div>');
  const todayStr = isoDate(new Date());
  for (let d = 1; d <= last.getDate(); d++) {
    const iso = `${month}-${String(d).padStart(2, '0')}`;
    const evs = byDay.get(d) ?? [];
    cells.push(`<div class="cal-cell ${iso === todayStr ? 'is-today' : ''}"><span class="cal-day">${d}</span>${evs.slice(0, 3).map((e) => `<span class="cal-ev tone-${KIND_TONE[e.kind] ?? 'mute'}" title="${esc(e.title)}">${esc(e.title)}</span>`).join('')}${evs.length > 3 ? `<small class="muted">+${evs.length - 3} more</small>` : ''}</div>`);
  }
  pane.innerHTML = `<div class="toolbar"><button class="btn btn-ghost btn-sm" data-cal="-1" aria-label="Previous month">‹</button><b>${esc(first.toLocaleDateString('en-NG', { month: 'long', year: 'numeric' }))}</b><button class="btn btn-ghost btn-sm" data-cal="1" aria-label="Next month">›</button><button class="btn btn-ghost btn-sm" data-cal="0">This month</button></div>
    <div class="cal-grid" role="grid">${['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map((d) => `<div class="cal-head">${d}</div>`).join('')}${cells.join('')}</div>
    <h3 class="section-title">This month</h3>
    ${tableHtml(['Date', 'What', 'Details', 'Source'], events.map((e) => [esc(dateLabel(e.date)) + (e.end_date && e.end_date !== e.date ? ` – ${esc(dateLabel(e.end_date))}` : ''), pill(KIND_LABEL[e.kind] ?? e.kind, KIND_TONE[e.kind] ?? 'mute'), esc(e.title), esc(e.source)]), 'Nothing on the calendar this month.')}`;
  pane.onclick = (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-cal]');
    if (!b) return;
    const step = Number(b.dataset.cal);
    if (step === 0) month = isoDate(new Date()).slice(0, 7);
    else { const d = new Date(y, mo - 1 + step, 1); month = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`; }
    void calendar(pane);
  };
}

/** Pests, diseases, weeds and nutrient problems seen, what was done and who advised. */
export async function cropHealth(pane: HTMLElement) {
  const [list, cycles] = await Promise.all([call<Health[]>('list_crop_health', { cycleId: null }), call<CropCycle[]>('list_crop_cycles', { includeClosed: false })]);
  pane.innerHTML = `<div class="callout callout-info"><div><b>Record what you see; confirm before treating</b><span>FarmBuddy does not identify pests or diseases and gives no chemical rates. Show the plants to an extension officer or agronomist, and use registered products only as the label says.</span></div></div>
    ${cycles.length ? `<div><button class="btn btn-primary" data-new-h>${icon('plus', 16)} Record a problem</button></div>` : '<p class="muted">Plant a crop first.</p>'}
    ${tableHtml(['Date', 'Crop', 'Problem', 'What you saw', 'Action and product', 'Advised by', 'Status', ''], list.map((h) => [esc(dateLabel(h.observed_date)), `${esc(h.crop)} <small class="muted">${esc(h.plot_code)}</small>`, esc(h.kind), esc(h.observation) + (h.affected_area ? ` <small class="muted">${fmtNum(h.affected_area)} affected</small>` : ''),
      esc([h.action_taken, h.product_applied && `${h.product_applied}${h.quantity_applied ? ` (${h.quantity_applied})` : ''}`, h.cost_kobo && auth.can('manager') ? money(h.cost_kobo) : ''].filter(Boolean).join(' · ')), esc(h.advice_source),
      `${reviewPill(h.review_status)}${h.reviewer ? ` <small class="muted">${esc(h.reviewer)}</small>` : ''}`, `<button class="icon-btn" data-edit-h="${h.id}" aria-label="Edit">${icon('edit', 15)}</button>${auth.can('manager') ? `<button class="icon-btn" data-del-h="${h.id}" aria-label="Delete">${icon('trash', 15)}</button>` : ''}`]), 'No crop problems recorded.')}`;
  const form = (h: Health | null) => formModal({ title: h ? 'Edit crop health record' : 'Record a crop problem', fields: [
    { name: 'cycle', label: 'Crop', type: 'select', options: cycles.map((c): [string, string] => [String(c.id), `${c.crop} · ${c.plot_code} · planted ${c.planting_date}`]), value: h ? String(h.cycle_id) : '', required: true, pair: true },
    { name: 'date', label: 'Date seen', type: 'date', value: h?.observed_date ?? isoDate(new Date()), required: true },
    { name: 'kind', label: 'Type', type: 'select', options: [['pest', 'Pest'], ['disease', 'Disease'], ['weed', 'Weeds'], ['nutrient', 'Nutrient (yellowing, poor growth)'], ['other', 'Other']], value: h?.kind ?? 'pest', pair: true },
    { name: 'area', label: 'Area affected (same unit as the plot)', type: 'number', value: h?.affected_area ?? '' },
    { name: 'obs', label: 'What you saw', type: 'textarea', value: h?.observation ?? '', required: true, placeholder: 'e.g. holes in leaves, white insects under leaves, yellow patches' },
    { name: 'action', label: 'What was done', type: 'textarea', value: h?.action_taken ?? '' },
    { name: 'product', label: 'Product applied (if any)', value: h?.product_applied ?? '', pair: true },
    { name: 'qty', label: 'Quantity applied (as on label)', value: h?.quantity_applied ?? '' },
    { name: 'advice', label: 'Who advised (required if a product was used)', value: h?.advice_source ?? '', placeholder: 'e.g. ADP extension officer, product label' },
    ...(auth.can('manager') ? [{ name: 'cost', label: 'Cost', type: 'money' as const, value: h?.cost_kobo ?? '' }] : []),
    ...(auth.canAdvise() ? [{ name: 'confirmed', label: 'Problem confirmed by an extension officer or agronomist', type: 'checkbox' as const, value: h?.review_status === 'confirmed' }, { name: 'reviewer', label: 'Confirmed by', value: h?.reviewer ?? '' }] : []),
    { name: 'notes', label: 'Notes', type: 'textarea', value: h?.notes ?? '' },
  ], done: 'Saved', submit: async (v) => {
    await call('save_crop_health', { input: { id: h?.id ?? null, cycle_id: Number(v.cycle), observed_date: v.date, kind: v.kind, observation: v.obs, affected_area: num(v.area) ?? 0, action_taken: v.action, product_applied: v.product, quantity_applied: v.qty, cost_kobo: v.cost ? toKobo(v.cost) : 0, advice_source: v.advice, review_status: v.confirmed ? 'confirmed' : 'unconfirmed', reviewer: v.reviewer ?? '', notes: v.notes } });
    void cropHealth(pane);
  } });
  pane.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-new-h]')) form(null);
    const ed = el.closest<HTMLElement>('[data-edit-h]');
    if (ed) form(list.find((h) => h.id === Number(ed.dataset.editH)) ?? null);
    const d = el.closest<HTMLElement>('[data-del-h]');
    if (d && await confirmDialog({ title: 'Delete this record?', message: 'It is removed from the crop history.', confirm: 'Delete', danger: true }) !== null) { try { await call('delete_crop_health', { id: Number(d.dataset.delH) }); void cropHealth(pane); } catch (x) { toast(errText(x), 'error'); } }
  };
}

/** Varieties you grow, with the maturity period from the seed supplier. */
export async function varieties(pane: HTMLElement) {
  const list = await call<Variety[]>('list_varieties');
  const manager = auth.can('manager');
  pane.innerHTML = `<p class="muted small">Keep the varieties you plant with their maturity period from the seed label or supplier. They are offered when you plant a crop.</p>
    ${manager ? `<div><button class="btn btn-primary" data-new-v>${icon('plus', 16)} Add variety</button></div>` : ''}
    ${tableHtml(['Crop', 'Variety', 'Maturity (days)#', 'Notes', ''], list.map((v) => [esc(v.crop), esc(v.variety), v.maturity_days === null ? '—' : String(v.maturity_days), esc(v.notes), manager ? `<button class="icon-btn" data-edit-v="${v.id}" aria-label="Edit">${icon('edit', 15)}</button><button class="icon-btn" data-del-v="${v.id}" aria-label="Delete">${icon('trash', 15)}</button>` : '']), 'No varieties yet.')}`;
  const form = (v: Variety | null) => formModal({ title: v ? 'Edit variety' : 'Add variety', fields: [
    { name: 'crop', label: 'Crop', value: v?.crop ?? '', required: true, list: CROPS, pair: true },
    { name: 'variety', label: 'Variety name', value: v?.variety ?? '', required: true },
    { name: 'days', label: 'Days to maturity (from the seed label or supplier)', type: 'number', value: v?.maturity_days ?? '' },
    { name: 'notes', label: 'Notes (source, traits)', type: 'textarea', value: v?.notes ?? '' },
  ], done: 'Variety saved', submit: async (x) => {
    await call('save_variety', { input: { id: v?.id ?? null, crop: x.crop, variety: x.variety, maturity_days: num(x.days), notes: x.notes } });
    void varieties(pane);
  } });
  pane.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-new-v]')) form(null);
    const ed = el.closest<HTMLElement>('[data-edit-v]');
    if (ed) form(list.find((v) => v.id === Number(ed.dataset.editV)) ?? null);
    const d = el.closest<HTMLElement>('[data-del-v]');
    if (d && await confirmDialog({ title: 'Delete this variety?', message: 'Crops already planted keep their variety name.', confirm: 'Delete', danger: true }) !== null) { try { await call('delete_variety', { id: Number(d.dataset.delV) }); void varieties(pane); } catch (x) { toast(errText(x), 'error'); } }
  };
}

/** Fields group plots (field → plots), and each plot keeps its crop history. */
export async function fields(pane: HTMLElement) {
  const [list, plots, farms] = await Promise.all([call<Field[]>('list_fields'), call<Plot[]>('list_plots', { includeArchived: false }), call<Farm[]>('list_farms').catch(() => [] as Farm[])]);
  const manager = auth.can('manager');
  const plotField = (p: Plot) => p.field_id;
  pane.innerHTML = `<p class="muted small">A field is a larger piece of land made of plots. Put each plot in its field, and open a plot to see every crop grown on it.</p>
    ${manager ? `<div><button class="btn btn-primary" data-new-f>${icon('plus', 16)} Add field</button></div>` : ''}
    ${tableHtml(['Field', 'Area', 'Plots#', 'Notes', ''], list.map((f) => [`<b>${esc(f.name)}</b>`, f.area ? `${fmtNum(f.area)} ${esc(f.area_unit.replace('_', ' '))}` : '—', String(f.plots), esc(f.notes), manager ? `<button class="icon-btn" data-edit-f="${f.id}" aria-label="Edit">${icon('edit', 15)}</button>` : '']), 'No fields yet.')}
    <h3 class="section-title">Plots</h3>
    ${tableHtml(['Plot', 'Field', 'Growing now', 'Crops so far#', ''], plots.map((p) => [`<b>${esc(p.code)}</b> ${esc(p.name)}`,
      manager ? `<select data-plot-field="${p.id}" aria-label="Field for ${esc(p.code)}"><option value="">—</option>${list.map((f) => `<option value="${f.id}" ${plotField(p) === f.id ? 'selected' : ''}>${esc(f.name)}</option>`).join('')}</select>` : esc(list.find((f) => f.id === plotField(p))?.name ?? '—'),
      esc(p.current_crop || '—'), String(p.cycles), `<button class="btn btn-ghost btn-sm" data-history="${p.id}">History</button>`]), 'No plots yet.')}`;
  const form = (f: Field | null) => formModal({ title: f ? `Edit ${f.name}` : 'Add field', fields: [
    { name: 'name', label: 'Name', value: f?.name ?? '', required: true },
    { name: 'area', label: 'Area', type: 'number', value: f?.area ?? '', pair: true },
    { name: 'unit', label: 'Unit', type: 'select', options: AREA_UNITS.map((u): [string, string] => [u, u.replace('_', ' ')]), value: f?.area_unit ?? 'hectare' },
    { name: 'farm', label: 'Farm', type: 'select', options: [['', '—'], ...farms.map((x): [string, string] => [String(x.id), x.name])], value: f?.farm_id ? String(f.farm_id) : '' },
    { name: 'notes', label: 'Notes (soil, slope, water)', type: 'textarea', value: f?.notes ?? '' },
  ], done: 'Field saved', submit: async (v) => {
    await call('save_field', { input: { id: f?.id ?? null, farm_id: v.farm ? Number(v.farm) : null, name: v.name, area: num(v.area) ?? 0, area_unit: v.unit, notes: v.notes } });
    void fields(pane);
  } });
  pane.onchange = async (e) => {
    const s = (e.target as HTMLElement).closest<HTMLSelectElement>('[data-plot-field]');
    if (!s) return;
    try { await call('set_plot_field', { plotId: Number(s.dataset.plotField), fieldId: s.value ? Number(s.value) : null }); toast('Plot moved'); } catch (x) { toast(errText(x), 'error'); }
  };
  pane.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-new-f]')) form(null);
    const ed = el.closest<HTMLElement>('[data-edit-f]');
    if (ed) form(list.find((f) => f.id === Number(ed.dataset.editF)) ?? null);
    const h = el.closest<HTMLElement>('[data-history]');
    if (h) {
      const p = plots.find((x) => x.id === Number(h.dataset.history))!;
      const hist = await call<CropCycle[]>('plot_history', { plotId: p.id });
      modal({ title: `History of ${p.code}`, subtitle: p.name, size: 'lg', body: tableHtml(['Crop', 'Variety', 'Planted', 'Status', 'Harvested', 'Yield per area#', 'Loss#', ...(auth.can('manager') ? ['Margin#'] : [])],
        hist.map((c) => [esc(c.crop), esc(c.variety), esc(dateLabel(c.planting_date)), esc(c.status === 'growing' ? `growing, day ${c.days_since_planting}` : `closed ${dateLabel(c.closed_date)}`), esc(c.harvests.map((x) => `${qty(x.quantity)} ${x.unit}`).join(', ') || '—'), fmtNum(c.yield_per_area), `${fmtNum(c.loss_pct)}%`, ...(auth.can('manager') ? [money(c.margin_kobo)] : [])]), 'Nothing grown on this plot yet.') });
    }
  };
}
