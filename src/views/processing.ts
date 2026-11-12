import { auth, call } from '../api';
import { exportCsv, nairaPlain } from '../lib/csv';
import { $, confirmDialog, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { dateLabel, isoDate, money, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { t, tl } from '../lib/i18n';
import { PROCESS_TEMPLATES, PROCESS_TYPES } from '../lib/content/farm';
import { store } from '../store';
import type { Farm, ProcBatch, ProcInput, ProcOutput } from '../types';
import type { ViewCtx } from './shell';

let showClosed = false;
const cur = () => esc(store.settings.currency_symbol || '₦');
const OUT_LABEL = { main: 'Main product', byproduct: 'By-product', waste: 'Waste' } as const;

/** Processing batches: raw materials and costs in, products out, cost per unit. */
export async function render(ctx: ViewCtx) {
  const manager = auth.can('manager');
  ctx.header(t('nav.processing'), 'Palm oil, garri, flour and feed as costed batches. Outputs go into stock at their real cost.', manager ? `<button class="btn btn-primary" data-new>${icon('plus', 16)} New batch</button>` : '');
  ctx.main.innerHTML = `<div class="toolbar"><label class="check inline"><input type="checkbox" data-closed ${showClosed ? 'checked' : ''}><span>${esc(t('common.show_closed'))}</span></label></div><div data-list></div>`;
  const list = $('[data-list]', ctx.main)!;
  const load = async () => {
    const rows = (await call<ProcBatch[]>('list_processing_batches', { includeClosed: showClosed })).filter((b) => store.inFarm(b.farm_id));
    list.innerHTML = rows.length ? `<div class="flock-grid">${rows.map((b) => `<button class="flock-card proc-card ${b.status === 'closed' ? 'is-closed' : ''}" data-open="${b.id}">
        <span class="flock-top"><b>${esc(b.batch_no)}</b><span>${b.is_demo ? `<span class="pill pill-warn">${esc(t('common.demo'))}</span> ` : ''}<span class="pill ${b.status === 'open' ? 'pill-ok' : 'pill-mute'}">${b.status === 'open' ? esc(b.stage || 'Open') : 'Closed'}</span></span></span>
        <span class="muted small">${esc(tl('proc', b.process_type))}${b.title ? ` · ${esc(b.title)}` : ''} · started ${esc(dateLabel(b.start_date))}</span>
        <span class="flock-figs">
          <span><small>Raw material</small><b>${b.raw_qty ? `${qty(b.raw_qty)} ${esc(b.raw_unit)}` : '—'}</b></span>
          ${auth.can('manager') ? `<span><small>Batch cost</small><b>${money(b.total_cost_kobo, { whole: true })}</b></span>` : ''}
          <span><small>Outputs</small><b>${b.output_count}</b></span>
          ${auth.can('manager') ? `<span><small>Profit on sold</small><b class="${b.profit_on_sold_kobo < 0 ? 'bad-text' : 'ok-text'}">${b.revenue_kobo ? money(b.profit_on_sold_kobo, { whole: true }) : '—'}</b></span>` : ''}
        </span></button>`).join('')}</div>`
      : emptyState('No processing batches', 'Start a batch each time you process: record what went in and what it cost, then what came out.', manager ? `<button class="btn btn-primary" data-new-inline>${icon('plus', 16)} New batch</button>` : '');
  };
  await load();
  ctx.main.addEventListener('change', (e) => { if ((e.target as HTMLElement).matches('[data-closed]')) { showClosed = (e.target as HTMLInputElement).checked; void load(); } });
  ctx.main.addEventListener('click', (e) => {
    const el = e.target as HTMLElement;
    const o = el.closest<HTMLElement>('[data-open]');
    if (o) void openBatch(Number(o.dataset.open), load);
    if (el.closest('[data-new-inline]')) void batchForm(null, load);
  });
  $('[data-new]')?.addEventListener('click', () => void batchForm(null, load));
  if (typeof ctx.params.open === 'number') void openBatch(ctx.params.open, load);
}

async function batchForm(b: ProcBatch | null, done: (id?: number) => void) {
  const farms = await call<Farm[]>('list_farms').catch(() => [] as Farm[]);
  const type = b?.process_type ?? 'palm_oil';
  const m = modal({ title: b ? `Edit ${b.batch_no}` : 'New processing batch', size: 'md', body: `<form class="stack">
    <div class="grid-2"><label class="field"><span>What are you processing?</span><select name="process_type">${PROCESS_TYPES.map((p) => `<option value="${p}" ${p === type ? 'selected' : ''}>${esc(tl('proc', p))}</option>`).join('')}</select></label>
      <label class="field"><span>Batch number</span><input name="batch_no" required maxlength="30" value="${esc(b?.batch_no ?? `PB-${isoDate(new Date()).replace(/-/g, '').slice(2)}`)}"></label></div>
    <div class="grid-2"><label class="field"><span>Start date</span><input type="date" name="start_date" required value="${esc(b?.start_date ?? isoDate(new Date()))}"></label>
      <label class="field"><span>Farm</span><select name="farm_id"><option value="">—</option>${farms.map((f) => `<option value="${f.id}" ${b?.farm_id === f.id ? 'selected' : ''}>${esc(f.name)}</option>`).join('')}</select></label></div>
    <label class="field"><span>Title <em class="muted">(${esc(t('common.optional'))})</em></span><input name="title" maxlength="120" value="${esc(b?.title ?? '')}" placeholder="e.g. Palm fruit from Eket"></label>
    <label class="field"><span>Notes</span><textarea name="notes" rows="2" maxlength="1000">${esc(b?.notes ?? '')}</textarea></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      let id: number;
      try { id = await call<number>('save_processing_batch', { input: { id: b?.id ?? null, batch_no: v.batch_no, process_type: v.process_type, title: v.title, farm_id: v.farm_id ? Number(v.farm_id) : null, start_date: v.start_date, stage: b?.stage ?? PROCESS_TEMPLATES[v.process_type]?.stages[0] ?? '', notes: v.notes } }); }
      catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Batch saved'); m.close(); done(id);
      if (!b) void openBatch(id, () => done());
    });
  });
}

async function openBatch(id: number, onChange: () => void) {
  const { batch: b, inputs, outputs } = await call<{ batch: ProcBatch; inputs: ProcInput[]; outputs: ProcOutput[] }>('get_processing_batch', { id });
  const manager = auth.can('manager');
  const open = b.status === 'open';
  const tpl = PROCESS_TEMPLATES[b.process_type] ?? PROCESS_TEMPLATES.other;
  const stageIdx = tpl.stages.indexOf(b.stage);
  const m = modal({ title: `${b.batch_no} · ${tl('proc', b.process_type)}`, subtitle: `${b.title ? `${b.title} · ` : ''}started ${dateLabel(b.start_date)}${open ? '' : ` · closed ${dateLabel(b.end_date)}`}${b.farm_name ? ` · ${b.farm_name}` : ''}`, size: 'lg', body: `<div class="stack">
    ${b.is_demo ? `<div class="callout callout-warn"><div><b>${esc(t('common.demo_data'))}</b><span>Illustrative figures from the FarmBuddy specification. Not a fixed market rate or yield.</span></div></div>` : ''}
    <ol class="stage-line" aria-label="Stages">${tpl.stages.map((s, i) => `<li class="${i < stageIdx ? 'is-done' : i === stageIdx ? 'is-on' : ''}">${open ? `<button type="button" data-stage="${esc(s)}">${esc(s)}</button>` : esc(s)}</li>`).join('')}</ol>
    <div class="figures">${auth.can('manager') ? `<div><span>Raw material</span><b>${money(b.raw_cost_kobo)}</b><small class="muted">${b.raw_qty ? `${qty(b.raw_qty)} ${esc(b.raw_unit)}` : 'Mixed units'}</small></div>` : ''}
      ${auth.can('manager') ? `<div><span>Processing costs</span><b>${money(b.processing_cost_kobo)}</b><small class="muted">Fees, labour, transport, fuel…</small></div>` : ''}
      ${auth.can('manager') ? `<div><span>Total batch cost</span><b>${money(b.total_cost_kobo)}</b><small class="muted">${b.main_share_pct > 0 ? `Split ${b.main_share_pct.toFixed(0)}% across main products` : 'Add main products to split the cost'}</small></div>` : ''}
      ${auth.can('manager') ? `<div><span>Profit on what was sold</span><b class="${b.profit_on_sold_kobo < 0 ? 'bad-text' : 'ok-text'}">${b.revenue_kobo ? money(b.profit_on_sold_kobo) : '—'}</b><small class="muted">${b.yield_pct != null ? `Yield ${b.yield_pct.toFixed(1)}% · ` : ''}${b.waste_pct != null ? `Waste ${b.waste_pct.toFixed(1)}%` : 'Stock left is not counted as income'}</small></div>` : ''}</div>
    <div class="grid-2 proc-cols">
      <section><h3 class="section-title">In: raw materials and costs</h3>
        <table class="table compact"><tbody>${inputs.map((i) => `<tr><td>${i.kind === 'raw' ? `<b>${esc(i.name)}</b>` : esc(i.name)}${i.quantity ? `<small class="muted block">${qty(i.quantity)} ${esc(i.unit)}${i.unit_cost_kobo ? ` × ${money(i.unit_cost_kobo, { whole: true })}` : ''}${i.product_id ? ' · from stock' : ''}${i.supplier ? ` · ${esc(i.supplier)}` : ''}</small>` : ''}</td>
          <td class="num">${money(i.amount_kobo)}</td>${manager && open ? `<td class="actions-col"><button class="icon-btn" data-del="input:${i.id}" aria-label="Delete">${icon('trash', 15)}</button></td>` : ''}</tr>`).join('') || '<tr><td class="muted">Nothing recorded yet.</td></tr>'}
        <tr class="total-row"><td>Total</td><td class="num">${money(b.total_cost_kobo)}</td>${manager && open ? '<td></td>' : ''}</tr></tbody></table>
        ${manager && open ? `<div class="row-actions"><button class="btn btn-ghost btn-sm" data-input="raw">${icon('plus', 14)} Raw material</button><button class="btn btn-ghost btn-sm" data-input="cost">${icon('plus', 14)} Cost</button></div>` : ''}
      </section>
      <section><h3 class="section-title">Out: products, by-products, waste</h3>
        <table class="table compact"><tbody>${outputs.map((o) => `<tr><td><b>${esc(o.name)}</b> <small class="muted">${OUT_LABEL[o.output_type]}${o.output_type === 'main' ? ` · ${o.cost_share_pct || (outputs.filter((x) => x.output_type === 'main').length === 1 ? 100 : 0)}% of cost` : ''}</small>
            <small class="muted block">${o.product_name ? `→ ${esc(o.product_name)}${o.added_to_stock ? ' (in stock)' : ' (added at close)'}` : ''}${o.sold_qty ? ` · ${qty(o.sold_qty)} sold${o.buyer ? ` to ${esc(o.buyer)}` : ''} for ${money(o.sale_amount_kobo, { whole: true })}` : ''}</small></td>
          <td class="num">${qty(o.quantity)} ${esc(o.unit)}${o.unit_cost_kobo ? `<small class="muted block">${money(o.unit_cost_kobo)}/${esc(o.unit)}</small>` : ''}</td>
          ${manager && open ? `<td class="actions-col"><button class="icon-btn" data-del="output:${o.id}" aria-label="Delete">${icon('trash', 15)}</button></td>` : ''}</tr>`).join('') || '<tr><td class="muted">Record what came out when processing ends.</td></tr>'}</tbody></table>
        ${open ? `<div class="row-actions"><button class="btn btn-ghost btn-sm" data-output>${icon('plus', 14)} Output</button></div>` : ''}
      </section>
    </div>
    <p class="muted small">Cost per unit = total batch cost × the product’s share ÷ quantity produced. FarmBuddy never assumes a fixed yield.</p>
    <div class="form-actions">${manager ? `<button class="btn btn-ghost" data-edit>${icon('edit', 16)} Edit</button><button class="btn btn-ghost" data-export>${icon('download', 16)} Export</button>
      <button class="btn ${open ? 'btn-primary' : 'btn-ghost'}" data-toggle>${open ? `${icon('check', 16)} Close batch` : 'Reopen batch'}</button>` : ''}</div>
  </div>` });
  const refresh = () => { m.close(); onChange(); void openBatch(id, onChange); };
  m.el.addEventListener('click', async (e) => {
    const el = e.target as HTMLElement;
    const st = el.closest<HTMLElement>('[data-stage]');
    if (st) { try { await call('set_processing_stage', { id, stage: st.dataset.stage }); refresh(); } catch (x) { toast(errText(x), 'error'); } return; }
    const inp = el.closest<HTMLElement>('[data-input]');
    if (inp) void inputForm(b, inp.dataset.input as 'raw' | 'cost', refresh);
    if (el.closest('[data-output]')) void outputForm(b, outputs, refresh);
    if (el.closest('[data-edit]')) { m.close(); void batchForm(b, () => { onChange(); void openBatch(id, onChange); }); }
    const del = el.closest<HTMLElement>('[data-del]');
    if (del) {
      const [kind, eid] = del.dataset.del!.split(':');
      if (await confirmDialog({ title: 'Delete this entry?', message: kind === 'input' ? 'Raw material taken from stock is not put back automatically.' : 'The batch figures will be recalculated.', confirm: 'Delete', danger: true }) === null) return;
      try { await call('delete_processing_entry', { kind, id: Number(eid) }); toast('Deleted'); refresh(); } catch (x) { toast(errText(x), 'error'); }
    }
    if (el.closest('[data-toggle]')) {
      if (open) {
        if (await confirmDialog({ title: `Close batch ${b.batch_no}?`, message: 'Outputs linked to stock items are added to stock at their cost per unit. Check the quantities first.', confirm: 'Close batch' }) === null) return;
        try { await call('close_processing_batch', { id, endDate: isoDate(new Date()) }); toast('Batch closed and outputs added to stock'); refresh(); } catch (x) { toast(errText(x), 'error'); }
      } else {
        try { await call('close_processing_batch', { id, endDate: '', reopen: true }); toast('Batch reopened'); refresh(); } catch (x) { toast(errText(x), 'error'); }
      }
    }
    if (el.closest('[data-export]')) exportCsv(`processing-${b.batch_no}.csv`, ['Section', 'Name', 'Type', 'Quantity', 'Unit', 'Unit cost', 'Amount'], [
      ...inputs.map((i) => ['In', i.name, i.kind === 'raw' ? 'Raw material' : 'Cost', i.quantity, i.unit, nairaPlain(i.unit_cost_kobo), nairaPlain(i.amount_kobo)]),
      ...outputs.map((o) => ['Out', o.name, OUT_LABEL[o.output_type], o.quantity, o.unit, nairaPlain(o.unit_cost_kobo), nairaPlain(o.sale_amount_kobo)]),
    ]);
  });
}

async function inputForm(b: ProcBatch, kind: 'raw' | 'cost', done: () => void) {
  const tpl = PROCESS_TEMPLATES[b.process_type] ?? PROCESS_TEMPLATES.other;
  if (kind === 'raw') await store.loadProducts();
  const stock = store.products.filter((p) => p.active && p.track_stock);
  const m = modal({ title: kind === 'raw' ? 'Add raw material' : 'Add a processing cost', subtitle: b.batch_no, size: 'md', body: `<form class="stack">
    <label class="field"><span>Date</span><input type="date" name="date" required value="${isoDate(new Date())}"></label>
    ${kind === 'raw' ? `
      <label class="field"><span>Raw material</span><input name="name" required maxlength="80" value="${esc(tpl.raw)}"></label>
      <label class="field"><span>Take from stock <em class="muted">(${esc(t('common.optional'))})</em></span><select name="product_id"><option value="">No, bought for this batch</option>${stock.map((p) => `<option value="${p.id}">${esc(p.name)}: ${qty(p.stock_qty)} ${esc(p.unit)}</option>`).join('')}</select><small>Stock is deducted and costed at its average cost.</small></label>
      <div class="grid-3"><label class="field"><span>Quantity</span><input name="quantity" inputmode="decimal" required></label>
        <label class="field" data-bought><span>Unit</span><input name="unit" maxlength="30" value="${esc(tpl.rawUnit)}"></label>
        <label class="field" data-bought><span>Price per unit</span><span class="input-affix"><span class="affix">${cur()}</span><input name="unit_cost" inputmode="decimal"></span></label></div>
      <label class="field" data-bought><span>Supplier</span><input name="supplier" maxlength="120"></label>`
    : `
      <label class="field"><span>Cost</span><input name="name" required maxlength="80" list="cost-cats" value="${esc(tpl.costs[0] ?? '')}"><datalist id="cost-cats">${tpl.costs.map((c) => `<option value="${esc(c)}">`).join('')}</datalist></label>
      <div class="grid-3"><label class="field"><span>Quantity <em class="muted">(${esc(t('common.optional'))})</em></span><input name="quantity" inputmode="decimal" value="0"></label>
        <label class="field"><span>Price per unit</span><span class="input-affix"><span class="affix">${cur()}</span><input name="unit_cost" inputmode="decimal"></span></label>
        <label class="field"><span>Or total amount</span><span class="input-affix"><span class="affix">${cur()}</span><input name="amount" inputmode="decimal"></span></label></div>
      <p class="muted small">Example: milling fee 100 bunches × ₦500, or transport ₦10,000 in total.</p>`}
    <label class="field"><span>Notes</span><input name="notes" maxlength="300"></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  const sync = () => { const fromStock = !!(form.elements.namedItem('product_id') as HTMLSelectElement | null)?.value; form.querySelectorAll<HTMLElement>('[data-bought]').forEach((x) => { x.hidden = fromStock; }); };
  form.addEventListener('change', sync);
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('add_processing_input', { input: { batch_id: b.id, entry_date: v.date, kind, name: v.name, quantity: Number((v.quantity ?? '').replace(/,/g, '')) || 0, unit: v.unit ?? '',
          unit_cost_kobo: toKobo(v.unit_cost ?? ''), amount_kobo: toKobo(v.amount ?? ''), product_id: v.product_id ? Number(v.product_id) : null, supplier: v.supplier ?? '', notes: v.notes } });
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast(t('common.saved')); m.close(); done();
    });
  });
}

