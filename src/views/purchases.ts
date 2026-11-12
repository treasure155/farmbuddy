import { t } from '../lib/i18n';
import { call } from '../api';
import { exportCsv, nairaPlain } from '../lib/csv';
import { $, $$, confirmDialog, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { dateLabel, isoDate, money, nairaInput, qty, relativeDay, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { store } from '../store';
import type { Product, Purchase, PurchaseSummary, Supplier } from '../types';
import type { ViewCtx } from './shell';

type Tab = 'purchases' | 'suppliers';
let tab: Tab = 'purchases';
const t0 = new Date();
let range = { from: isoDate(new Date(t0.getFullYear(), t0.getMonth() - 2, 1)), to: isoDate(t0), owing: false };
const cur = () => esc(store.settings.currency_symbol || '₦');

export async function render(ctx: ViewCtx) {
  ctx.header(t('page.purchases.title', {}, 'Purchases'), t('page.purchases.subtitle', {}, 'Goods received from suppliers. Stock and cost prices update automatically.'), `<button class="btn btn-primary" data-new>${icon('plus', 16)} Receive goods</button>`);
  ctx.main.innerHTML = `<div class="tabs" role="tablist">${(['purchases', 'suppliers'] as Tab[]).map((t) => `<button role="tab" class="tab" data-tab="${t}">${t === 'purchases' ? 'Purchases' : 'Suppliers'}</button>`).join('')}</div><div data-pane></div>`;
  const pane = $('[data-pane]', ctx.main)!;
  const show = async () => {
    $$('[data-tab]', ctx.main).forEach((b) => { b.classList.toggle('is-on', b.dataset.tab === tab); b.setAttribute('aria-selected', String(b.dataset.tab === tab)); });
    if (tab === 'purchases') await purchasesPane(pane); else await suppliersPane(pane);
  };
  ctx.main.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-tab]');
    if (b) { tab = b.dataset.tab as Tab; show().catch((x) => toast(errText(x), 'error')); }
  });
  $('[data-new]')!.addEventListener('click', () => purchaseForm(() => show()));
  await show();
}

async function purchasesPane(pane: HTMLElement) {
  pane.innerHTML = `<div class="toolbar">
      <label class="date-field"><span>From</span><input type="date" data-r="from" value="${range.from}"></label>
      <label class="date-field"><span>To</span><input type="date" data-r="to" value="${range.to}"></label>
      <label class="check inline"><input type="checkbox" data-r="owing" ${range.owing ? 'checked' : ''}><span>Only unpaid</span></label>
      <button class="btn btn-ghost push-right" data-pexport>${icon('download', 16)} Export</button>
    </div><section class="panel flush" data-list></section>`;
  let rows: PurchaseSummary[] = [];
  const load = async () => {
    rows = await call<PurchaseSummary[]>('list_purchases', { from: range.from, to: range.to, supplierId: null, owingOnly: range.owing });
    const total = rows.reduce((a, r) => a + r.total_kobo, 0), owed = rows.reduce((a, r) => a + r.balance_kobo, 0);
    $('[data-list]', pane)!.innerHTML = rows.length ? `<div class="table-meta"><span>${rows.length} purchase${rows.length === 1 ? '' : 's'}</span><span>Value ${money(total)}</span>${owed ? `<span class="warn-text">You owe ${money(owed)}</span>` : ''}</div>
      <div class="table-scroll"><table class="table table-click"><thead><tr><th>Purchase</th><th>When</th><th>Supplier</th><th>Invoice</th><th class="num">Items</th><th class="num">Total</th><th class="num">Unpaid</th></tr></thead><tbody>
      ${rows.map((r) => `<tr data-open="${r.id}" tabindex="0"><td class="mono-ish">${esc(r.purchase_no)}</td><td>${esc(relativeDay(r.created_at))}</td><td>${esc(r.supplier_name || '—')}</td><td>${esc(r.invoice_ref || '—')}</td><td class="num">${r.item_count}</td><td class="num">${money(r.total_kobo)}</td><td class="num">${r.balance_kobo ? `<b class="warn-text">${money(r.balance_kobo)}</b>` : '<span class="muted">—</span>'}</td></tr>`).join('')}
      </tbody></table></div>`
      : emptyState('No purchases in this period', 'Use “Receive goods” when stock arrives from a supplier. Stock goes up and the average cost is updated.');
  };
  await load();
  pane.onchange = (e) => {
    const el = (e.target as HTMLElement).closest<HTMLInputElement>('[data-r]');
    if (!el) return;
    if (el.dataset.r === 'owing') range.owing = el.checked; else (range as Record<string, unknown>)[el.dataset.r!] = el.value;
    load().catch((x) => toast(errText(x), 'error'));
  };
  pane.onclick = async (e) => {
    const t = e.target as HTMLElement;
    const row = t.closest<HTMLElement>('[data-open]');
    if (row) openPurchase(await call<Purchase>('get_purchase', { id: Number(row.dataset.open) }), load);
    if (t.closest('[data-pexport]')) exportCsv(`purchases-${range.from}-to-${range.to}.csv`, ['Purchase', 'Date', 'Supplier', 'Invoice', 'Items', 'Total', 'Paid', 'Unpaid'],
      rows.map((r) => [r.purchase_no, r.created_at, r.supplier_name, r.invoice_ref, r.item_count, nairaPlain(r.total_kobo), nairaPlain(r.amount_paid_kobo), nairaPlain(r.balance_kobo)]));
  };
}

