import { t } from '../lib/i18n';
import { auth, call } from '../api';
import { $, $$, confirmDialog, debounce, errText, esc, modal, toast, withBusy } from '../lib/dom';
import { money, nairaInput, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { printReceipt, receiptMarkup } from '../lib/receipt';
import { receiptText, shareWhatsApp } from '../lib/whatsapp';
import { shareReceipt } from '../lib/receipt-pdf';
import { store } from '../store';
import type { Customer, HeldSale, Product, Receipt, ShiftReport } from '../types';
import type { ViewCtx } from './shell';
import { openShiftDialog } from './shifts';
import { isMobile } from '../lib/platform';

type SellAs = 'unit' | 'pack';
/** `override` is a price typed at the till; otherwise the list, pack or wholesale price applies. */
interface Line { product: Product; qty: number; sellAs: SellAs; override: number | null }
interface Pick { id: number | null; name: string; phone: string; email: string }

// The sale in progress survives moving between pages.
let cart: Line[] = [];
let discount = 0;
let customer: Pick | null = null;
let activeCategory: number | 'all' = 'all';
let query = '';
let shift: ShiftReport | null = null;

const hasPack = (p: Product) => !!p.pack_unit && p.pack_size > 1;
const factor = (l: Line) => (l.sellAs === 'pack' ? l.product.pack_size : 1);
const unitName = (l: Line) => (l.sellAs === 'pack' ? l.product.pack_unit : l.product.unit);
const isWholesale = (l: Line) => l.sellAs === 'unit' && l.product.wholesale_price_kobo > 0 && l.product.wholesale_min_qty > 0 && l.qty + 1e-9 >= l.product.wholesale_min_qty;
function listPrice(l: Line): number {
  const p = l.product;
  if (l.sellAs === 'pack') return p.pack_price_kobo > 0 ? p.pack_price_kobo : Math.round(p.selling_price_kobo * p.pack_size);
  return isWholesale(l) ? p.wholesale_price_kobo : p.selling_price_kobo;
}
const price = (l: Line) => l.override ?? listPrice(l);

const sellable = () => store.products.filter((p) => p.active && p.kind !== 'material');
const canOverride = () => store.flag('allow_price_override') || auth.can('manager');
const allowNegative = () => store.flag('allow_negative_stock');

function totals() {
  const subtotal = cart.reduce((a, l) => a + Math.round(price(l) * l.qty), 0);
  const d = Math.min(discount, subtotal);
  const rate = store.flag('tax_enabled') ? Number(store.settings.tax_rate || 0) : 0;
  const tax = rate > 0 ? Math.round(((subtotal - d) * rate) / 100) : 0;
  return { subtotal, discount: d, tax, total: subtotal - d + tax, count: cart.reduce((a, l) => a + l.qty, 0) };
}

/** Base units (pieces, plates, kg…) of an item already in the sale. */
function inCart(id: number) { return cart.filter((l) => l.product.id === id).reduce((a, l) => a + l.qty * factor(l), 0); }

function stockOk(p: Product, extraBase: number): boolean {
  if (p.track_stock && !allowNegative() && inCart(p.id) + extraBase > p.stock_qty + 1e-9) {
    toast(p.stock_qty <= 0 ? `${p.name} is out of stock` : `Only ${qty(p.stock_qty)} ${p.unit} of ${p.name} in stock`, 'error');
    return false;
  }
  return true;
}

function addProduct(p: Product, amount = 1, sellAs: SellAs = 'unit'): boolean {
  if (!stockOk(p, amount * (sellAs === 'pack' ? p.pack_size : 1))) return false;
  const line = cart.find((l) => l.product.id === p.id && l.sellAs === sellAs && l.override === null);
  if (line) line.qty = Math.round((line.qty + amount) * 1000) / 1000;
  else cart.push({ product: p, qty: amount, sellAs, override: null });
  return true;
}

export async function render(ctx: ViewCtx) {
  const [, , , current] = await Promise.all([store.loadProducts(), store.loadCategories(), store.loadPaymentMethods(), call<ShiftReport | null>('current_shift').catch(() => null)]);
  shift = current;
  const drawHeader = (held = 0) => ctx.header(t('page.pos.title', {}, 'New sale'), shift ? `Shift open since ${shift.opened_at.slice(11, 16)}` : store.flag('require_shift') ? 'Open your shift to start selling.' : 'Tap items or scan barcodes, then take payment.', `
    <button class="btn btn-ghost" data-act="shift">${icon('cash', 16)} ${shift ? 'Shift' : 'Open shift'}</button>
    <button class="btn btn-ghost" data-act="held">${icon('pause', 16)} Held sales${held ? ` <span class="count-badge">${held}</span>` : ''}</button>
    <button class="btn btn-ghost" data-act="clear">${icon('trash', 16)} Clear</button>`);
  let heldCount = 0;
  const refreshHeld = async () => { heldCount = (await call<HeldSale[]>('list_held_sales').catch(() => [])).length; drawHeader(heldCount); };
  drawHeader();
  refreshHeld();
  // Drop cart lines whose item disappeared or changed since the last visit.
  cart = cart.filter((l) => store.products.some((p) => p.id === l.product.id && p.active)).map((l) => ({ ...l, product: store.products.find((p) => p.id === l.product.id)! }))
    .filter((l) => l.sellAs === 'unit' || hasPack(l.product));

  const cats = store.categories.filter((c) => sellable().some((p) => p.category_id === c.id));
  ctx.main.innerHTML = `<div class="pos">
    <section class="pos-catalog" aria-label="${esc(store.label('items'))}">
      <div class="pos-search">
        ${icon('search', 18)}
        <input type="search" id="pos-search" placeholder="Search ${esc(store.label('items').toLowerCase())} or scan a barcode" autocomplete="off" spellcheck="false" value="${esc(query)}" aria-label="Search or scan">
        ${isMobile ? `<button type="button" class="btn btn-ghost btn-sm cam-btn" data-camscan aria-label="Scan with camera">${icon('barcode', 16)} Scan</button>` : `<span class="pos-search-hint">${icon('barcode', 16)} Scanner ready</span>`}
      </div>
      <div class="chips" role="tablist" aria-label="Categories">
        <button class="chip ${activeCategory === 'all' ? 'is-on' : ''}" data-cat="all" role="tab" aria-selected="${activeCategory === 'all'}">All</button>
        ${cats.map((c) => `<button class="chip ${activeCategory === c.id ? 'is-on' : ''}" data-cat="${c.id}" role="tab" aria-selected="${activeCategory === c.id}">${esc(c.name)}</button>`).join('')}
      </div>
      <div class="tiles" id="tiles"></div>
    </section>
    <aside class="pos-ticket" aria-label="Current sale">
      <div class="ticket" id="ticket"></div>
      <div class="ticket-actions" id="ticket-actions"></div>
    </aside>
  </div>`;

  const tiles = $('#tiles', ctx.main)!;
  const ticket = $('#ticket', ctx.main)!;
  const actions = $('#ticket-actions', ctx.main)!;
  const search = $<HTMLInputElement>('#pos-search', ctx.main)!;

  const filtered = () => {
    const q = query.trim().toLowerCase();
    return sellable().filter((p) => (activeCategory === 'all' || p.category_id === activeCategory) &&
      (!q || p.name.toLowerCase().includes(q) || p.sku.toLowerCase().includes(q) || p.barcode.toLowerCase() === q));
  };

  const drawTiles = () => {
    const list = filtered();
    if (!sellable().length) {
      tiles.innerHTML = `<div class="empty"><h3>Nothing to sell yet</h3><p>${auth.can('manager') ? `Add your ${esc(store.label('items').toLowerCase())} and their prices in Farm stock, then come back here.` : 'Ask a manager to add items in Farm stock.'}</p>${auth.can('manager') ? '<button class="btn btn-primary" data-go-inventory>Open Farm stock</button>' : ''}</div>`;
      return;
    }
    if (!list.length) { tiles.innerHTML = `<div class="empty compact"><p>No ${esc(store.label('items').toLowerCase())} match “${esc(query)}”.</p></div>`; return; }
    tiles.innerHTML = list.map((p) => {
      const left = p.stock_qty - inCart(p.id);
      const low = p.track_stock && p.stock_qty <= p.reorder_level;
      const out = p.track_stock && left <= 0 && !allowNegative();
      const stock = p.kind === 'service' ? 'Service' : p.track_stock ? (left <= 0 ? 'Out of stock' : `${qty(left)} ${p.unit} left`) : `Per ${p.unit}`;
      const n = inCart(p.id);
      return `<button class="tile ${out ? 'is-out' : ''} ${low ? 'is-low' : ''}" data-add="${p.id}" ${out ? 'aria-disabled="true"' : ''}>
        ${n ? `<span class="tile-count" aria-label="${qty(n)} in sale">${qty(n)}</span>` : ''}
        <span class="tile-name">${esc(p.name)}</span>
        <span class="tile-foot"><b>${money(p.selling_price_kobo)}</b><small>${esc(stock)}</small></span>
        ${hasPack(p) ? `<span class="tile-pack" data-addpack="${p.id}" aria-hidden="true" title="Add one ${esc(p.pack_unit)}">+ ${esc(p.pack_unit)} ${money(p.pack_price_kobo || Math.round(p.selling_price_kobo * p.pack_size), { whole: true })}</span>` : ''}
      </button>`;
    }).join('');
  };

  const drawTicket = () => {
    const t = totals();
    const s = store.settings;
    ticket.innerHTML = `
      <div class="ticket-head">
        <div><span class="ticket-biz">${esc(s.business_name || 'Current sale')}</span><span class="ticket-sub">${cart.length ? `${cart.length} line${cart.length === 1 ? '' : 's'}` : 'New sale'}</span></div>
        <button class="ticket-customer ${customer ? 'has-customer' : ''}" data-act="customer">${icon('people', 16)}<span>${customer ? esc(customer.name) : `Add ${esc(store.label('customer').toLowerCase())}`}</span></button>
      </div>
      <div class="ticket-lines">
        ${cart.length ? cart.map((l, i) => `
          <div class="tline">
            <div class="tline-top"><span class="tline-name">${esc(l.product.name)}${isWholesale(l) && l.override === null ? ' <span class="pill pill-info">Wholesale</span>' : ''}</span><button class="tline-x" data-remove="${i}" aria-label="Remove ${esc(l.product.name)}">${icon('close', 14)}</button></div>
            <div class="tline-bottom">
              <span class="stepper">
                <button data-dec="${i}" aria-label="Decrease quantity">${icon('minus', 14)}</button>
                <input data-qty="${i}" value="${qty(l.qty)}" inputmode="decimal" aria-label="Quantity of ${esc(l.product.name)}">
                <button data-inc="${i}" aria-label="Increase quantity">${icon('plus', 14)}</button>
              </span>
              ${hasPack(l.product) ? `<button class="tline-unit is-toggle" data-toggle="${i}" title="Switch between ${esc(l.product.unit)} and ${esc(l.product.pack_unit)}">${esc(unitName(l))} ⇄</button>` : `<span class="tline-unit">${esc(unitName(l))} ×</span>`}
              ${canOverride() ? `<button class="tline-price is-editable" data-price="${i}" title="Change price">${money(price(l))}</button>` : `<span class="tline-price">${money(price(l))}</span>`}
              <b class="tline-total">${money(Math.round(price(l) * l.qty))}</b>
            </div>
          </div>`).join('') : `<div class="ticket-empty">${icon('barcode', 26)}<p>Tap an item or scan a barcode to start this sale.</p></div>`}
      </div>
      <div class="ticket-sums">
        <div class="tsum"><span>Subtotal</span><span>${money(t.subtotal)}</span></div>
        <div class="tsum"><button class="link-btn" data-act="discount" ${cart.length ? '' : 'disabled'}>${t.discount ? 'Discount' : 'Add discount'}</button><span>${t.discount ? `−${money(t.discount)}` : ''}</span></div>
        ${t.tax ? `<div class="tsum"><span>${esc(s.tax_label || 'Tax')} ${esc(s.tax_rate)}%</span><span>${money(t.tax)}</span></div>` : ''}
        <div class="tsum tsum-total"><span>Total</span><span>${money(t.total)}</span></div>
      </div>`;
    actions.innerHTML = `<button class="btn btn-primary btn-xl btn-block" data-act="pay" ${cart.length ? '' : 'disabled'}>${icon('cash', 20)} Take payment<span class="btn-amount">${money(t.total)}</span></button>
      <div class="ticket-row"><button class="btn btn-ghost btn-sm" data-act="hold" ${cart.length ? '' : 'disabled'}>${icon('pause', 15)} Hold this sale</button><p class="ticket-hint"><kbd>F9</kbd> pay · <kbd>F2</kbd> search</p></div>`;
  };

  const redraw = () => { drawTiles(); drawTicket(); };
  redraw();
  search.focus();

  const onSearch = debounce(() => { query = search.value; drawTiles(); }, 120);
  search.addEventListener('input', onSearch);
  search.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter') return;
    e.preventDefault();
    handleCode();
  });
  // Phone camera scanning (Android): the scanned code goes through the same path as a USB scanner.
  $('[data-camscan]', ctx.main)?.addEventListener('click', async () => {
    try {
      const bs = await import('@tauri-apps/plugin-barcode-scanner');
      let perm = await bs.checkPermissions();
      if (perm !== 'granted') perm = await bs.requestPermissions();
      if (perm !== 'granted') { toast('Allow camera access for FarmBuddy in your phone settings to scan barcodes.', 'error'); return; }
      const res = await bs.scan({ windowed: false, formats: [bs.Format.EAN13, bs.Format.EAN8, bs.Format.UPC_A, bs.Format.UPC_E, bs.Format.Code128, bs.Format.Code39, bs.Format.QRCode] });
      if (res?.content) { search.value = res.content.trim(); handleCode(); }
    } catch (x) {
      const msg = errText(x);
      if (!/cancel/i.test(msg)) toast(`Camera scanning is not available: ${msg}`, 'error');
    }
  });
  function handleCode() {
    const code = search.value.trim().toLowerCase();
    if (!code) return;
    const exact = sellable().find((p) => (p.barcode && p.barcode.toLowerCase() === code) || p.sku.toLowerCase() === code);
    query = search.value;
    const list = filtered();
    const pick = exact ?? (list.length === 1 ? list[0] : null);
    if (pick) {
      if (addProduct(pick)) { query = ''; search.value = ''; redraw(); flash(pick.id); }
    } else if (!list.length) {
      const code = search.value.trim();
      if (auth.can('manager') && /^[A-Za-z0-9-]{4,64}$/.test(code)) offerNewBarcode(code);
      else { toast(`No ${store.label('item').toLowerCase()} with code “${code}”`, 'error'); search.select(); }
    } else drawTiles();
  }

  // A scanned code nobody has yet: managers can create the item on the spot, and it goes straight into the sale.
  const offerNewBarcode = async (code: string) => {
    const label = store.label('item').toLowerCase();
    const ok = await confirmDialog({ title: `New barcode: ${code}`, message: `No ${label} has this barcode yet. Add it as a new ${label} now? It will be added to this sale after you save.`, confirm: `Add new ${label}` });
    if (ok === null) { search.select(); return; }
    const { itemForm } = await import('./inventory');
    await store.loadCategories();
    itemForm(null, async (id) => {
      await store.loadProducts();
      const p = store.products.find((x) => x.id === id);
      query = ''; search.value = '';
      const added = !!p && p.kind !== 'material' && addProduct(p);
      redraw();
      if (added && p) flash(p.id);
      search.focus();
    }, { barcode: code });
  };

  const flash = (id: number) => {
    const el = $(`[data-add="${id}"]`, tiles);
    el?.classList.remove('flash'); void el?.offsetWidth; el?.classList.add('flash');
  };

  ctx.main.onclick = async (e) => {
    const t = e.target as HTMLElement;
    const chip = t.closest<HTMLElement>('[data-cat]');
    if (chip) {
      activeCategory = chip.dataset.cat === 'all' ? 'all' : Number(chip.dataset.cat);
      $$('.chip', ctx.main).forEach((c) => { c.classList.toggle('is-on', c === chip); c.setAttribute('aria-selected', String(c === chip)); });
      drawTiles(); return;
    }
    if (t.closest('[data-go-inventory]')) { ctx.go('inventory'); return; }
    const addPack = t.closest<HTMLElement>('[data-addpack]');
    if (addPack) { const p = store.products.find((x) => x.id === Number(addPack.dataset.addpack)); if (p && addProduct(p, 1, 'pack')) { redraw(); flash(p.id); } return; }
    const add = t.closest<HTMLElement>('[data-add]');
    if (add) { const p = store.products.find((x) => x.id === Number(add.dataset.add)); if (p && addProduct(p)) { redraw(); flash(p.id); } return; }
    const inc = t.closest<HTMLElement>('[data-inc]');
    if (inc) { const l = cart[Number(inc.dataset.inc)]; if (stockOk(l.product, factor(l))) { l.qty = Math.round((l.qty + 1) * 1000) / 1000; redraw(); } return; }
    const tog = t.closest<HTMLElement>('[data-toggle]');
    if (tog) {
      const l = cart[Number(tog.dataset.toggle)];
      const next: SellAs = l.sellAs === 'pack' ? 'unit' : 'pack';
      const delta = l.qty * ((next === 'pack' ? l.product.pack_size : 1) - factor(l));
      if (delta <= 0 || stockOk(l.product, delta)) { l.sellAs = next; l.override = null; redraw(); }
      return;
    }
    const dec = t.closest<HTMLElement>('[data-dec]');
    if (dec) { const i = Number(dec.dataset.dec); cart[i].qty = Math.round((cart[i].qty - 1) * 1000) / 1000; if (cart[i].qty <= 0) cart.splice(i, 1); redraw(); return; }
    const rm = t.closest<HTMLElement>('[data-remove]');
    if (rm) { cart.splice(Number(rm.dataset.remove), 1); redraw(); return; }
    const price = t.closest<HTMLElement>('[data-price]');
    if (price) { editPrice(cart[Number(price.dataset.price)], redraw); return; }
    const act = t.closest<HTMLElement>('[data-act]')?.dataset.act;
    if (act === 'discount') editDiscount(redraw);
    if (act === 'customer') pickCustomer(drawTicket);
    if (act === 'pay') pay();
    if (act === 'hold') holdSale(() => { redraw(); refreshHeld(); search.focus(); });
  };
  const pay = () => {
    if (store.flag('require_shift') && !shift) {
      openShiftDialog((s) => { shift = s; drawHeader(heldCount); checkout(() => { redraw(); search.focus(); }); });
      return;
    }
    checkout(() => { redraw(); search.focus(); });
  };
  // Header buttons live outside the page element.
  document.querySelector<HTMLElement>('[data-actions]')!.onclick = async (e) => {
    const act = (e.target as HTMLElement).closest<HTMLElement>('[data-act]')?.dataset.act;
    if (act === 'held') recallHeld(() => { redraw(); refreshHeld(); });
    if (act === 'shift') {
      if (shift) ctx.go('shifts');
      else openShiftDialog((s) => { shift = s; drawHeader(heldCount); search.focus(); });
    }
    if (act === 'clear') {
      if (!cart.length && !customer) return;
      if (await confirmDialog({ title: 'Clear this sale?', message: 'All lines, the discount and the customer will be removed.', confirm: 'Clear sale', danger: true }) === null) return;
      cart = []; discount = 0; customer = null; redraw(); search.focus();
    }
  };
  ctx.main.onchange = (e) => {
    const input = (e.target as HTMLElement).closest<HTMLInputElement>('[data-qty]');
    if (!input) return;
    const l = cart[Number(input.dataset.qty)];
    const v = Number(input.value.replace(',', '.'));
    if (!Number.isFinite(v) || v <= 0) { cart.splice(Number(input.dataset.qty), 1); redraw(); return; }
    const others = inCart(l.product.id) - l.qty * factor(l);
    if (l.product.track_stock && !allowNegative() && others + v * factor(l) > l.product.stock_qty + 1e-9) {
      toast(`Only ${qty(l.product.stock_qty)} ${l.product.unit} of ${l.product.name} in stock`, 'error');
      l.qty = Math.max(0, Math.floor(((l.product.stock_qty - others) / factor(l)) * 1000) / 1000);
      if (l.qty <= 0) cart.splice(cart.indexOf(l), 1);
    } else l.qty = Math.round(v * 1000) / 1000;
    redraw();
  };


  const onKey = (e: KeyboardEvent) => {
    if (!ctx.main.isConnected || !ctx.main.classList.contains('view-pos')) { document.removeEventListener('keydown', onKey); return; }
    if (document.body.classList.contains('has-modal')) return;
    if (e.key === 'F9' && cart.length) { e.preventDefault(); pay(); }
    if (e.key === 'F2') { e.preventDefault(); search.focus(); search.select(); }
  };
  document.addEventListener('keydown', onKey);
}