async function outputForm(b: ProcBatch, existing: ProcOutput[], done: () => void) {
  const tpl = PROCESS_TEMPLATES[b.process_type] ?? PROCESS_TEMPLATES.other;
  const manager = auth.can('manager');
  await store.loadProducts();
  const items = store.products.filter((p) => p.active && p.track_stock && p.kind !== 'service');
  const used = existing.filter((o) => o.output_type === 'main').reduce((a, o) => a + o.cost_share_pct, 0);
  const next = tpl.outputs.find((o) => !existing.some((x) => x.name.toLowerCase() === o.name.toLowerCase())) ?? { type: 'main', name: '', unit: 'kg' };
  const m = modal({ title: 'Record an output', subtitle: b.batch_no, size: 'md', body: `<form class="stack">
    <div class="grid-2"><label class="field"><span>Type</span><select name="output_type">${(['main', 'byproduct', 'waste'] as const).map((x) => `<option value="${x}" ${next.type === x ? 'selected' : ''}>${OUT_LABEL[x]}</option>`).join('')}</select></label>
      <label class="field"><span>Name</span><input name="name" required maxlength="80" list="out-names" value="${esc(next.name)}"><datalist id="out-names">${tpl.outputs.map((o) => `<option value="${esc(o.name)}">`).join('')}</datalist></label></div>
    <div class="grid-3"><label class="field"><span>Quantity produced</span><input name="quantity" inputmode="decimal" required autofocus></label>
      <label class="field"><span>Unit</span><input name="unit" required maxlength="30" value="${esc(next.unit)}"></label>
      ${manager ? `<label class="field" data-main><span>Share of batch cost (%)</span><input name="share" inputmode="decimal" value="${Math.max(0, 100 - used)}"><small>${used ? `${used}% already used.` : 'All main products together: 100%.'}</small></label>` : ''}</div>
    ${manager ? `<label class="field" data-stockable><span>Add to stock as <em class="muted">(${esc(t('common.optional'))})</em></span><select name="product_id"><option value="">Do not add to stock</option>${items.map((p) => `<option value="${p.id}" data-unit="${esc(p.unit)}">${esc(p.name)} (${esc(p.unit)})</option>`).join('')}</select><small>Added when you close the batch, at its cost per unit.</small></label>
    <div class="grid-3" data-stockable><label class="field"><span>Sold straight from the batch</span><input name="sold" inputmode="decimal" value="0"></label>
      <label class="field"><span>Amount received</span><span class="input-affix"><span class="affix">${cur()}</span><input name="amount" inputmode="decimal"></span></label>
      <label class="field"><span>Buyer</span><input name="buyer" maxlength="120"></label></div>` : ''}
    <label class="field"><span>Notes</span><input name="notes" maxlength="300"></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  const sync = () => {
    const type = (form.elements.namedItem('output_type') as HTMLSelectElement).value;
    form.querySelectorAll<HTMLElement>('[data-main]').forEach((x) => { x.hidden = type !== 'main'; });
    form.querySelectorAll<HTMLElement>('[data-stockable]').forEach((x) => { x.hidden = type === 'waste'; });
  };
  form.addEventListener('change', (e) => {
    sync();
    const sel = e.target as HTMLSelectElement;
    if (sel.name === 'product_id' && sel.selectedOptions[0]?.dataset.unit) (form.elements.namedItem('unit') as HTMLInputElement).value = sel.selectedOptions[0].dataset.unit!;
  });
  sync();
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const type = v.output_type;
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('add_processing_output', { input: { batch_id: b.id, entry_date: isoDate(new Date()), output_type: type, name: v.name, quantity: Number((v.quantity ?? '').replace(/,/g, '')) || 0, unit: v.unit,
          cost_share_pct: type === 'main' ? Number(v.share ?? 0) || 0 : 0, product_id: type !== 'waste' && v.product_id ? Number(v.product_id) : null,
          sold_qty: type !== 'waste' ? Number(v.sold ?? 0) || 0 : 0, sale_amount_kobo: type !== 'waste' ? toKobo(v.amount ?? '') : 0, buyer: v.buyer ?? '', notes: v.notes } });
      } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast(t('common.saved')); m.close(); done();
    });
  });
}
