import { t } from '../lib/i18n';
import { call } from '../api';
import { exportCsv, nairaPlain } from '../lib/csv';
import { $, $$, confirmDialog, debounce, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { dateLabel, isoDate, money, nairaInput, qty, relativeDay, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { store } from '../store';
import type { Batch, Kind, Movement, Product, RecipeLine } from '../types';
import { barcodeSvg } from '../lib/barcode';
import { printHtml } from '../lib/receipt';
import type { ViewCtx } from './shell';

const KIND_LABEL: Record<Kind, string> = { product: 'For sale', service: 'Service', material: 'Internal use' };
const MOVE_LABEL: Record<string, string> = { opening: 'Opening stock', stock_in: 'Received', stock_out: 'Removed', adjustment: 'Count correction', sale: 'Sold', void_reversal: 'Returned (void)', return: 'Returned by customer', purchase: 'Purchase', recipe: 'Used in recipe', flock_issue: 'Issued to livestock', crop_issue: 'Used on a crop', harvest: 'Harvest', processing_issue: 'Used in processing', processing_output: 'From processing' };

type Tab = 'items' | 'expiry' | 'categories' | 'history';
let tab: Tab = 'items';
const f = { search: '', category: '', kind: '', low: false, archived: false };

export async function render(ctx: ViewCtx) {
  if (ctx.params.low) { f.low = true; tab = 'items'; }
  if (ctx.params.tab) tab = ctx.params.tab as Tab;
  ctx.header(t('page.inventory.title', {}, 'Inventory'), `${store.label('items')}, services and the stock you keep.`, `
    <button class="btn btn-ghost" data-export>${icon('download', 16)} Export</button>
    <button class="btn btn-ghost" data-labels>${icon('label', 16)} Print labels</button>
    <button class="btn btn-primary" data-new>${icon('plus', 16)} Add ${esc(store.label('item').toLowerCase())}</button>`);
  ctx.main.innerHTML = `
    <div class="tabs" role="tablist">
      ${(['items', 'expiry', 'categories', 'history'] as Tab[]).map((t) => `<button role="tab" class="tab ${tab === t ? 'is-on' : ''}" aria-selected="${tab === t}" data-tab="${t}">${{ items: store.label('items'), expiry: 'Expiry dates', categories: 'Categories', history: 'Stock history' }[t]}</button>`).join('')}
    </div>
    <div data-pane></div>`;
  const pane = $('[data-pane]', ctx.main)!;

  const show = async () => {
    $$('[data-tab]', ctx.main).forEach((b) => { b.classList.toggle('is-on', b.dataset.tab === tab); b.setAttribute('aria-selected', String(b.dataset.tab === tab)); });
    if (tab === 'items') await itemsPane(pane);
    if (tab === 'expiry') await expiryPane(pane);
    if (tab === 'categories') await categoriesPane(pane);
    if (tab === 'history') await historyPane(pane);
  };
  ctx.main.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-tab]');
    if (b) { tab = b.dataset.tab as Tab; show().catch((x) => toast(errText(x), 'error')); }
  });
  $('[data-new]')!.addEventListener('click', () => itemForm(null, () => show()));
  $('[data-labels]')!.addEventListener('click', async () => { await store.loadProducts(); labelsDialog(); });
  $('[data-export]')!.addEventListener('click', async () => {
    await store.loadProducts(true);
    exportCsv(`inventory-${isoDate(new Date())}.csv`, ['Code', 'Barcode', 'Name', 'Category', 'Type', 'Unit', 'Selling price', 'Cost price', 'Tracks stock', 'In stock', 'Reorder level', 'Stock value (cost)', 'Active'],
      store.products.map((p) => [p.sku, p.barcode, p.name, p.category_name, KIND_LABEL[p.kind], p.unit, nairaPlain(p.selling_price_kobo), nairaPlain(p.cost_price_kobo), p.track_stock ? 'Yes' : 'No', p.stock_qty, p.reorder_level, nairaPlain(Math.round(p.stock_qty * p.cost_price_kobo)), p.active ? 'Yes' : 'No']));
  });
  await Promise.all([store.loadCategories(), store.loadTemplates()]);
  await show();
}

// ---------------------------------------------------------------- items

