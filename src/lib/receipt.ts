import { call } from '../api';
import { store } from '../store';
import type { Receipt } from '../types';
import { esc, toast, errText } from './dom';
import { dateTimeLabel, money, qty } from './format';
import { isAndroid } from './platform';

const statusText: Record<string, string> = { paid: 'Paid', partial: 'Part paid', unpaid: 'Not paid', void: 'Void' };

/** The receipt as printed on thermal paper (58/80 mm) or an A4 sheet. */
export function receiptMarkup(r: Receipt, opts: { width?: string } = {}): string {
  const s = store.settings;
  const width = opts.width ?? s.receipt_width_mm ?? '80';
  const contact = [s.business_address, s.business_phone, s.business_email].filter(Boolean).map(esc).join('<br>');
  const lines = r.items.map((i) => `
    <div class="slip-line">
      <div class="slip-name">${esc(i.product_name)}</div>
      <div class="slip-calc"><span>${qty(i.quantity)} ${esc(i.unit)} × ${money(i.unit_price_kobo)}</span><b>${money(i.line_total_kobo)}</b></div>
      ${i.returned_qty ? `<div class="slip-calc slip-returned"><span>Returned ${qty(i.returned_qty)} ${esc(i.unit)}</span></div>` : ''}
    </div>`).join('');
  const row = (label: string, value: string, cls = '') => `<div class="slip-row ${cls}"><span>${esc(label)}</span><span>${value}</span></div>`;
  const pays = r.payments.map((p) => row(`${p.method}${p.reference ? ` (${p.reference})` : ''}`, money(p.amount_kobo))).join('');
  return `<article class="slip slip-${esc(width)} ${r.payment_status === 'void' ? 'is-void' : ''}">
    <header class="slip-head">
      ${s.business_logo ? `<img class="slip-logo" src="${esc(s.business_logo)}" alt="">` : ''}
      <h3>${esc(s.business_name || 'My Farm')}</h3>
      ${contact ? `<p>${contact}</p>` : ''}
    </header>
    <div class="slip-title">${esc(s.receipt_title || 'Sales receipt')}</div>
    <div class="slip-meta">
      ${row('No.', esc(r.receipt_no))}
      ${row('Date', esc(dateTimeLabel(r.created_at)))}
      ${row('Served by', esc(r.cashier_name || '—'))}
      ${r.customer_name ? row(store.label('customer'), esc(r.customer_name)) : ''}
      ${r.customer_phone ? row('Phone', esc(r.customer_phone)) : ''}
      ${r.collection_date ? row('Collect on', esc(r.collection_date)) : ''}
    </div>
    <div class="slip-items">${lines}</div>
    <div class="slip-totals">
      ${row('Subtotal', money(r.subtotal_kobo))}
      ${r.discount_kobo ? row('Discount', `−${money(r.discount_kobo)}`) : ''}
      ${r.tax_kobo ? row(`${s.tax_label || 'Tax'} (${s.tax_rate}%)`, money(r.tax_kobo)) : ''}
      ${row('Total', money(r.total_kobo), 'slip-grand')}
      ${r.returned_kobo ? row('Returned goods', `−${money(r.returned_kobo)}`) : ''}
    </div>
    <div class="slip-pay">
      ${pays}
      ${r.change_kobo > 0 ? row('Change', money(r.change_kobo)) : ''}
      ${r.refunded_kobo > 0 ? row('Refunded', `−${money(r.refunded_kobo)}`) : ''}
      ${r.balance_kobo > 0 ? row('Balance due', money(r.balance_kobo), 'slip-due') : ''}
      ${row('Status', statusText[r.payment_status] ?? r.payment_status)}
    </div>
    ${r.note ? `<p class="slip-note">${esc(r.note)}</p>` : ''}
    ${r.payment_status === 'void' ? `<p class="slip-void">VOID: ${esc(r.void_reason)}</p>` : ''}
    <footer class="slip-foot">${esc(s.receipt_footer || '')}</footer>
  </article>`;
}

/** Puts the receipt in the print-only area and opens the system print dialog. */
export async function printReceipt(r: Receipt) {
  if (isAndroid) { const { printReceiptAndroidPdf } = await import('./receipt-pdf'); return printReceiptAndroidPdf(r); }
  const width = store.settings.receipt_width_mm || '80';
  await printHtml(receiptMarkup(r, { width }), width === 'a4' ? '@page { size: A4; margin: 14mm; }' : `@page { size: ${width}mm auto; margin: 3mm 2mm; }`);
}

/** Prints any markup through the print-only area (used for receipts and the recovery key sheet). */
export async function printHtml(html: string, pageCss = '@page { size: A4; margin: 18mm; }') {
  if (isAndroid) {
    toast('Printing this page needs the Windows or Mac app. On Android you can print receipts (through RawBT) and send statements on WhatsApp.', 'info');
    return;
  }
  const area = document.getElementById('print-area')!;
  area.innerHTML = html;
  let style = document.getElementById('page-size') as HTMLStyleElement | null;
  if (!style) { style = document.createElement('style'); style.id = 'page-size'; document.head.append(style); }
  style.textContent = pageCss;
  document.body.classList.add('printing');
  const cleanup = () => { document.body.classList.remove('printing'); window.removeEventListener('afterprint', cleanup); };
  window.addEventListener('afterprint', cleanup);
  try {
    await call('print_window');
  } catch {
    try { window.print(); } catch (e) { toast(`Printing is not available: ${errText(e)}`, 'error'); }
  }
  window.setTimeout(cleanup, 60_000);
}