function editPrice(line: Line, done: () => void) {
  const m = modal({ title: 'Change price', subtitle: line.product.name, size: 'sm', body: `<form class="stack">
    <label class="field"><span>Price per ${esc(unitName(line))}</span><span class="input-affix"><span class="affix">${esc(store.settings.currency_symbol || '₦')}</span><input name="price" inputmode="decimal" value="${nairaInput(price(line))}" required autofocus></span><small>List price is ${money(listPrice(line))}.</small></label>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-reset>Use list price</button><button class="btn btn-primary">Apply</button></div></form>` });
  const form = m.el.querySelector('form')!;
  $('[data-reset]', form)!.addEventListener('click', () => { line.override = null; m.close(); done(); });
  form.addEventListener('submit', (e) => { e.preventDefault(); const v = toKobo((form.elements.namedItem('price') as HTMLInputElement).value); if (v < 0) return; line.override = v === listPrice(line) ? null : v; m.close(); done(); });
}

// ---------------------------------------------------------------- held (parked) sales

interface HeldCart { lines: Array<{ product_id: number; qty: number; sellAs: SellAs; override: number | null }>; discount: number; customer: Pick | null }

function holdSale(done: () => void) {
  const t = totals();
  const m = modal({ title: 'Hold this sale', size: 'sm', body: `<form class="stack">
    <p class="muted">Park the sale to serve someone else, then bring it back from <b>Held sales</b>. Stock is not taken until the sale is completed.</p>
    <label class="field"><span>Name it</span><input name="label" required maxlength="60" autofocus placeholder="e.g. Table 4, Mrs Bello, Car 2" value="${esc(customer?.name ?? '')}"></label>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>Cancel</button><button class="btn btn-primary">${icon('pause', 16)} Hold</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const held: HeldCart = { lines: cart.map((l) => ({ product_id: l.product.id, qty: l.qty, sellAs: l.sellAs, override: l.override })), discount, customer };
    withBusy(form.querySelector('.btn-primary'), async () => {
      await call('hold_sale', { label: (form.elements.namedItem('label') as HTMLInputElement).value, cartJson: JSON.stringify(held), itemCount: t.count, totalKobo: t.total });
      cart = []; discount = 0; customer = null;
      toast('Sale held'); m.close(); done();
    });
  });
}

async function recallHeld(done: () => void) {
  const rows = await call<HeldSale[]>('list_held_sales').catch((e) => { toast(errText(e), 'error'); return null; });
  if (!rows) return;
  const m = modal({ title: 'Held sales', size: 'md', body: rows.length ? `<ul class="pick-list">${rows.map((h) => `<li class="held-row">
      <button data-resume="${h.id}"><b>${esc(h.label)}</b><span>${qty(h.item_count)} item${h.item_count === 1 ? '' : 's'} · ${money(h.total_kobo)} · ${esc(h.user_name)}, ${esc(h.created_at.slice(11, 16))}</span></button>
      <button class="icon-btn" data-drop-held="${h.id}" aria-label="Delete held sale ${esc(h.label)}">${icon('trash', 16)}</button></li>`).join('')}</ul>`
    : '<p class="muted pad">No held sales. Use “Hold this sale” on the ticket to park one.</p>' });
  m.el.addEventListener('click', async (e) => {
    const t = e.target as HTMLElement;
    const drop = t.closest<HTMLElement>('[data-drop-held]');
    if (drop) { await call('delete_held_sale', { id: Number(drop.dataset.dropHeld) }); m.close(); done(); recallHeld(done); return; }
    const resume = t.closest<HTMLElement>('[data-resume]');
    if (!resume) return;
    if (cart.length && await confirmDialog({ title: 'Replace the current sale?', message: 'The sale on the ticket now will be cleared. Hold it first if you need it.', confirm: 'Replace' }) === null) return;
    const h = rows.find((x) => x.id === Number(resume.dataset.resume))!;
    const data = JSON.parse(h.cart_json) as HeldCart;
    await store.loadProducts();
    const missing: string[] = [];
    cart = [];
    for (const l of data.lines) {
      const p = store.products.find((x) => x.id === l.product_id && x.active);
      if (!p || (l.sellAs === 'pack' && !hasPack(p))) { missing.push(String(l.product_id)); continue; }
      cart.push({ product: p, qty: l.qty, sellAs: l.sellAs, override: l.override });
    }
    discount = data.discount; customer = data.customer;
    await call('delete_held_sale', { id: h.id });
    if (missing.length) toast(`${missing.length} item(s) are no longer available and were left out`, 'info');
    m.close(); done();
  });
}

function editDiscount(done: () => void) {
  const t = totals();
  const m = modal({ title: 'Discount', size: 'sm', body: `<form class="stack">
    <div class="seg" role="radiogroup" aria-label="Discount type"><label><input type="radio" name="mode" value="amount" checked><span>Amount</span></label><label><input type="radio" name="mode" value="percent"><span>Percent</span></label></div>
    <label class="field"><span>Discount</span><input name="value" inputmode="decimal" value="${nairaInput(discount)}" autofocus></label>
    <p class="muted small">Subtotal is ${money(t.subtotal)}.</p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-remove-discount>Remove discount</button><button class="btn btn-primary">Apply</button></div></form>` });
  const form = m.el.querySelector('form')!;
  $('[data-remove-discount]', form)!.addEventListener('click', () => { discount = 0; m.close(); done(); });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const mode = (form.querySelector('input[name=mode]:checked') as HTMLInputElement).value;
    const raw = (form.elements.namedItem('value') as HTMLInputElement).value;
    const v = mode === 'percent' ? Math.round((t.subtotal * Math.min(100, Math.max(0, Number(raw) || 0))) / 100) : toKobo(raw);
    if (v > t.subtotal) { toast('Discount cannot be more than the subtotal', 'error'); return; }
    discount = Math.max(0, v); m.close(); done();
  });
}

export function pickCustomer(done: () => void, onPick?: (c: Pick | null) => void) {
  const label = store.label('customer');
  const m = modal({ title: `Choose ${label.toLowerCase()}`, size: 'md', body: `
    <div class="stack">
      <div class="search-field">${icon('search', 16)}<input type="search" placeholder="Search by name, phone or email" data-csearch autofocus></div>
      <ul class="pick-list" data-clist><li class="muted">Loading…</li></ul>
      <details class="new-customer" ${customer && !customer.id ? 'open' : ''}><summary>${icon('plus', 15)} New ${esc(label.toLowerCase())}</summary>
        <form class="stack" data-cnew>
          <label class="field"><span>Name</span><input name="name" required maxlength="120" value="${esc(customer && !customer.id ? customer.name : '')}"></label>
          <div class="grid-2"><label class="field"><span>Phone</span><input name="phone" maxlength="40" inputmode="tel"></label><label class="field"><span>Email</span><input name="email" type="email" maxlength="254"></label></div>
          <label class="check"><input type="checkbox" name="save" checked><span>Save to ${esc(label.toLowerCase())} list</span></label>
          <div class="form-actions"><button class="btn btn-primary">Use this ${esc(label.toLowerCase())}</button></div>
        </form>
      </details>
      ${customer ? `<button class="btn btn-ghost" data-cclear>Remove ${esc(label.toLowerCase())} from sale</button>` : ''}
    </div>` });
  const list = $('[data-clist]', m.el)!;
  const set = (c: Pick | null) => { if (onPick) onPick(c); else customer = c; m.close(); done(); };
  const load = async (q = '') => {
    try {
      const rows = await call<Customer[]>('list_customers', { search: q });
      list.innerHTML = rows.length ? rows.slice(0, 50).map((c) => `<li><button data-cpick="${c.id}"><b>${esc(c.name)}</b><span>${esc([c.phone, c.email].filter(Boolean).join(' · ') || 'No contact details')}</span>${c.balance_kobo > 0 ? `<em>Owes ${money(c.balance_kobo)}</em>` : ''}</button></li>`).join('')
        : `<li class="muted">${q ? 'No match. Add them below.' : `No saved ${esc(label.toLowerCase())}s yet.`}</li>`;
      list.onclick = (e) => {
        const b = (e.target as HTMLElement).closest<HTMLElement>('[data-cpick]');
        if (!b) return;
        const c = rows.find((r) => r.id === Number(b.dataset.cpick))!;
        set({ id: c.id, name: c.name, phone: c.phone, email: c.email });
      };
    } catch (e) { list.innerHTML = `<li class="form-error">${esc(errText(e))}</li>`; }
  };
  load();
  $('[data-csearch]', m.el)!.addEventListener('input', debounce((e: Event) => load((e.target as HTMLInputElement).value), 200));
  $('[data-cclear]', m.el)?.addEventListener('click', () => set(null));
  const form = $<HTMLFormElement>('[data-cnew]', m.el)!;
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const f = new FormData(form);
    const pick: Pick = { id: null, name: String(f.get('name')).trim(), phone: String(f.get('phone')).trim(), email: String(f.get('email')).trim() };
    if (!pick.name) return;
    if ((form.elements.namedItem('save') as HTMLInputElement).checked) {
      await withBusy(form.querySelector('button'), async () => {
        pick.id = await call<number>('save_customer', { input: { id: null, name: pick.name, phone: pick.phone, email: pick.email, address: '', notes: '' } });
        set(pick);
      });
    } else set(pick);
  });
}

function checkout(after: () => void) {
  const t = totals();
  if (!cart.length) return;
  const methods = store.paymentMethods.filter((p) => p.active);
  const collection = store.flag('track_collection');
  const label = store.label('customer');
  const pays: Array<{ method: string; amount: number; reference: string }> = [{ method: methods[0]?.name ?? 'Cash', amount: t.total, reference: '' }];
  let pristine = true;
  const m = modal({ title: 'Take payment', size: 'lg', body: `<div class="checkout">
    <div class="checkout-due"><span>Amount due</span><strong>${money(t.total)}</strong><small>${cart.length} line${cart.length === 1 ? '' : 's'}${customer ? ` · ${esc(customer.name)}` : ''}</small></div>
    <div class="stack">
      <div><span class="field-label">Payment method</span><div class="method-row">${methods.map((p) => `<button type="button" class="method" data-method="${esc(p.name)}">${esc(p.name)}</button>`).join('')}</div></div>
      <div class="pay-lines" data-lines></div>
      <div class="quick-cash" data-quick></div>
      <div class="pay-summary" data-summary></div>
      <div class="grid-2">
        ${collection ? `<label class="field"><span>Collection date</span><input type="date" name="collection_date" data-collection></label>` : ''}
        <label class="field ${collection ? '' : 'span-2'}"><span>Note on receipt <em class="muted">(optional)</em></span><input name="note" maxlength="500" data-note placeholder="${collection ? 'e.g. tag no. 214, 3 white shirts' : 'e.g. delivered to Ikot Ekpene'}"></label>
      </div>
      <p class="form-error" role="alert" data-err></p>
      <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>Back to sale</button><button type="button" class="btn btn-primary btn-lg" data-complete>Complete sale</button></div>
    </div></div>` });
  const lines = $('[data-lines]', m.el)!, quick = $('[data-quick]', m.el)!, summary = $('[data-summary]', m.el)!, err = $('[data-err]', m.el)!;

  const draw = () => {
    $$<HTMLElement>('[data-method]', m.el).forEach((b) => b.classList.toggle('is-on', pays.some((p) => p.method === b.dataset.method)));
    lines.innerHTML = pays.map((p, i) => `<div class="pay-line">
      <span class="pay-method">${esc(p.method)}</span>
      <span class="input-affix"><span class="affix">${esc(store.settings.currency_symbol || '₦')}</span><input data-amt="${i}" inputmode="decimal" value="${nairaInput(p.amount)}" aria-label="${esc(p.method)} amount"></span>
      ${/cash/i.test(p.method) ? '<span></span>' : `<input data-ref="${i}" value="${esc(p.reference)}" placeholder="Reference (optional)" maxlength="80" aria-label="Payment reference">`}
      <button type="button" class="icon-btn" data-drop="${i}" aria-label="Remove ${esc(p.method)} payment">${icon('close', 16)}</button>
    </div>`).join('') || `<p class="muted small">No payment yet. Pick a method above, or record the whole amount as owed by a named ${esc(label.toLowerCase())}.</p>`;
    drawExtras();
  };
  // Summary and quick-cash buttons redraw without touching the amount inputs being edited.
  const drawExtras = () => {
    drawSummary();
    const cash = pays.find((p) => /cash/i.test(p.method));
    if (cash) {
      const others = pays.filter((p) => p !== cash).reduce((a, p) => a + p.amount, 0);
      const need = Math.max(0, t.total - others);
      // Exact amount, then the next round figures a customer is likely to hand over.
      const steps = [50000, 100000, 500000, 1000000, 2000000, 5000000, 10000000];
      const options = [need, ...steps.map((s) => Math.ceil(need / s) * s)].filter((v, i, a) => v > 0 && a.indexOf(v) === i).slice(0, 5);
      quick.innerHTML = `<span class="field-label">Cash received</span><div class="quick-row">${options.map((v, i) => `<button type="button" class="chip" data-cash="${v}">${i === 0 ? 'Exact ' : ''}${money(v, { whole: true })}</button>`).join('')}</div>`;
    } else quick.innerHTML = '';
  };
  const drawSummary = () => {
    const paid = pays.reduce((a, p) => a + p.amount, 0);
    const change = Math.max(0, paid - t.total);
    const balance = Math.max(0, t.total - paid);
    summary.innerHTML = `<div class="sum-row"><span>Paid</span><b>${money(paid)}</b></div>
      ${change ? `<div class="sum-row sum-change"><span>Change to give</span><b>${money(change)}</b></div>` : ''}
      ${balance ? `<div class="sum-row sum-balance"><span>Balance owed</span><b>${money(balance)}</b></div>
        ${customer ? `<p class="small muted">${esc(customer.name)} will owe ${money(balance)}. Record later payments from Receipts.</p>`
          : `<div class="owed-box"><p class="small">A part-paid or unpaid sale needs a ${esc(label.toLowerCase())} name so the balance can be followed up.</p><button type="button" class="btn btn-ghost btn-sm" data-cust>${icon('people', 15)} Add ${esc(label.toLowerCase())}</button></div>`}` : ''}`;
    const btn = $<HTMLButtonElement>('[data-complete]', m.el)!;
    btn.innerHTML = balance && paid ? 'Complete part-paid sale' : balance ? 'Record sale as owed' : 'Complete sale';
  };
  draw();

  m.el.addEventListener('click', (e) => {
    const el = e.target as HTMLElement;
    const method = el.closest<HTMLElement>('[data-method]')?.dataset.method;
    if (method) {
      // Until the cashier edits it, the pre-filled line simply switches to the method clicked.
      if (pristine && pays.length === 1 && pays[0].method !== method) { pays[0].method = method; pays[0].reference = ''; draw(); return; }
      pristine = false;
      const existing = pays.find((p) => p.method === method);
      const paid = pays.reduce((a, p) => a + p.amount, 0);
      if (!existing) pays.push({ method, amount: Math.max(0, t.total - paid), reference: '' });
      draw();
      ($(`[data-amt="${pays.findIndex((p) => p.method === method)}"]`, m.el) as HTMLInputElement | null)?.select();
      return;
    }
    const drop = el.closest<HTMLElement>('[data-drop]');
    if (drop) { pristine = false; pays.splice(Number(drop.dataset.drop), 1); draw(); return; }
    const cash = el.closest<HTMLElement>('[data-cash]');
    if (cash) {
      const i = pays.findIndex((x) => /cash/i.test(x.method));
      if (i >= 0) { pristine = false; pays[i].amount = Number(cash.dataset.cash); ($(`[data-amt="${i}"]`, m.el) as HTMLInputElement).value = nairaInput(pays[i].amount); drawExtras(); }
      return;
    }
    if (el.closest('[data-cust]')) { pickCustomer(drawSummary); return; }
    const complete = el.closest<HTMLButtonElement>('[data-complete]');
    if (complete) submit(complete);
  });
  m.el.addEventListener('input', (e) => {
    const el = e.target as HTMLInputElement;
    if (el.dataset.amt !== undefined) { pristine = false; pays[Number(el.dataset.amt)].amount = toKobo(el.value); drawSummary(); }
    if (el.dataset.ref !== undefined) pays[Number(el.dataset.ref)].reference = el.value;
  });
  m.el.addEventListener('change', (e) => { if ((e.target as HTMLElement).dataset.amt !== undefined) drawExtras(); });

  const submit = (btn: HTMLButtonElement) => withBusy(btn, async () => {
    err.textContent = '';
    const paid = pays.reduce((a, p) => a + p.amount, 0);
    if (paid < t.total && !customer) { err.textContent = `Add a ${label.toLowerCase()} for a sale that is not fully paid.`; return; }
    const r = await call<Receipt>('create_receipt', { input: {
      customer_id: customer?.id ?? null, customer_name: customer?.name ?? '', customer_phone: customer?.phone ?? '', customer_email: customer?.email ?? '',
      discount_kobo: t.discount, note: ($('[data-note]', m.el) as HTMLInputElement).value, collection_date: ($('[data-collection]', m.el) as HTMLInputElement | null)?.value ?? '',
      items: cart.map((l) => ({ product_id: l.product.id, quantity: l.qty, unit_price_kobo: l.override, sell_as: l.sellAs === 'pack' ? 'pack' : null })),
      payments: pays.filter((p) => p.amount > 0).map((p) => ({ method: p.method, amount_kobo: p.amount, reference: p.reference })),
    } }).catch((e) => { err.textContent = errText(e); return null; });
    if (!r) return;
    cart = []; discount = 0; customer = null;
    m.close();
    await store.loadProducts();
    after();
    showReceipt(r, { fresh: true });
  });
}

/** Receipt preview with print and email actions. Shared with the Receipts page. */
export function showReceipt(r: Receipt, opts: { fresh?: boolean; onChange?: () => void } = {}) {
  const m = modal({ title: opts.fresh ? 'Sale complete' : r.receipt_no, size: 'lg', body: `<div class="receipt-view">
    <div class="paper-wrap">${receiptMarkup(r, { width: store.settings.receipt_width_mm === 'a4' ? '80' : store.settings.receipt_width_mm })}</div>
    <div class="stack receipt-side">
      ${opts.fresh ? `<div class="done-mark">${icon('check', 22)}<div><b>${money(r.total_kobo)}</b><span>${r.change_kobo ? `Give ${money(r.change_kobo)} change` : r.balance_kobo ? `${money(r.balance_kobo)} still owed` : 'Paid in full'}</span></div></div>` : ''}
      <button class="btn btn-primary btn-lg btn-block" data-print>${icon('print', 18)} Print receipt</button>
      <button class="btn btn-ghost btn-block" data-wa>${icon('chat', 17)} Send on WhatsApp</button>
      <form class="stack email-form" data-email>
        <label class="field"><span>Email the receipt</span><span class="input-affix"><input type="email" name="to" placeholder="customer@example.com" value="${esc(r.customer_email)}" required maxlength="254"><button class="affix-btn" type="submit">${icon('mail', 16)} Send</button></span></label>
      </form>
      ${opts.fresh ? `<button class="btn btn-ghost btn-block" data-close>${icon('plus', 16)} Start next sale</button>` : ''}
    </div></div>` });
  $('[data-print]', m.el)!.addEventListener('click', () => printReceipt(r));
  $('[data-wa]', m.el)!.addEventListener('click', () => shareReceipt(r, () => shareWhatsApp(r.customer_phone, receiptText(r))));
  const form = $<HTMLFormElement>('[data-email]', m.el)!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const to = (form.elements.namedItem('to') as HTMLInputElement).value.trim();
    withBusy(form.querySelector('button'), async () => {
      const msg = await call<string>('send_receipt_email', { receiptId: r.id, recipient: to });
      toast(msg);
      opts.onChange?.();
    });
  });
  if (opts.fresh) window.setTimeout(() => $<HTMLButtonElement>('[data-print]', m.el)?.focus(), 60);
  return m;
}