async function itemsPane(pane: HTMLElement) {
  await store.loadProducts(true);
  pane.innerHTML = `
    <div class="figures" data-figures></div>
    <div class="toolbar">
      <div class="search-field grow">${icon('search', 16)}<input type="search" data-f="search" value="${esc(f.search)}" placeholder="Name, code or barcode" aria-label="Search inventory"></div>
      <select data-f="category" aria-label="Category"><option value="">All categories</option>${store.categories.map((c) => `<option value="${c.id}" ${f.category === String(c.id) ? 'selected' : ''}>${esc(c.name)}</option>`).join('')}<option value="none" ${f.category === 'none' ? 'selected' : ''}>Uncategorised</option></select>
      <select data-f="kind" aria-label="Type"><option value="">All types</option>${(Object.keys(KIND_LABEL) as Kind[]).map((k) => `<option value="${k}" ${f.kind === k ? 'selected' : ''}>${KIND_LABEL[k]}</option>`).join('')}</select>
      <label class="check inline"><input type="checkbox" data-f="low" ${f.low ? 'checked' : ''}><span>Running low</span></label>
      <label class="check inline"><input type="checkbox" data-f="archived" ${f.archived ? 'checked' : ''}><span>Show archived</span></label>
    </div>
    <section class="panel flush" data-list></section>`;
  const list = $('[data-list]', pane)!;

  const draw = () => {
    const q = f.search.trim().toLowerCase();
    const rows = store.products.filter((p) =>
      (f.archived || p.active) &&
      (!q || p.name.toLowerCase().includes(q) || p.sku.toLowerCase().includes(q) || p.barcode.toLowerCase().includes(q)) &&
      (!f.category || (f.category === 'none' ? !p.category_id : String(p.category_id) === f.category)) &&
      (!f.kind || p.kind === f.kind) &&
      (!f.low || (p.track_stock && p.stock_qty <= p.reorder_level)));
    const active = store.products.filter((p) => p.active);
    const tracked = active.filter((p) => p.track_stock);
    const atCost = tracked.reduce((a, p) => a + Math.round(Math.max(0, p.stock_qty) * p.cost_price_kobo), 0);
    const atPrice = tracked.filter((p) => p.kind === 'product').reduce((a, p) => a + Math.round(Math.max(0, p.stock_qty) * p.selling_price_kobo), 0);
    const low = tracked.filter((p) => p.stock_qty <= p.reorder_level).length;
    $('[data-figures]', pane)!.innerHTML = `
      <div><span>${esc(store.label('items'))} and services</span><b>${active.length}</b></div>
      <div><span>Stock value at cost</span><b>${money(atCost)}</b></div>
      <div><span>Stock value at selling price</span><b>${money(atPrice)}</b></div>
      <div><span>Running low</span><b class="${low ? 'warn-text' : ''}">${low}</b></div>`;
    list.innerHTML = rows.length ? `<div class="table-scroll"><table class="table">
      <thead><tr><th>${esc(store.label('item'))}</th><th>Category</th><th>Type</th><th class="num">Selling price</th><th class="num">Cost</th><th class="num">In stock</th><th class="actions-col"><span class="sr-only">Actions</span></th></tr></thead>
      <tbody>${rows.map((p) => {
        const low = p.track_stock && p.stock_qty <= p.reorder_level;
        return `<tr class="${p.active ? '' : 'is-archived'}">
          <td><b>${esc(p.name)}</b>${p.has_recipe ? ' <span class="pill pill-info">Recipe</span>' : ''}${p.track_expiry ? ' <span class="pill pill-mute">Expiry</span>' : ''}<small class="muted block">${esc(p.sku)}${p.barcode ? ` · ${esc(p.barcode)}` : ''}${p.active ? '' : ' · Archived'}</small></td>
          <td>${esc(p.category_name || '—')}</td>
          <td><span class="kind kind-${p.kind}">${KIND_LABEL[p.kind]}</span></td>
          <td class="num">${p.kind === 'material' ? '<span class="muted">—</span>' : money(p.selling_price_kobo)}<small class="muted block">per ${esc(p.unit)}</small>${p.pack_unit && p.pack_size > 1 && p.kind !== 'material' ? `<small class="muted block">${esc(p.pack_unit)} of ${qty(p.pack_size)}: ${money(p.pack_price_kobo || Math.round(p.selling_price_kobo * p.pack_size))}</small>` : ''}${p.wholesale_price_kobo ? `<small class="muted block">${qty(p.wholesale_min_qty)}+ at ${money(p.wholesale_price_kobo)}</small>` : ''}</td>
          <td class="num">${money(p.has_recipe ? p.recipe_cost_kobo : p.cost_price_kobo)}${p.has_recipe ? '<small class="muted block">from recipe</small>' : ''}</td>
          <td class="num">${p.track_stock ? `<b class="${p.stock_qty <= 0 ? 'bad-text' : low ? 'warn-text' : ''}">${qty(p.stock_qty)}</b> <small class="muted">${esc(p.unit)}</small>${low ? `<small class="block warn-text">Reorder at ${qty(p.reorder_level)}</small>` : ''}` : '<span class="muted">Not tracked</span>'}</td>
          <td class="actions-col">
            ${p.kind !== 'material' && p.active && (store.flag('feature_recipes') || p.has_recipe) ? `<button class="btn btn-ghost btn-sm" data-recipe="${p.id}">Recipe</button>` : ''}
            ${p.track_stock && p.active ? `<button class="btn btn-ghost btn-sm" data-stock="${p.id}">${icon('stock', 15)} Stock</button>` : ''}
            <button class="icon-btn" data-edit="${p.id}" aria-label="Edit ${esc(p.name)}">${icon('edit', 16)}</button>
          </td></tr>`;
      }).join('')}</tbody></table></div>`
      : store.products.length ? emptyState('Nothing matches', 'Clear the filters or try another search.')
      : emptyState(`No ${store.label('items').toLowerCase()} yet`, 'Add what you sell and the stock you keep. You can also load sample items for your business type in Settings.', `<button class="btn btn-primary" data-new-inline>${icon('plus', 16)} Add ${esc(store.label('item').toLowerCase())}</button>`);
  };
  draw();

  const redraw = debounce(draw, 120);
  pane.oninput = (e) => {
    const el = (e.target as HTMLElement).closest<HTMLInputElement>('[data-f]');
    if (!el) return;
    const k = el.dataset.f as keyof typeof f;
    (f as Record<string, unknown>)[k] = el.type === 'checkbox' ? el.checked : el.value;
    redraw();
  };
  pane.onclick = (e) => {
    const t = e.target as HTMLElement;
    const reload = async () => { await store.loadProducts(true); draw(); };
    const edit = t.closest<HTMLElement>('[data-edit]');
    if (edit) itemForm(store.products.find((p) => p.id === Number(edit.dataset.edit))!, reload);
    const st = t.closest<HTMLElement>('[data-stock]');
    if (st) stockForm(store.products.find((p) => p.id === Number(st.dataset.stock))!, reload);
    const rc = t.closest<HTMLElement>('[data-recipe]');
    if (rc) recipeForm(store.products.find((p) => p.id === Number(rc.dataset.recipe))!, reload);
    if (t.closest('[data-new-inline]')) itemForm(null, reload);
  };
}

