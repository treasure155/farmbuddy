import { store } from '../store';
import type { Receipt, Statement } from '../types';
import { openExternal } from './platform';
import { dateLabel, dateTimeLabel, money, qty } from './format';

/** Turns a local number like 0803 123 4567 into the international form WhatsApp needs (2348031234567). */
export function waNumber(phone: string): string {
  const cc = (store.settings.whatsapp_country_code || '234').replace(/\D/g, '');
  let d = phone.replace(/[^\d+]/g, '');
  if (d.startsWith('+')) return d.slice(1);
  if (d.startsWith('00')) return d.slice(2);
  if (d.startsWith('0')) return cc + d.slice(1);
  if (d.length > 0 && d.length <= 10 && !d.startsWith(cc)) d = cc + d;
  return d;
}

/** Opens WhatsApp (app or web) with the message ready to send. Without a phone number, WhatsApp asks who to send it to. */
export async function shareWhatsApp(phone: string, text: string) {
  const num = phone.trim() ? waNumber(phone) : '';
  const url = `https://wa.me/${num}?text=${encodeURIComponent(text)}`;
  await openExternal(url);
}

const biz = () => store.settings.business_name || 'My Farm';

export function receiptText(r: Receipt): string {
  const lines = r.items.map((i) => `• ${i.product_name}: ${qty(i.quantity)} ${i.unit} × ${money(i.unit_price_kobo)} = ${money(i.line_total_kobo)}${i.returned_qty ? ` (returned ${qty(i.returned_qty)})` : ''}`);
  const out = [
    `*${biz()}*`,
    `${store.settings.receipt_title || 'Sales receipt'} ${r.receipt_no}`,
    dateTimeLabel(r.created_at),
    '',
    ...lines,
    '',
    r.discount_kobo ? `Discount: −${money(r.discount_kobo)}` : '',
    r.tax_kobo ? `${store.settings.tax_label || 'Tax'}: ${money(r.tax_kobo)}` : '',
    `*Total: ${money(r.total_kobo)}*`,
    r.returned_kobo ? `Returned: −${money(r.returned_kobo)}` : '',
    `Paid: ${money(r.amount_paid_kobo - r.refunded_kobo)}`,
    r.balance_kobo > 0 ? `*Balance due: ${money(r.balance_kobo)}*` : '',
    r.collection_date ? `Collect on: ${dateLabel(r.collection_date)}` : '',
    '',
    store.settings.receipt_footer || 'Thank you.',
  ];
  return out.filter((l, i, a) => l !== '' || (a[i - 1] !== '' && i > 0)).join('\n').trim();
}

export function statementText(s: Statement): string {
  return [
    `*${biz()}*`,
    `Statement of account for ${s.customer_name}`,
    `${dateLabel(s.from)} to ${dateLabel(s.to)}`,
    '',
    `Opening balance: ${money(s.opening_kobo)}`,
    ...s.lines.map((l) => `${dateLabel(l.date)} · ${l.kind} ${l.reference}: ${l.debit_kobo ? `+${money(l.debit_kobo)}` : `−${money(l.credit_kobo)}`}`),
    '',
    `*Balance now: ${money(s.closing_kobo)}*`,
    s.closing_kobo > 0 ? 'Kindly settle the balance at your earliest convenience. Thank you.' : 'Thank you for your patronage.',
  ].join('\n');
}
