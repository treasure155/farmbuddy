import { call } from '../api';
import { errText, esc, modal, toast, withBusy } from './dom';
import { dateLabel, money, nairaInput, toKobo } from './format';
import { store } from '../store';
import type { Customer, ReceiptSummary } from '../types';

const owed = (r: ReceiptSummary) => (r.total_kobo - r.returned_kobo) - (r.amount_paid_kobo - r.refunded_kobo);

/**
 * A customer who owes comes to pay: one amount, applied to their unpaid receipts oldest first.
 * Each part is saved as a normal "Record payment" on that receipt, so it lands in the open shift
 * (cash-up and X / Z report), in the day's takings and in the customer's balance.
 */
export async function receivePayment(customerId: number, done: () => void) {
  try {
    const [customers, receipts] = await Promise.all([
      call<Customer[]>('list_customers', { search: '' }),
      call<ReceiptSummary[]>('list_receipts', { filter: { customer_id: customerId, status: 'owing' } }),
    ]);
    const customer = customers.find((c) => c.id === customerId);
    const unpaid = receipts.filter((r) => r.payment_status !== 'void' && owed(r) > 0).sort((a, b) => a.id - b.id);
    if (!customer) { toast('Customer not found', 'error'); return; }
    if (!unpaid.length) { toast('There is no unpaid receipt here that you can take payment on.', 'info'); return; }
    const total = unpaid.reduce((a, r) => a + owed(r), 0);
    const methods = store.paymentMethods.filter((p) => p.active);
    const hidden = customer.balance_kobo > total;

    const m = modal({ title: 'Receive payment', subtitle: customer.name, size: 'sm', body: `<form class="stack">
      <div class="callout callout-warn"><div><b>${money(total)} owed</b><span>${unpaid.length} unpaid receipt${unpaid.length === 1 ? '' : 's'}. The payment clears the oldest first.</span></div></div>
      <ul class="pay-receipts">${unpaid.map((r) => `<li><span>${esc(r.receipt_no)} · ${esc(dateLabel(r.created_at))}</span><b>${money(owed(r))}</b></li>`).join('')}</ul>
      ${hidden ? '<p class="muted small">This customer also owes on receipts issued by someone else. The owner or a manager can take payment on those.</p>' : ''}
      <label class="field"><span>Amount received</span><span class="input-affix"><span class="affix">${esc(store.settings.currency_symbol || '₦')}</span><input name="amount" inputmode="decimal" required value="${nairaInput(total)}" autofocus></span></label>
      <label class="field"><span>Method</span><select name="method">${methods.map((p) => `<option>${esc(p.name)}</option>`).join('')}</select></label>
      <label class="field"><span>Reference <em class="muted">(optional)</em></span><input name="reference" maxlength="80"></label>
      <p class="form-error" role="alert" data-err></p>
      <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>Cancel</button><button class="btn btn-primary">Save payment</button></div></form>` });

    const form = m.el.querySelector('form')!;
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      const err = form.querySelector('[data-err]')!;
      err.textContent = '';
      const f = new FormData(form);
      const amount = toKobo(String(f.get('amount')));
      if (!(amount > 0)) { err.textContent = 'Enter an amount greater than zero.'; return; }
      if (amount > total) { err.textContent = `That is more than the ${money(total)} owed.`; return; }
      withBusy(form.querySelector('.btn-primary'), async () => {
        let left = amount;
        try {
          for (const r of unpaid) {
            if (left <= 0) break;
            const part = Math.min(left, owed(r));
            await call('record_payment', { receiptId: r.id, method: f.get('method'), amountKobo: part, reference: String(f.get('reference') || '') });
            left -= part;
          }
          const still = total - amount;
          toast(still > 0 ? `Payment saved. ${money(still)} still owed.` : 'Payment saved. Fully paid.');
          m.close();
        } catch (x) {
          err.textContent = errText(x);
        }
        done();
      });
    });
  } catch (x) {
    toast(errText(x), 'error');
  }
}