function nextSku(): string {
  const prefix = (store.settings.receipt_prefix || 'ITM').slice(0, 3).toUpperCase();
  const nums = store.products.map((p) => p.sku.match(new RegExp(`^${prefix}-(\\d+)$`, 'i'))?.[1]).filter(Boolean).map(Number);
  return `${prefix}-${String((nums.length ? Math.max(...nums) : 0) + 1).padStart(3, '0')}`;
}

/** Add/edit form. `prefill.barcode` is used when an unknown barcode is scanned at the till. */
export function itemForm(p: Product | null, done: (id?: number) => void | Promise<void>, prefill: { barcode?: string } = {}) {
  const isNew = !p;
  const item: Product = p ?? { id: 0, sku: nextSku(), barcode: prefill.barcode ?? '', name: '', category_id: null, category_name: '', kind: 'product' as Kind, unit: 'piece', selling_price_kobo: 0, cost_price_kobo: 0, track_stock: true, stock_qty: 0, reorder_level: 0, description: '', active: true,
    pack_unit: '', pack_size: 0, pack_price_kobo: 0, wholesale_price_kobo: 0, wholesale_min_qty: 0, track_expiry: false, has_recipe: false, recipe_cost_kobo: 0 };
  const cur = esc(store.settings.currency_symbol || '₦');
  const m = modal({ title: isNew ? `Add ${store.label('item').toLowerCase()}` : `Edit ${item.name}`, size: 'lg', body: `<form class="stack" novalidate>
    <div class="field"><span>What is it?</span>
      <div class="seg seg-wide" role="radiogroup" aria-label="Type">
        ${(Object.keys(KIND_LABEL) as Kind[]).map((k) => `<label><input type="radio" name="kind" value="${k}" ${item.kind === k ? 'checked' : ''}><span><b>${KIND_LABEL[k]}</b><small>${{ product: 'Goods, meals, birds or produce you sell', service: 'Work you charge for, no stock', material: 'Stock you use but do not sell' }[k]}</small></span></label>`).join('')}
      </div>
    </div>
    <div class="grid-3">
      <label class="field span-2"><span>Name</span><input name="name" required maxlength="120" value="${esc(item.name)}" autofocus></label>
      <label class="field"><span>Code (SKU)</span><input name="sku" required maxlength="40" value="${esc(item.sku)}" spellcheck="false"></label>
    </div>
    <div class="grid-3">
      <label class="field"><span>Category</span><select name="category_id"><option value="">No category</option>${store.categories.map((c) => `<option value="${c.id}" ${item.category_id === c.id ? 'selected' : ''}>${esc(c.name)}</option>`).join('')}</select></label>
      <label class="field"><span>Sold or counted per</span><input name="unit" required maxlength="24" list="unit-list" value="${esc(item.unit)}"><datalist id="unit-list">${store.units().map((u) => `<option value="${esc(u)}">`).join('')}</datalist></label>
      <label class="field"><span>Barcode <em class="muted">(optional)</em></span><input name="barcode" maxlength="64" value="${esc(item.barcode)}" placeholder="Scan into this box" spellcheck="false"></label>
    </div>
    <div class="grid-3">
      <label class="field" data-sell><span>Selling price</span><span class="input-affix"><span class="affix">${cur}</span><input name="selling_price" inputmode="decimal" value="${nairaInput(item.selling_price_kobo)}"></span></label>
      <label class="field"><span>Cost price</span><span class="input-affix"><span class="affix">${cur}</span><input name="cost_price" inputmode="decimal" value="${nairaInput(item.cost_price_kobo)}"></span><small>What you pay or spend per unit. Used for profit.</small></label>
      <div class="field" data-margin><span>Margin</span><output class="margin-out"></output></div>
    </div>
    <details class="subtle-details" data-packbox ${item.pack_unit || item.wholesale_price_kobo ? 'open' : ''}>
      <summary>Bags, crates and wholesale prices <em class="muted">(optional)</em></summary>
      <div class="grid-3">
        <label class="field"><span>Pack name</span><input name="pack_unit" maxlength="24" value="${esc(item.pack_unit)}" placeholder="e.g. bag, crate, basin"></label>
        <label class="field"><span>How many ${esc(item.unit || 'units')} in one pack</span><input name="pack_size" inputmode="decimal" value="${item.pack_size ? qty(item.pack_size) : ''}" placeholder="e.g. 40"></label>
        <label class="field"><span>Pack price</span><span class="input-affix"><span class="affix">${cur}</span><input name="pack_price" inputmode="decimal" value="${nairaInput(item.pack_price_kobo)}" placeholder="Unit price × pack size"></span></label>
      </div>
      <div class="grid-3">
        <label class="field"><span>Wholesale price per ${esc(item.unit || 'unit')}</span><span class="input-affix"><span class="affix">${cur}</span><input name="wholesale_price" inputmode="decimal" value="${nairaInput(item.wholesale_price_kobo)}"></span></label>
        <label class="field"><span>From quantity</span><input name="wholesale_min" inputmode="decimal" value="${item.wholesale_min_qty ? qty(item.wholesale_min_qty) : ''}" placeholder="e.g. 12"></label>
        <p class="muted small field-note">Sell produce sells single units or whole packs, and switches to the wholesale price when a buyer takes that many units or more.</p>
      </div>
    </details>
    <fieldset class="subtle stack" data-stockbox>
      <label class="check"><input type="checkbox" name="track_stock" ${item.track_stock ? 'checked' : ''}><span><b>Track stock for this ${esc(store.label('item').toLowerCase())}</b><br><small class="muted">Sales reduce the count. Turn off for things you never run out of.</small></span></label>
      <div class="grid-3" data-stockfields>
        ${isNew ? `<label class="field"><span>Opening stock</span><input name="stock_qty" inputmode="decimal" value="0"></label>` : `<div class="field"><span>In stock now</span><output>${qty(item.stock_qty)} ${esc(item.unit)}</output><small>Change it with the Stock button.</small></div>`}
        <label class="field"><span>Reorder level</span><input name="reorder_level" inputmode="decimal" value="${qty(item.reorder_level)}"><small>Warn me when stock falls to this.</small></label>
      </div>
      <label class="check" data-expirybox><input type="checkbox" name="track_expiry" ${item.track_expiry ? 'checked' : ''}><span><b>Track expiry dates</b><br><small class="muted">Record batch and expiry date when stock arrives. The oldest batch is sold first, and you are warned before it expires.</small></span></label>
    </fieldset>
    <label class="field"><span>Description <em class="muted">(optional)</em></span><textarea name="description" rows="2" maxlength="500">${esc(item.description)}</textarea></label>
    ${!isNew ? `<label class="check"><input type="checkbox" name="active" ${item.active ? 'checked' : ''}><span>Active <small class="muted">— archived items are hidden from the till but kept on old receipts</small></span></label>` : ''}
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions">
      ${!isNew ? `<button type="button" class="btn btn-ghost btn-danger-text push-left" data-remove>${icon('trash', 16)} Delete</button>` : ''}
      <button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button>
      ${isNew ? '<button type="submit" class="btn btn-ghost" data-again>Save and add another</button>' : ''}
      <button type="submit" class="btn btn-primary">${esc(t('common.save'))}</button>
    </div></form>` });
  const form = m.el.querySelector('form')!;
  const sync = () => {
    const kind = (form.querySelector('input[name=kind]:checked') as HTMLInputElement).value as Kind;
    $('[data-sell]', form)!.hidden = kind === 'material';
    $('[data-margin]', form)!.hidden = kind === 'material';
    $('[data-stockbox]', form)!.hidden = kind === 'service';
    const track = (form.elements.namedItem('track_stock') as HTMLInputElement).checked;
    $('[data-stockfields]', form)!.hidden = !track;
    $('[data-expirybox]', form)!.hidden = !track;
    $('[data-packbox]', form)!.hidden = kind === 'service';
    const sp = toKobo((form.elements.namedItem('selling_price') as HTMLInputElement).value);
    const cp = toKobo((form.elements.namedItem('cost_price') as HTMLInputElement).value);
    $('.margin-out', form)!.textContent = sp && cp ? `${money(sp - cp)} (${Math.round(((sp - cp) / sp) * 100)}%)` : '—';
  };
  sync();
  form.addEventListener('input', sync);
  let again = false;
  $('[data-again]', form)?.addEventListener('click', () => { again = true; });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const err = $('[data-err]', form)!;
    err.textContent = '';
    const kind = v.kind as Kind;
    if (!v.name.trim()) { err.textContent = 'Enter a name.'; return; }
    const sell = kind === 'material' ? 0 : toKobo(v.selling_price);
    if (kind !== 'material' && sell <= 0) { err.textContent = 'Enter a selling price.'; return; }
    const input = {
      id: isNew ? null : item.id, sku: v.sku.trim(), barcode: v.barcode.trim(), name: v.name.trim(), category_id: v.category_id ? Number(v.category_id) : null,
      kind, unit: v.unit.trim(), selling_price_kobo: sell, cost_price_kobo: toKobo(v.cost_price), track_stock: kind !== 'service' && v.track_stock === '1',
      stock_qty: isNew ? Number(v.stock_qty || 0) : item.stock_qty, reorder_level: Number(v.reorder_level || 0), description: v.description ?? '', active: isNew ? true : v.active === '1',
      pack_unit: kind === 'service' ? '' : v.pack_unit.trim(), pack_size: Number(v.pack_size || 0), pack_price_kobo: toKobo(v.pack_price), wholesale_price_kobo: toKobo(v.wholesale_price), wholesale_min_qty: Number(v.wholesale_min || 0),
      track_expiry: kind !== 'service' && v.track_stock === '1' && v.track_expiry === '1',
    };
    if (input.pack_unit && !(input.pack_size > 1)) { err.textContent = 'Enter how many units are in one pack (more than 1), or clear the pack name.'; return; }
    if (input.wholesale_price_kobo && !(input.wholesale_min_qty > 0)) { err.textContent = 'Enter the quantity from which the wholesale price applies.'; return; }
    if (!Number.isFinite(input.stock_qty) || input.stock_qty < 0 || !Number.isFinite(input.reorder_level) || input.reorder_level < 0) { err.textContent = 'Stock numbers must be zero or more.'; return; }
    const btn = again ? $<HTMLButtonElement>('[data-again]', form) : $<HTMLButtonElement>('.btn-primary', form);
    withBusy(btn, async () => {
      let id: number;
      try {
        id = await call<number>('save_product', { input });
      } catch (x) { err.textContent = errText(x); again = false; return; }
      toast(`${input.name} saved`);
      m.close();
      await done(id);
      if (again) { await store.loadProducts(true); itemForm(null, done); }
    });
  });
  $('[data-remove]', form)?.addEventListener('click', async () => {
    if (await confirmDialog({ title: `Delete ${item.name}?`, message: 'If it has been sold before, it is archived instead so old receipts stay correct.', confirm: 'Delete', danger: true }) === null) return;
    try {
      const how = await call<string>('remove_product', { productId: item.id });
      toast(how === 'archived' ? `${item.name} archived` : `${item.name} deleted`);
      m.close(); done();
    } catch (x) { toast(errText(x), 'error'); }
  });
}

