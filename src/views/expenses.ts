import { t } from '../lib/i18n';
import { call } from '../api';
import { exportCsv, nairaPlain } from '../lib/csv';
import { $, confirmDialog, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { dateLabel, isoDate, money, nairaInput, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { store } from '../store';
import type { Expense, ExpenseList } from '../types';
import type { ViewCtx } from './shell';

const today = () => new Date();
let range = { from: isoDate(new Date(today().getFullYear(), today().getMonth(), 1)), to: isoDate(today()) };

export async function render(ctx: ViewCtx) {
  ctx.header(t('page.expenses.title', {}, 'Expenses'), t('page.expenses.subtitle', {}, 'Diesel, salaries, rent and other running costs. Reports take them off your profit.'), `
    <button class="btn btn-ghost" data-export>${icon('download', 16)} Export</button>
    <button class="btn btn-primary" data-new>${icon('plus', 16)} Record expense</button>`);
  ctx.main.innerHTML = `<div class="toolbar">
      <label class="date-field"><span>From</span><input type="date" data-from value="${range.from}"></label>
      <label class="date-field"><span>To</span><input type="date" data-to value="${range.to}"></label>
    </div><div data-out></div>`;
  let data: ExpenseList | null = null;
  const out = $('[data-out]', ctx.main)!;
  const load = async () => {
    data = await call<ExpenseList>('list_expenses', { from: range.from, to: range.to });
    const d = data;
    const max = Math.max(1, ...d.by_category.map((c) => c[1]));
    out.innerHTML = `<div class="report-grid two">
      <section class="panel"><header class="panel-head"><div><h2>${money(d.total_kobo)}</h2><p class="muted small">spent from ${esc(dateLabel(range.from))} to ${esc(dateLabel(range.to))}</p></div></header>
        ${d.by_category.length ? `<ul class="share-list">${d.by_category.map(([n, v]) => `<li><div class="share-top"><span>${esc(n)}</span><b>${money(v)}</b></div><div class="share-bar"><span style="width:${Math.max(1, (v / max) * 100)}%"></span></div></li>`).join('')}</ul>` : '<p class="muted pad">No expenses recorded in this period.</p>'}
      </section>
      <section class="panel flush">${d.items.length ? `<div class="table-scroll"><table class="table">
        <thead><tr><th>Date</th><th>Category</th><th>Details</th><th>Paid by</th><th class="num">Amount</th><th class="actions-col"><span class="sr-only">Actions</span></th></tr></thead>
        <tbody>${d.items.map((x) => `<tr><td>${esc(dateLabel(x.expense_date))}</td><td>${esc(x.category)}</td><td>${esc(x.description || '—')}${x.paid_to ? `<small class="muted block">To ${esc(x.paid_to)}</small>` : ''}</td><td>${esc(x.method || '—')}<small class="muted block">${esc(x.user_name)}</small></td><td class="num"><b>${money(x.amount_kobo)}</b></td>
          <td class="actions-col"><button class="icon-btn" data-edit="${x.id}" aria-label="Edit">${icon('edit', 16)}</button></td></tr>`).join('')}</tbody></table></div>`
        : emptyState('No expenses yet', 'Record what the business spends so your reports show real profit.', `<button class="btn btn-primary" data-new-inline>${icon('plus', 16)} Record expense</button>`)}</section>
    </div>`;
  };
  await load();
  const reload = () => load().catch((e) => toast(errText(e), 'error'));
  ctx.main.addEventListener('change', (e) => {
    const el = e.target as HTMLInputElement;
    if (el.matches('[data-from]')) range.from = el.value;
    if (el.matches('[data-to]')) range.to = el.value;
    if (el.matches('[data-from],[data-to]') && range.from && range.to) reload();
  });
  ctx.main.addEventListener('click', (e) => {
    const t = e.target as HTMLElement;
    const ed = t.closest<HTMLElement>('[data-edit]');
    if (ed && data) expenseForm(data.items.find((x) => x.id === Number(ed.dataset.edit))!, data.categories, reload);
    if (t.closest('[data-new-inline]') && data) expenseForm(null, data.categories, reload);
  });
  $('[data-new]')!.addEventListener('click', () => data && expenseForm(null, data.categories, reload));
  $('[data-export]')!.addEventListener('click', () => data && exportCsv(`expenses-${range.from}-to-${range.to}.csv`, ['Date', 'Category', 'Details', 'Paid to', 'Method', 'Amount', 'Recorded by'],
    data.items.map((x) => [x.expense_date, x.category, x.description, x.paid_to, x.method, nairaPlain(x.amount_kobo), x.user_name])));
}

export function expenseForm(x: Expense | null, categories: string[], done: () => void) {
  const methods = store.paymentMethods.filter((p) => p.active).map((p) => p.name);
  const m = modal({ title: x ? 'Edit expense' : 'Record expense', size: 'md', body: `<form class="stack">
    <div class="grid-2">
      <label class="field"><span>Date</span><input type="date" name="expense_date" required value="${esc(x?.expense_date ?? isoDate(new Date()))}" max="${isoDate(new Date())}"></label>
      <label class="field"><span>Amount</span><span class="input-affix"><span class="affix">${esc(store.settings.currency_symbol || '₦')}</span><input name="amount" inputmode="decimal" required value="${nairaInput(x?.amount_kobo ?? 0)}" autofocus></span></label>
    </div>
    <label class="field"><span>Category</span><input name="category" required maxlength="60" list="exp-cats" value="${esc(x?.category ?? '')}" placeholder="Choose or type"><datalist id="exp-cats">${categories.map((c) => `<option value="${esc(c)}">`).join('')}</datalist></label>
    <label class="field"><span>Details <em class="muted">(optional)</em></span><input name="description" maxlength="300" value="${esc(x?.description ?? '')}" placeholder="e.g. 50 litres for the generator"></label>
    <div class="grid-2">
      <label class="field"><span>Paid to <em class="muted">(optional)</em></span><input name="paid_to" maxlength="120" value="${esc(x?.paid_to ?? '')}"></label>
      <label class="field"><span>Paid by</span><input name="method" maxlength="40" list="exp-methods" value="${esc(x?.method ?? methods[0] ?? 'Cash')}"><datalist id="exp-methods">${methods.map((c) => `<option value="${esc(c)}">`).join('')}</datalist><small>Cash paid during your shift comes off the expected cash.</small></label>
    </div>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions">${x ? `<button type="button" class="btn btn-ghost btn-danger-text push-left" data-del>${icon('trash', 16)} Delete</button>` : ''}<button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('save_expense', { input: { id: x?.id ?? null, expense_date: v.expense_date, category: v.category, description: v.description, amount_kobo: toKobo(v.amount), method: v.method, paid_to: v.paid_to } });
      } catch (err) { $('[data-err]', form)!.textContent = errText(err); return; }
      toast('Expense saved'); m.close(); done();
    });
  });
  $('[data-del]', form)?.addEventListener('click', async () => {
    if (await confirmDialog({ title: 'Delete this expense?', message: 'It will no longer count in reports.', confirm: 'Delete', danger: true }) === null) return;
    try { await call('delete_expense', { id: x!.id }); toast('Expense deleted'); m.close(); done(); } catch (err) { toast(errText(err), 'error'); }
  });
}
