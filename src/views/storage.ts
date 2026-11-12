import { auth, call } from '../api';
import { exportCsv } from '../lib/csv';
import { $, confirmDialog, emptyState, errText, esc, modal, toast } from '../lib/dom';
import { isoDate, money, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { t } from '../lib/i18n';
import { fmtNum, formModal, num, pill, tableHtml } from '../lib/ui';
import { store } from '../store';
import type { Farm } from '../types';
import type { ViewCtx } from './shell';

interface Warehouse { id: number; farm_id: number | null; name: string; kind: string; notes: string; status: string; is_demo: boolean; items: number; value_kobo: number }
interface Located { product_id: number; product_name: string; unit: string; quantity: number; business_stock: number; unallocated: number; value_kobo: number }
interface Move { id: number; move_date: string; kind: string; product_name: string; unit: string; quantity: number; other_store: string; grade: string; moisture_pct: number | null; cost_kobo: number; note: string; user_name: string }

export const STORE_KINDS: Array<[string, string]> = [['store', 'Store'], ['warehouse', 'Warehouse'], ['cold_room', 'Cold room'], ['fish_pond', 'Fish pond holding'], ['palm_oil_storage', 'Palm oil storage'], ['grain_store', 'Grain store / crib'], ['processing_store', 'Processing store'], ['retail_shop', 'Retail shop'], ['other', 'Other']];
const ACTIONS: Array<[string, string, string]> = [
  ['put_away', 'Put away', 'Stock you already own goes into this store'],
  ['issue', 'Issue out', 'Taken out for use or sale (stock stays in the business)'],
  ['transfer', 'Transfer', 'Move to another store; the total does not change'],
  ['spoilage', 'Spoilage', 'Spoiled: removed from stock'],
  ['damage', 'Damage', 'Damaged: removed from stock'],
  ['loss', 'Loss / theft', 'Missing: removed from stock'],
  ['count', 'Count', 'Set what is physically in this store'],
  ['inspection', 'Quality check', 'Grade or moisture reading; stock does not change'],
];
const kindLabel = (k: string) => STORE_KINDS.find((x) => x[0] === k)?.[1] ?? k;
const moveLabel: Record<string, string> = { put_away: 'Put away', issue: 'Issued', transfer_out: 'Transferred out', transfer_in: 'Transferred in', spoilage: 'Spoilage', damage: 'Damage', loss: 'Loss', count: 'Count', inspection: 'Quality check' };

/** Storage locations: what sits where, transfers between stores, spoilage and quality checks. */
export async function render(ctx: ViewCtx) {
  const manager = auth.can('manager');
  ctx.header(t('nav.storage', {}, 'Storage'), 'Where your stock is kept, transfers, spoilage, moisture and quality checks.', manager ? `<button class="btn btn-primary" data-new>${icon('plus', 16)} New store</button>` : '');
  const list = await call<Warehouse[]>('list_warehouses', { includeArchived: true });
  const active = list.filter((w) => w.status === 'active');
  ctx.main.innerHTML = list.length ? `<div class="card-grid">${list.map((w) => `<button class="farm-tile ${w.status === 'archived' ? 'is-closed' : ''}" data-open="${w.id}">
      <span class="farm-tile-top"><b>${esc(w.name)}</b>${w.is_demo ? pill(t('common.demo'), 'warn') : ''}${w.status === 'archived' ? pill('Archived') : ''}</span>
      <span class="muted small">${esc(kindLabel(w.kind))} · ${w.items} item${w.items === 1 ? '' : 's'}${manager ? ` · ${money(w.value_kobo, { whole: true })} at cost` : ''}</span></button>`).join('')}</div>`
    : emptyState('No stores yet', 'Add the places you keep feed, produce and products: a store room, a cold room, a crib, a shop.', manager ? `<button class="btn btn-primary" data-new-inline>${icon('plus', 16)} New store</button>` : '');
  const reload = () => render(ctx);
  ctx.main.onclick = (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-new-inline]')) void storeForm(null, reload);
    const o = el.closest<HTMLElement>('[data-open]');
    if (o) void openStore(list.find((w) => w.id === Number(o.dataset.open))!, active, reload);
  };
  $('[data-new]')?.addEventListener('click', () => void storeForm(null, reload));
}