function stockForm(p: Product, done: () => void) {
  const cur = esc(store.settings.currency_symbol || '₦');
  const hasPack = !!p.pack_unit && p.pack_size > 1;
  const reasons = ['Died or culled', 'Used on the farm', 'Spoilt', 'Damaged', 'Given out', 'Stolen or missing'];
  const m = modal({ title: `Stock: ${p.name}`, subtitle: `${qty(p.stock_qty)} ${p.unit} in stock now`, size: 'md', body: `<form class="stack">
    <div class="seg seg-wide" role="radiogroup" aria-label="Stock action">
      <label><input type="radio" name="mode" value="stock_in" checked><span><b>Receive</b><small>New stock arrived</small></span></label>
      <label><input type="radio" name="mode" value="stock_out"><span><b>Remove</b><small>Damaged, used or lost</small></span></label>
      <label><input type="radio" name="mode" value="count"><span><b>Count</b><small>Set the exact amount</small></span></label>
    </div>
    <div class="grid-2">
      <label class="field"><span data-qlabel>Quantity received</span><span class="input-affix"><input name="quantity" inputmode="decimal" required autofocus>${hasPack ? `<select name="as" class="affix-select" aria-label="Unit"><option value="unit">${esc(p.unit)}</option><option value="pack">${esc(p.pack_unit)} of ${qty(p.pack_size)}</option></select>` : `<span class="affix">${esc(p.unit)}</span>`}</span></label>
      <label class="field" data-costbox><span data-clabel>Cost per ${esc(p.unit)} <em class="muted">(optional)</em></span><span class="input-affix"><span class="affix">${cur}</span><input name="unit_cost" inputmode="decimal" placeholder="${nairaInput(p.cost_price_kobo)}"></span><small>Updates the average cost.</small></label>
    </div>
    ${p.track_expiry ? `<div class="grid-2" data-batchbox><label class="field"><span>Batch / lot no. <em class="muted">(optional)</em></span><input name="batch_no" maxlength="40"></label><label class="field"><span>Expiry date</span><input type="date" name="expiry_date" min="${isoDate(new Date())}"></label></div>` : ''}
    <label class="field"><span data-nlabel>Note <em class="muted">(optional)</em></span><input name="note" maxlength="200" list="reason-list" placeholder="e.g. supplier invoice no."><datalist id="reason-list">${reasons.map((r) => `<option value="${esc(r)}">`).join('')}</datalist></label>
    <p class="muted small" data-preview></p>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">Save stock change</button></div>
  </form>` });
  const form = m.el.querySelector('form')!;
  const sync = () => {
    const mode = (form.querySelector('input[name=mode]:checked') as HTMLInputElement).value;
    const asPack = hasPack && mode !== 'count' && (form.elements.namedItem('as') as HTMLSelectElement).value === 'pack';
    const q = Number((form.elements.namedItem('quantity') as HTMLInputElement).value || 0) * (asPack ? p.pack_size : 1);
    $('[data-clabel]', form)!.innerHTML = `Cost per ${esc(asPack ? p.pack_unit : p.unit)} <em class="muted">(optional)</em>`;
    const bb = $('[data-batchbox]', form); if (bb) bb.hidden = mode !== 'stock_in';
    const asSel = form.elements.namedItem('as') as HTMLSelectElement | null; if (asSel) asSel.disabled = mode === 'count';
    $('[data-qlabel]', form)!.textContent = { stock_in: 'Quantity received', stock_out: 'Quantity removed', count: 'Quantity counted' }[mode]!;
    $('[data-costbox]', form)!.hidden = mode !== 'stock_in';
    $('[data-nlabel]', form)!.innerHTML = mode === 'stock_out' ? 'Reason' : 'Note <em class="muted">(optional)</em>';
    (form.elements.namedItem('note') as HTMLInputElement).placeholder = mode === 'stock_out' ? reasons.join(', ') : 'e.g. supplier invoice no.';
    const after = mode === 'stock_in' ? p.stock_qty + q : mode === 'stock_out' ? p.stock_qty - q : q;
    $('[data-preview]', form)!.textContent = q || mode === 'count' ? `Stock after this: ${qty(after)} ${p.unit}` : '';
  };
  sync();
  form.addEventListener('input', sync);
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const err = $('[data-err]', form)!;
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        const q = await call<number>('adjust_stock', { input: { product_id: p.id, mode: v.mode, quantity: Number(v.quantity), unit_cost_kobo: v.unit_cost ? toKobo(v.unit_cost) : null, note: v.note,
          as_pack: hasPack && v.mode !== 'count' && v.as === 'pack', batch_no: v.batch_no ?? '', expiry_date: v.mode === 'stock_in' ? (v.expiry_date ?? '') : '' } });
        toast(`${p.name}: ${qty(q)} ${p.unit} in stock`);
        m.close(); done();
      } catch (x) { err.textContent = errText(x); }
    });
  });
}