function openPurchase(p: Purchase, onChange: () => void) {
  const methods = store.paymentMethods.filter((x) => x.active).map((x) => x.name);
  const m = modal({ title: p.purchase_no, subtitle: `${relativeDay(p.created_at)} · ${p.user_name}`, size: 'drawer', body: `
    <dl class="kv"><div><dt>Supplier</dt><dd>${esc(p.supplier_name || '—')}</dd></div><div><dt>Invoice</dt><dd>${esc(p.invoice_ref || '—')}</dd></div>${p.note ? `<div><dt>Note</dt><dd>${esc(p.note)}</dd></div>` : ''}</dl>
    <table class="table compact"><thead><tr><th>Item</th><th class="num">Qty</th><th class="num">Cost</th><th class="num">Total</th></tr></thead><tbody>
      ${p.lines.map((l) => `<tr><td>${esc(l.product_name)}${l.expiry_date ? `<small class="muted block">Batch ${esc(l.batch_no || '—')}, expires ${esc(dateLabel(l.expiry_date))}</small>` : ''}</td><td class="num">${qty(l.quantity)} <small class="muted">${esc(l.unit)}</small></td><td class="num">${money(l.unit_cost_kobo)}</td><td class="num">${money(l.line_total_kobo)}</td></tr>`).join('')}
    </tbody><tfoot><tr><th colspan="3">Total</th><th class="num">${money(p.total_kobo)}</th></tr></tfoot></table>
    <h3 class="section-title">Payments to supplier</h3>
    <ul class="history">${p.payments.map((x) => `<li><span>${esc(x.method || 'Payment')}${x.reference ? ` <small class="muted">(${esc(x.reference)})</small>` : ''}<small class="muted block">${esc(relativeDay(x.created_at))} by ${esc(x.user_name)}</small></span><b>${money(x.amount_kobo)}</b></li>`).join('') || '<li class="muted">Nothing paid yet</li>'}</ul>
    ${p.balance_kobo > 0 ? `<form class="stack sub-panel" data-pay>
      <b class="warn-text">${money(p.balance_kobo)} still owed to the supplier</b>
      <div class="grid-3"><label class="field"><span>Amount</span><span class="input-affix"><span class="affix">${cur()}</span><input name="amount" inputmode="decimal" value="${nairaInput(p.balance_kobo)}" required></span></label>
      <label class="field"><span>Method</span><input name="method" list="sp-methods" value="${esc(methods.find((x) => /transfer/i.test(x)) ?? methods[0] ?? 'Cash')}"><datalist id="sp-methods">${methods.map((x) => `<option value="${esc(x)}">`).join('')}</datalist></label>
      <label class="field"><span>Reference</span><input name="reference" maxlength="80"></label></div>
      <div><button class="btn btn-primary">Record payment</button></div></form>` : '<p class="ok-text">Fully paid.</p>'}` });
  $<HTMLFormElement>('[data-pay]', m.el)?.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(e.currentTarget as HTMLFormElement);
    withBusy($<HTMLButtonElement>('[data-pay] .btn-primary', m.el), async () => {
      const next = await call<Purchase>('record_supplier_payment', { purchaseId: p.id, amountKobo: toKobo(v.amount), method: v.method, reference: v.reference });
      toast('Payment recorded'); m.close(); onChange(); openPurchase(next, onChange);
    });
  });
}