async function storeForm(w: Warehouse | null, done: () => void) {
  const farms = await call<Farm[]>('list_farms').catch(() => [] as Farm[]);
  formModal({ title: w ? `Edit ${w.name}` : 'New store', fields: [
    { name: 'name', label: 'Name', value: w?.name ?? '', required: true, maxlength: 80, pair: true },
    { name: 'kind', label: 'Type', type: 'select', options: STORE_KINDS, value: w?.kind ?? 'store' },
    { name: 'farm_id', label: 'Farm', type: 'select', options: [['', '—'], ...farms.map((f): [string, string] => [String(f.id), f.name])], value: w?.farm_id ? String(w.farm_id) : '' },
    { name: 'notes', label: 'Notes', type: 'textarea', value: w?.notes ?? '' },
  ], done: 'Store saved', submit: async (v) => {
    await call('save_warehouse', { input: { id: w?.id ?? null, name: v.name, kind: v.kind, farm_id: v.farm_id ? Number(v.farm_id) : null, notes: v.notes } });
    done();
  } });
}

async function openStore(w: Warehouse, stores: Warehouse[], done: () => void) {
  const manager = auth.can('manager');
  const m = modal({ title: w.name, subtitle: kindLabel(w.kind), size: 'lg', onClose: done, body: '<div data-store class="stack"></div>' });
  const box = $('[data-store]', m.el)!;
  const draw = async () => {
    const [stock, moves] = await Promise.all([call<Located[]>('location_stock', { warehouseId: w.id }), call<Move[]>('storage_history', { warehouseId: w.id })]);
    box.innerHTML = `${w.status === 'active' ? `<div class="row-actions">${ACTIONS.map(([k, l]) => `<button class="btn ${k === 'put_away' ? 'btn-primary' : 'btn-ghost'} btn-sm" data-act="${k}">${esc(l)}</button>`).join('')}</div>` : '<p class="muted">This store is archived.</p>'}
      <h3 class="section-title">In this store</h3>
      ${tableHtml(['Item', 'Here#', 'Not in any store#', 'Business total#', ...(manager ? ['Value at cost#'] : [])], stock.map((s) => [esc(s.product_name), `${qty(s.quantity)} ${esc(s.unit)}`, s.unallocated < 0 ? `<span class="warn-text" title="The stores hold more than the stock count. Issue out what was sold or used.">${qty(s.unallocated)}</span>` : qty(s.unallocated), qty(s.business_stock), ...(manager ? [money(s.value_kobo)] : [])]), 'Nothing here yet. Use Put away to place stock you own in this store.')}
      ${stock.some((s) => s.unallocated < 0) ? '<p class="warn-text small">Some items show more in stores than in stock. Sales and usage reduce stock but not a store: use Issue out to take sold or used stock out of the store.</p>' : ''}
      <h3 class="section-title">History</h3>
      ${tableHtml(['Date', 'Action', 'Item', 'Quantity#', 'Details', 'By'], moves.map((x) => [esc(x.move_date), esc(moveLabel[x.kind] ?? x.kind), esc(x.product_name), x.quantity ? `${qty(x.quantity)} ${esc(x.unit)}` : '', esc([x.other_store && (x.kind === 'transfer_out' ? `to ${x.other_store}` : `from ${x.other_store}`), x.grade && `grade ${x.grade}`, x.moisture_pct !== null ? `moisture ${fmtNum(x.moisture_pct)}%` : '', x.cost_kobo ? money(x.cost_kobo) : '', x.note].filter(Boolean).join(' · ')), esc(x.user_name)]), 'No storage history yet.')}
      <div class="form-actions">${manager ? `<button class="btn btn-ghost" data-edit>${icon('edit', 16)} Edit</button><button class="btn btn-ghost" data-archive>${w.status === 'active' ? 'Archive' : 'Restore'}</button>` : ''}
        <button class="btn btn-ghost" data-csv>${icon('download', 16)} Export CSV</button></div>`;
    box.onclick = async (e) => {
      const el = e.target as HTMLElement;
      const a = el.closest<HTMLElement>('[data-act]');
      if (a) await actionForm(w, a.dataset.act!, stock, stores, draw);
      if (el.closest('[data-edit]')) { m.close(); void storeForm(w, done); }
      if (el.closest('[data-archive]')) {
        const restore = w.status !== 'active';
        if (!restore && await confirmDialog({ title: `Archive ${w.name}?`, message: 'Move everything out first. History is kept.', confirm: 'Archive' }) === null) return;
        try { await call('archive_warehouse', { id: w.id, restore }); m.close(); } catch (x) { toast(errText(x), 'error'); }
      }
      if (el.closest('[data-csv]')) void exportCsv(`storage-${w.name}.csv`, ['Date', 'Action', 'Item', 'Quantity', 'Unit', 'Other store', 'Grade', 'Moisture %', 'Cost (NGN)', 'Note', 'By'], moves.map((x) => [x.move_date, moveLabel[x.kind] ?? x.kind, x.product_name, x.quantity, x.unit, x.other_store, x.grade, x.moisture_pct ?? '', x.cost_kobo / 100, x.note, x.user_name]));
    };
  };
  await draw().catch((x) => { box.innerHTML = `<p class="form-error">${esc(errText(x))}</p>`; });
}