// ---------------------------------------------------------------- recipes

/** Ingredients used for ONE unit of a meal or service. Selling it takes these out of stock. */
async function recipeForm(p: Product, done: () => void) {
  const lines = await call<RecipeLine[]>('get_recipe', { productId: p.id });
  const materials = store.products.filter((x) => x.active && x.id !== p.id && x.kind !== 'service').sort((a, b) => a.name.localeCompare(b.name));
  const m = modal({ title: `Recipe: ${p.name}`, subtitle: `What goes into one ${p.unit}`, size: 'md', body: `<form class="stack">
    <p class="muted small">Each sale of ${esc(p.name)} takes these ingredients out of stock, and its cost becomes the cost of the ingredients. Leave the list empty to remove the recipe.</p>
    ${p.track_stock ? `<p class="callout callout-info small">${icon('alert', 16)}<span>${esc(p.name)} also keeps its own stock count. For meals cooked to order, edit the item and untick <b>Track stock</b>, so only the ingredients are counted.</span></p>` : ''}
    <div data-rlines></div>
    <div><button type="button" class="btn btn-ghost btn-sm" data-radd>${icon('plus', 15)} Add ingredient</button></div>
    <div class="sum-row"><span>Ingredient cost per ${esc(p.unit)}</span><b data-rcost></b></div>
    <p class="muted small" data-rmargin></p>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">Save recipe</button></div></form>` });
  const form = m.el.querySelector('form')!;
  const rows: Array<{ id: number; q: number }> = lines.map((l) => ({ id: l.material_id, q: l.quantity }));
  const box = $('[data-rlines]', form)!;
  const cost = () => rows.reduce((a, r) => a + Math.round((materials.find((x) => x.id === r.id)?.cost_price_kobo ?? 0) * r.q), 0);
  const sums = () => {
    const c = cost();
    $('[data-rcost]', form)!.textContent = money(c);
    $('[data-rmargin]', form)!.textContent = c && p.selling_price_kobo ? `Sells at ${money(p.selling_price_kobo)}, leaving ${money(p.selling_price_kobo - c)} (${Math.round(((p.selling_price_kobo - c) / p.selling_price_kobo) * 100)}%) before other costs.` : '';
  };
  const draw = () => {
    box.innerHTML = rows.map((r, i) => {
      const mat = materials.find((x) => x.id === r.id);
      return `<div class="rline" data-i="${i}"><select data-k="id" aria-label="Ingredient"><option value="">Choose…</option>${materials.map((x) => `<option value="${x.id}" ${x.id === r.id ? 'selected' : ''}>${esc(x.name)}</option>`).join('')}</select>
        <span class="input-affix"><input data-k="q" inputmode="decimal" value="${r.q || ''}" aria-label="Quantity"><span class="affix">${esc(mat?.unit ?? '')}</span></span>
        <button type="button" class="icon-btn" data-rdrop="${i}" aria-label="Remove ingredient">${icon('close', 15)}</button></div>`;
    }).join('') || '<p class="muted small">No ingredients yet.</p>';
    sums();
  };
  draw();
  box.addEventListener('change', (e) => { const el = e.target as HTMLSelectElement; const i = Number(el.closest<HTMLElement>('[data-i]')?.dataset.i); if (el.dataset.k === 'id') { rows[i].id = Number(el.value); draw(); } });
  box.addEventListener('input', (e) => { const el = e.target as HTMLInputElement; const i = Number(el.closest<HTMLElement>('[data-i]')?.dataset.i); if (el.dataset.k === 'q') { rows[i].q = Number(el.value.replace(',', '.')) || 0; sums(); } });
  box.addEventListener('click', (e) => { const d = (e.target as HTMLElement).closest<HTMLElement>('[data-rdrop]'); if (d) { rows.splice(Number(d.dataset.rdrop), 1); draw(); } });
  $('[data-radd]', form)!.addEventListener('click', () => { rows.push({ id: 0, q: 0 }); draw(); });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const used = rows.filter((r) => r.id);
    if (used.some((r) => !(r.q > 0))) { $('[data-err]', form)!.textContent = 'Every ingredient needs a quantity.'; return; }
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('save_recipe', { productId: p.id, lines: used.map((r) => ({ material_id: r.id, quantity: r.q })) }); }
      catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast(used.length ? 'Recipe saved' : 'Recipe removed'); m.close(); done();
    });
  });
}