interface PLine { product: Product | null; qty: number; cost: number; asPack: boolean; batch: string; expiry: string }

export async function purchaseForm(done: () => void, preset?: Product) {
  await store.loadProducts();
  const suppliers = await call<Supplier[]>('list_suppliers');
  const items = store.products.filter((p) => p.active && p.track_stock).sort((a, b) => a.name.localeCompare(b.name));
  const methods = store.paymentMethods.filter((x) => x.active).map((x) => x.name);
  const lines: PLine[] = [{ product: preset ?? null, qty: 1, cost: preset?.cost_price_kobo ?? 0, asPack: false, batch: '', expiry: '' }];
  const m = modal({ title: 'Receive goods', subtitle: 'Record a delivery from a supplier', size: 'lg', body: `<form class="stack" novalidate>
    <div class="grid-3">
      <label class="field span-2"><span>Supplier</span><input name="supplier" list="sup-list" maxlength="120" placeholder="Choose or type a new supplier"><datalist id="sup-list">${suppliers.map((s) => `<option value="${esc(s.name)}">`).join('')}</datalist></label>
      <label class="field"><span>Invoice / waybill no.</span><input name="invoice" maxlength="80"></label>
    </div>
    <div class="purchase-lines" data-lines></div>
    <div><button type="button" class="btn btn-ghost btn-sm" data-add-line>${icon('plus', 15)} Add another item</button></div>
    <div class="grid-3 purchase-foot">
      <div class="field"><span>Total</span><output class="figure-sm" data-total></output></div>
      <label class="field"><span>Amount paid now</span><span class="input-affix"><span class="affix">${cur()}</span><input name="paid" inputmode="decimal" data-paid></span><small>Leave less than the total to record what you owe.</small></label>
      <label class="field"><span>Paid by</span><input name="method" list="pm-list" value="${esc(methods.find((x) => /transfer/i.test(x)) ?? methods[0] ?? 'Cash')}"><datalist id="pm-list">${methods.map((x) => `<option value="${esc(x)}">`).join('')}</datalist></label>
    </div>
    <label class="field"><span>Note <em class="muted">(optional)</em></span><input name="note" maxlength="500"></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">Save and add to stock</button></div></form>` });
  const form = m.el.querySelector('form')!;
  const box = $('[data-lines]', form)!;
  const total = () => lines.reduce((a, l) => a + Math.round(l.cost * l.qty), 0);
  let paidTouched = false;
  const drawTotal = () => {
    $('[data-total]', form)!.textContent = money(total());
    if (!paidTouched) ($('[data-paid]', form) as HTMLInputElement).value = nairaInput(total());
  };
  const draw = () => {
    box.innerHTML = `<div class="pline pline-head"><span>Item</span><span>Quantity</span><span>Cost each</span><span>Line total</span><span></span></div>` + lines.map((l, i) => {
      const p = l.product;
      const pack = p && p.pack_unit && p.pack_size > 1;
      return `<div class="pline" data-i="${i}">
        <select data-k="product" aria-label="Item"><option value="">Choose…</option>${items.map((x) => `<option value="${x.id}" ${p?.id === x.id ? 'selected' : ''}>${esc(x.name)} (${esc(x.sku)})</option>`).join('')}</select>
        <span class="pline-qty"><input data-k="qty" inputmode="decimal" value="${qty(l.qty)}" aria-label="Quantity">
          ${pack ? `<select data-k="unit" aria-label="Unit"><option value="unit" ${l.asPack ? '' : 'selected'}>${esc(p!.unit)}</option><option value="pack" ${l.asPack ? 'selected' : ''}>${esc(p!.pack_unit)} of ${qty(p!.pack_size)}</option></select>` : `<small class="muted">${esc(p?.unit ?? '')}</small>`}</span>
        <span class="input-affix"><span class="affix">${cur()}</span><input data-k="cost" inputmode="decimal" value="${nairaInput(l.cost)}" aria-label="Cost each"></span>
        <b class="num" data-lt>${money(Math.round(l.cost * l.qty))}</b>
        <button type="button" class="icon-btn" data-drop="${i}" aria-label="Remove line" ${lines.length === 1 ? 'disabled' : ''}>${icon('close', 15)}</button>
        ${p?.track_expiry ? `<div class="pline-batch"><label class="field"><span>Batch no.</span><input data-k="batch" maxlength="40" value="${esc(l.batch)}"></label><label class="field"><span>Expiry date</span><input type="date" data-k="expiry" value="${esc(l.expiry)}" min="${isoDate(new Date())}"></label></div>` : ''}
      </div>`;
    }).join('');
    drawTotal();
  };
  draw();
  box.addEventListener('change', (e) => {
    const el = e.target as HTMLInputElement | HTMLSelectElement;
    const row = el.closest<HTMLElement>('[data-i]');
    if (!row) return;
    const l = lines[Number(row.dataset.i)];
    if (el.dataset.k === 'product') {
      l.product = items.find((x) => x.id === Number(el.value)) ?? null;
      l.asPack = false; l.cost = l.product?.cost_price_kobo ?? 0; draw();
    }
    if (el.dataset.k === 'unit' && l.product) {
      const toPack = el.value === 'pack';
      if (toPack !== l.asPack) { l.cost = Math.round(toPack ? l.cost * l.product.pack_size : l.cost / l.product.pack_size); l.asPack = toPack; draw(); }
    }
    if (el.dataset.k === 'expiry') l.expiry = el.value;
  });
  box.addEventListener('input', (e) => {
    const el = e.target as HTMLInputElement;
    const row = el.closest<HTMLElement>('[data-i]');
    if (!row) return;
    const l = lines[Number(row.dataset.i)];
    if (el.dataset.k === 'qty') l.qty = Number(el.value.replace(',', '.')) || 0;
    if (el.dataset.k === 'cost') l.cost = toKobo(el.value);
    if (el.dataset.k === 'batch') l.batch = el.value;
    $('[data-lt]', row)!.textContent = money(Math.round(l.cost * l.qty));
    drawTotal();
  });
  box.addEventListener('click', (e) => {
    const d = (e.target as HTMLElement).closest<HTMLElement>('[data-drop]');
    if (d) { lines.splice(Number(d.dataset.drop), 1); draw(); }
  });
  $('[data-add-line]', form)!.addEventListener('click', () => { lines.push({ product: null, qty: 1, cost: 0, asPack: false, batch: '', expiry: '' }); draw(); ($$<HTMLSelectElement>('[data-k="product"]', box).pop())?.focus(); });
  $('[data-paid]', form)!.addEventListener('input', () => { paidTouched = true; });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const err = $('[data-err]', form)!;
    err.textContent = '';
    const used = lines.filter((l) => l.product);
    if (!used.length) { err.textContent = 'Choose at least one item.'; return; }
    if (used.some((l) => !(l.qty > 0))) { err.textContent = 'Every quantity must be more than zero.'; return; }
    const sup = suppliers.find((s) => s.name.toLowerCase() === v.supplier.trim().toLowerCase());
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        let supplierId = sup?.id ?? null;
        if (!supplierId && v.supplier.trim()) supplierId = await call<number>('save_supplier', { input: { id: null, name: v.supplier.trim(), phone: '', email: '', address: '', notes: '' } });
        const p = await call<Purchase>('create_purchase', { input: {
          supplier_id: supplierId, supplier_name: v.supplier.trim(), invoice_ref: v.invoice, note: v.note,
          lines: used.map((l) => ({ product_id: l.product!.id, quantity: l.qty, unit_cost_kobo: l.cost, as_pack: l.asPack, batch_no: l.batch, expiry_date: l.expiry })),
          amount_paid_kobo: Math.min(total(), toKobo(v.paid)), payment_method: v.method,
        } });
        toast(`${p.purchase_no} saved. Stock updated.`);
        m.close(); await store.loadProducts(); done();
      } catch (x) { err.textContent = errText(x); }
    });
  });
}

