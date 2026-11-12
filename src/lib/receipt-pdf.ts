import { store } from '../store';
import type { Receipt } from '../types';
import { $, errText, modal, toast } from './dom';
import { dateTimeLabel, money, qty } from './format';
import { icon } from './icons';
import { isAndroid } from './platform';
import { layoutFor, pdfBase64, receiptLines } from './pdf';
import { waNumber } from './whatsapp';

/** Functions added to the Android app by tools/android-native.mjs (absent on Windows, Mac and older APKs). */
interface Native {
  shareFile(b64: string, name: string, mime: string, packages: string, text: string, jid: string): string;
  printPdf(b64: string, name: string): string;
}
const native = () => (window as unknown as { EstockNative?: Native }).EstockNative;

/** The receipt as a PDF (base64), laid out for the paper width chosen in Settings. */
export function receiptPdf(r: Receipt): { b64: string; name: string } {
  const s = store.settings;
  const layout = layoutFor(s.receipt_width_mm || '80');
  const lines = receiptLines(r, {
    name: s.business_name || 'My Farm',
    contact: [s.business_address, s.business_phone, s.business_email],
    title: s.receipt_title || 'Sales receipt',
    footer: s.receipt_footer || '',
    taxLabel: s.tax_label || 'Tax', taxRate: s.tax_rate || '',
    customerLabel: store.label('customer'),
    money, qty, dateTime: dateTimeLabel,
  }, layout.cols);
  return { b64: pdfBase64(lines, layout), name: `Receipt-${r.receipt_no.replace(/[^A-Za-z0-9-]/g, '')}.pdf` };
}

/** Android: shares the receipt as a PDF straight into WhatsApp. Everywhere else the old text message is used. */
export async function shareReceipt(r: Receipt, fallback: () => void) {
  const n = native();
  if (!isAndroid || !n) return fallback();
  try {
    const { b64, name } = receiptPdf(r);
    const caption = `${store.settings.business_name || 'Receipt'} - ${r.receipt_no}`;
    const err = n.shareFile(b64, name, 'application/pdf', 'com.whatsapp,com.whatsapp.w4b', caption, r.customer_phone.trim() ? waNumber(r.customer_phone) : '');
    if (err) toast(`Could not share the receipt: ${err}`, 'error');
  } catch (e) {
    toast(`Could not share the receipt: ${errText(e)}`, 'error');
  }
}

/** Android: lets the user pick the phone's own print screen (Save as PDF or any printer) or a thermal printer. */
export async function printReceiptAndroidPdf(r: Receipt) {
  const thermal = async () => { const { printReceiptAndroid } = await import('./escpos'); return printReceiptAndroid(r); };
  const n = native();
  if (!n) return thermal();
  const m = modal({ title: 'Print receipt', size: 'sm', body: `<div class="stack">
    <button class="btn btn-primary btn-lg btn-block" data-pdf>${icon('print', 18)} Print or save as PDF</button>
    <p class="muted small">Opens your phone's print screen. Choose <b>Save as PDF</b>, or any printer your phone can reach.</p>
    <button class="btn btn-ghost btn-block" data-thermal>Thermal printer (RawBT)</button>
  </div>` });
  $('[data-pdf]', m.el)!.addEventListener('click', () => {
    m.close();
    try {
      const { b64, name } = receiptPdf(r);
      const err = n.printPdf(b64, name);
      if (err) toast(`Could not open printing: ${err}`, 'error');
    } catch (e) { toast(`Could not open printing: ${errText(e)}`, 'error'); }
  });
  $('[data-thermal]', m.el)!.addEventListener('click', () => { m.close(); void thermal(); });
}