// ---------------------------------------------------------------- expiry

async function expiryPane(pane: HTMLElement) {
  const warn = Number(store.settings.expiry_warn_days || 30);
  let within: number | null = warn;
  pane.innerHTML = `<div class="toolbar"><div class="chips" role="group" aria-label="Show">
      <button class="chip is-on" data-within="${warn}">Expired or due in ${warn} days</button><button class="chip" data-within="90">Next 90 days</button><button class="chip" data-within="all">All batches</button></div></div>
    <section class="panel flush" data-batches></section>`;
  const load = async () => {
    const rows = await call<Batch[]>('list_batches', { productId: null, withinDays: within });
    $('[data-batches]', pane)!.innerHTML = rows.length ? `<div class="table-scroll"><table class="table"><thead><tr><th>${esc(store.label('item'))}</th><th>Batch</th><th>Expiry date</th><th class="num">Left</th><th class="actions-col"></th></tr></thead><tbody>
      ${rows.map((b) => `<tr><td><b>${esc(b.product_name)}</b></td><td>${esc(b.batch_no || '—')}</td><td>${esc(dateLabel(b.expiry_date))} ${b.days_left < 0 ? `<span class="pill pill-bad">Expired ${-b.days_left} day${b.days_left === -1 ? '' : 's'} ago</span>` : b.days_left <= warn ? `<span class="pill pill-warn">${b.days_left === 0 ? 'Today' : `${b.days_left} day${b.days_left === 1 ? '' : 's'} left`}</span>` : `<span class="muted small">${b.days_left} days</span>`}</td>
        <td class="num">${qty(b.quantity_left)} <small class="muted">${esc(b.unit)}</small></td>
        <td class="actions-col">${b.days_left < 0 ? `<button class="btn btn-ghost btn-sm btn-danger-text" data-writeoff="${b.id}">Remove from stock</button>` : ''}</td></tr>`).join('')}</tbody></table></div>`
      : emptyState('Nothing expiring', 'Turn on “Track expiry dates” for an item, then record the expiry date when stock arrives.');
    pane.onclick = async (e) => {
      const t = e.target as HTMLElement;
      const ch = t.closest<HTMLElement>('[data-within]');
      if (ch) { within = ch.dataset.within === 'all' ? null : Number(ch.dataset.within); pane.querySelectorAll('[data-within]').forEach((c) => c.classList.toggle('is-on', c === ch)); load(); return; }
      const w = t.closest<HTMLElement>('[data-writeoff]');
      if (!w) return;
      const b = rows.find((x) => x.id === Number(w.dataset.writeoff))!;
      if (await confirmDialog({ title: `Remove expired ${b.product_name}?`, message: `${qty(b.quantity_left)} ${b.unit} from batch ${b.batch_no || '(no number)'} will be taken out of stock as expired.`, confirm: 'Remove from stock', danger: true }) === null) return;
      try {
        await call('adjust_stock', { input: { product_id: b.product_id, mode: 'stock_out', quantity: b.quantity_left, unit_cost_kobo: null, note: `Expired (batch ${b.batch_no || b.expiry_date})`, batch_id: b.id } });
        toast('Expired stock removed'); load();
      } catch (x) { toast(errText(x), 'error'); }
    };
  };
  await load();
}