async function suppliersPane(pane: HTMLElement) {
  const rows = await call<Supplier[]>('list_suppliers');
  pane.innerHTML = `<div class="toolbar"><button class="btn btn-ghost push-right" data-snew>${icon('plus', 16)} Add supplier</button></div>
    <section class="panel flush">${rows.length ? `<div class="table-scroll"><table class="table"><thead><tr><th>Supplier</th><th>Contact</th><th class="num">Purchases</th><th class="num">Total bought</th><th class="num">You owe</th><th class="actions-col"><span class="sr-only">Actions</span></th></tr></thead><tbody>
      ${rows.map((s) => `<tr><td><b>${esc(s.name)}</b>${s.address ? `<small class="muted block">${esc(s.address)}</small>` : ''}</td><td>${esc(s.phone || '—')}${s.email ? `<small class="muted block">${esc(s.email)}</small>` : ''}</td><td class="num">${s.purchase_count}</td><td class="num">${money(s.total_bought_kobo)}</td><td class="num">${s.owed_kobo ? `<b class="warn-text">${money(s.owed_kobo)}</b>` : '<span class="muted">—</span>'}</td>
        <td class="actions-col"><button class="icon-btn" data-sedit="${s.id}" aria-label="Edit ${esc(s.name)}">${icon('edit', 16)}</button></td></tr>`).join('')}</tbody></table></div>`
      : emptyState('No suppliers yet', 'Suppliers are added automatically when you receive goods, or add them here.')}</section>`;
  pane.onclick = (e) => {
    const t = e.target as HTMLElement;
    const ed = t.closest<HTMLElement>('[data-sedit]');
    if (ed) supplierForm(rows.find((s) => s.id === Number(ed.dataset.sedit))!, () => suppliersPane(pane));
    if (t.closest('[data-snew]')) supplierForm(null, () => suppliersPane(pane));
  };
  pane.onchange = null;
}