async function actionForm(w: Warehouse, kind: string, here: Located[], stores: Warehouse[], done: () => Promise<void>) {
  if (!store.products.length) await store.loadProducts();
  const stocked = store.products.filter((p) => p.track_stock);
  const items: Array<[string, string]> = (kind === 'put_away' || kind === 'count' ? stocked.map((p): [string, string] => [String(p.id), `${p.name} (${qty(p.stock_qty)} ${p.unit} in business)`]) : here.map((s): [string, string] => [String(s.product_id), `${s.product_name} (${qty(s.quantity)} ${s.unit} here)`]));
  if (kind === 'inspection') items.push(...stocked.filter((p) => !here.some((h) => h.product_id === p.id)).map((p): [string, string] => [String(p.id), p.name]));
  if (!items.length) { toast(kind === 'put_away' ? 'No stocked items yet. Add items in Farm stock first.' : 'Nothing is in this store yet.', 'info'); return; }
  const meta = ACTIONS.find((a) => a[0] === kind)!;
  formModal({ title: `${meta[1]} · ${w.name}`, intro: esc(meta[2]), fields: [
    { name: 'product', label: 'Item', type: 'select', options: items, required: true },
    { name: 'date', label: 'Date', type: 'date', value: isoDate(new Date()), required: true, pair: kind !== 'inspection' },
    ...(kind !== 'inspection' ? [{ name: 'qty', label: kind === 'count' ? 'Quantity counted here' : 'Quantity', type: 'number' as const, required: true }] : []),
    ...(kind === 'transfer' ? [{ name: 'to', label: 'To store', type: 'select' as const, options: stores.filter((s) => s.id !== w.id).map((s): [string, string] => [String(s.id), s.name]), required: true }] : []),
    ...(kind === 'inspection' ? [{ name: 'grade', label: 'Grade or quality', pair: true }, { name: 'moisture', label: 'Moisture %', type: 'number' as const }] : []),
    ...(kind === 'transfer' || kind === 'issue' ? [{ name: 'cost', label: 'Cost (transport, handling)', type: 'money' as const, help: 'Optional. Recorded with the movement.' }] : []),
    { name: 'note', label: kind === 'spoilage' || kind === 'damage' || kind === 'loss' ? 'Reason' : 'Note', type: 'textarea', required: ['spoilage', 'damage', 'loss'].includes(kind) },
  ], done: 'Saved', submit: async (v) => {
    await call('storage_action', { input: { warehouse_id: w.id, product_id: Number(v.product), kind, date: v.date, quantity: num(v.qty) ?? 0, to_warehouse_id: v.to ? Number(v.to) : null, grade: v.grade ?? '', moisture_pct: num(v.moisture), cost_kobo: v.cost ? toKobo(v.cost) : 0, note: v.note } });
    await store.loadProducts();
    await done();
  } });
}