// ---------------------------------------------------------------- labels

function labelsDialog() {
  const items = store.products.filter((p) => p.active && p.kind !== 'material').sort((a, b) => a.name.localeCompare(b.name));
  const copies = new Map<number, number>();
  const m = modal({ title: 'Print labels', subtitle: 'Price labels with barcodes for shelves and products', size: 'lg', body: `<div class="stack">
    <div class="toolbar tight"><div class="search-field grow">${icon('search', 16)}<input type="search" data-lq placeholder="Find ${esc(store.label('items').toLowerCase())}"></div>
      <label class="field inline-field"><span>Label size</span><select data-size><option value="a4">A4 sheet, 24 per page (63.5 × 33.9 mm)</option><option value="roll">Label printer roll (50 × 30 mm)</option></select></label></div>
    <div class="label-pick" data-list></div>
    <p class="muted small">Items without a barcode get an in-store code starting with 20, which any scanner reads. Their barcode is saved so the till recognises it.</p>
    <div class="form-actions"><span class="push-left muted" data-count>0 labels</span><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary" data-go>${icon('print', 16)} Print labels</button></div></div>` });
  const list = $('[data-list]', m.el)!;
  const draw = (q = '') => {
    const ql = q.toLowerCase();
    list.innerHTML = items.filter((p) => !ql || p.name.toLowerCase().includes(ql) || p.sku.toLowerCase().includes(ql)).slice(0, 300).map((p) => `<label class="label-row"><span><b>${esc(p.name)}</b><small class="muted block">${esc(p.barcode || 'No barcode yet')} · ${money(p.selling_price_kobo)}</small></span>
      <input type="number" min="0" max="500" class="w-num" data-copies="${p.id}" value="${copies.get(p.id) ?? 0}" aria-label="Number of labels for ${esc(p.name)}"></label>`).join('') || '<p class="muted">No match.</p>';
  };
  const count = () => { const n = [...copies.values()].reduce((a, b) => a + b, 0); $('[data-count]', m.el)!.textContent = `${n} label${n === 1 ? '' : 's'}`; };
  draw();
  $('[data-lq]', m.el)!.addEventListener('input', debounce((e: Event) => draw((e.target as HTMLInputElement).value), 150));
  list.addEventListener('input', (e) => { const el = e.target as HTMLInputElement; if (el.dataset.copies) { copies.set(Number(el.dataset.copies), Math.max(0, Math.min(500, Math.round(Number(el.value) || 0)))); count(); } });
  $<HTMLButtonElement>('[data-go]', m.el)!.addEventListener('click', (e) => withBusy(e.currentTarget as HTMLButtonElement, async () => {
    const chosen = [...copies.entries()].filter(([, n]) => n > 0);
    if (!chosen.length) { toast('Enter how many labels you need for at least one item', 'error'); return; }
    const codes = new Map(await call<Array<[number, string]>>('generate_barcodes', { productIds: chosen.map(([id]) => id) }));
    await store.loadProducts();
    const roll = ($('[data-size]', m.el) as HTMLSelectElement).value === 'roll';
    const biz = esc(store.settings.business_name || '');
    const one = (p: Product) => `<div class="plabel ${roll ? 'plabel-roll' : ''}"><span class="plabel-biz">${biz}</span><b class="plabel-name">${esc(p.name)}</b><span class="plabel-price">${money(p.selling_price_kobo)}<small>/${esc(p.unit)}</small></span>${barcodeSvg(codes.get(p.id) || p.barcode, { widthMm: roll ? 44 : 52, heightMm: roll ? 9 : 10 })}</div>`;
    const html = chosen.flatMap(([id, n]) => { const p = store.products.find((x) => x.id === id)!; return Array.from({ length: n }, () => one(p)); }).join('');
    await printHtml(`<div class="label-sheet ${roll ? 'is-roll' : ''}">${html}</div>`, roll ? '@page { size: 50mm 30mm; margin: 0; }' : '@page { size: A4; margin: 12.9mm 7.2mm; }');
  }));
}

// ---------------------------------------------------------------- categories