function supplierForm(s: Supplier | null, done: () => void) {
  const m = modal({ title: s ? `Edit ${s.name}` : 'Add supplier', size: 'md', body: `<form class="stack">
    <label class="field"><span>Name</span><input name="name" required maxlength="120" value="${esc(s?.name ?? '')}" autofocus></label>
    <div class="grid-2"><label class="field"><span>Phone</span><input name="phone" maxlength="40" value="${esc(s?.phone ?? '')}"></label><label class="field"><span>Email</span><input name="email" type="email" maxlength="254" value="${esc(s?.email ?? '')}"></label></div>
    <label class="field"><span>Address</span><input name="address" maxlength="300" value="${esc(s?.address ?? '')}"></label>
    <label class="field"><span>Notes</span><textarea name="notes" rows="2" maxlength="500">${esc(s?.notes ?? '')}</textarea></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions">${s ? `<button type="button" class="btn btn-ghost btn-danger-text push-left" data-del>${icon('trash', 16)} Delete</button>` : ''}<button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('save_supplier', { input: { id: s?.id ?? null, name: v.name, phone: v.phone, email: v.email, address: v.address, notes: v.notes } }); }
      catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Supplier saved'); m.close(); done();
    });
  });
  $('[data-del]', form)?.addEventListener('click', async () => {
    if (await confirmDialog({ title: `Delete ${s!.name}?`, message: 'Old purchases keep the supplier name.', confirm: 'Delete', danger: true }) === null) return;
    try { await call('delete_supplier', { id: s!.id }); toast('Supplier deleted'); m.close(); done(); } catch (x) { toast(errText(x), 'error'); }
  });
}