async function categoriesPane(pane: HTMLElement) {
  await store.loadCategories();
  pane.innerHTML = `<section class="panel narrow">
    <header class="panel-head"><div><h2>Categories</h2><p class="muted small">Group ${esc(store.label('items').toLowerCase())} on Sell produce and in reports. Lower numbers show first.</p></div></header>
    <form class="inline-form" data-add>
      <input name="name" maxlength="60" required placeholder="New category name" aria-label="New category name">
      <input name="sort" type="number" min="0" max="9999" value="${store.categories.length + 1}" aria-label="Order" class="w-num">
      <button class="btn btn-primary">${icon('plus', 16)} Add</button>
    </form>
    ${store.categories.length ? `<ul class="cat-list">${store.categories.map((c) => `<li data-id="${c.id}">
      <input value="${esc(c.name)}" maxlength="60" data-name aria-label="Category name">
      <input type="number" class="w-num" value="${c.sort_order}" data-sort aria-label="Order">
      <span class="muted small">${c.item_count} ${c.item_count === 1 ? 'item' : 'items'}</span>
      <button class="btn btn-ghost btn-sm" data-save>${esc(t('common.save'))}</button>
      <button class="icon-btn" data-del aria-label="Delete ${esc(c.name)}">${icon('trash', 16)}</button>
    </li>`).join('')}</ul>` : '<p class="muted pad">No categories yet.</p>'}
  </section>`;
  $<HTMLFormElement>('[data-add]', pane)!.addEventListener('submit', (e) => {
    e.preventDefault();
    const form = e.currentTarget as HTMLFormElement;
    const v = formValues(form);
    withBusy(form.querySelector('button'), async () => { await call('save_category', { id: null, name: v.name, sortOrder: Number(v.sort) || 0 }); toast('Category added'); await categoriesPane(pane); });
  });
  pane.onclick = async (e) => {
    const t = e.target as HTMLElement;
    const li = t.closest<HTMLElement>('li[data-id]');
    if (!li) return;
    const id = Number(li.dataset.id);
    if (t.closest('[data-save]')) withBusy(t.closest('button'), async () => {
      await call('save_category', { id, name: ($('[data-name]', li) as HTMLInputElement).value, sortOrder: Number(($('[data-sort]', li) as HTMLInputElement).value) || 0 });
      toast('Category saved'); await categoriesPane(pane);
    });
    if (t.closest('[data-del]')) {
      if (await confirmDialog({ title: 'Delete this category?', message: 'Items in it are kept and become uncategorised.', confirm: 'Delete category', danger: true }) === null) return;
      try { await call('delete_category', { id }); toast('Category deleted'); await categoriesPane(pane); } catch (x) { toast(errText(x), 'error'); }
    }
  };
}

// ---------------------------------------------------------------- history

async function historyPane(pane: HTMLElement) {
  const today = new Date();
  const from = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 30);
  pane.innerHTML = `<div class="toolbar">
      <select data-h="product" aria-label="${esc(store.label('item'))}"><option value="">All ${esc(store.label('items').toLowerCase())}</option>${store.products.filter((p) => p.track_stock).map((p) => `<option value="${p.id}">${esc(p.name)}</option>`).join('')}</select>
      <label class="date-field"><span>From</span><input type="date" data-h="from" value="${isoDate(from)}"></label>
      <label class="date-field"><span>To</span><input type="date" data-h="to" value="${isoDate(today)}"></label>
      <button class="btn btn-ghost" data-hexport>${icon('download', 16)} Export</button>
    </div><section class="panel flush" data-hist></section>`;
  let rows: Movement[] = [];
  const load = async () => {
    const val = (k: string) => ($(`[data-h="${k}"]`, pane) as HTMLInputElement).value;
    rows = await call<Movement[]>('list_movements', { productId: val('product') ? Number(val('product')) : null, from: val('from'), to: val('to') });
    $('[data-hist]', pane)!.innerHTML = rows.length ? `<div class="table-scroll"><table class="table">
      <thead><tr><th>When</th><th>${esc(store.label('item'))}</th><th>Change</th><th class="num">Quantity</th><th>Note</th><th>By</th></tr></thead>
      <tbody>${rows.map((r) => `<tr><td>${esc(relativeDay(r.created_at))}</td><td>${esc(r.product_name)}</td><td><span class="move move-${r.quantity_delta >= 0 ? 'in' : 'out'}">${esc(MOVE_LABEL[r.movement_type] ?? r.movement_type)}</span></td>
        <td class="num"><b class="${r.quantity_delta >= 0 ? 'ok-text' : ''}">${r.quantity_delta >= 0 ? '+' : '−'}${qty(Math.abs(r.quantity_delta))}</b> <small class="muted">${esc(r.unit)}</small></td>
        <td>${esc(r.note)}${r.receipt_no ? ` <small class="muted">${esc(r.receipt_no)}</small>` : ''}</td><td>${esc(r.user_name || '—')}</td></tr>`).join('')}</tbody></table></div>`
      : emptyState('No stock changes', 'Nothing was received, sold or adjusted in this period.');
  };
  await load();
  pane.onchange = (e) => { if ((e.target as HTMLElement).closest('[data-h]')) load().catch((x) => toast(errText(x), 'error')); };
  pane.onclick = (e) => {
    if (!(e.target as HTMLElement).closest('[data-hexport]')) return;
    exportCsv(`stock-history-${isoDate(new Date())}.csv`, ['Date', 'Item', 'Change', 'Quantity', 'Unit', 'Unit cost', 'Note', 'Receipt', 'By'],
      rows.map((r) => [r.created_at, r.product_name, MOVE_LABEL[r.movement_type] ?? r.movement_type, r.quantity_delta, r.unit, nairaPlain(r.unit_cost_kobo), r.note, r.receipt_no, r.user_name]));
  };
}
