/**
 * Receipt printing on Android: builds ESC/POS bytes for 58/80 mm Bluetooth or
 * USB thermal printers and hands them to the free RawBT print service app
 * (Play Store: "RawBT Print Service"), which talks to the printer.
 */
import { store } from '../store';
import type { Receipt } from '../types';
import { toast, errText } from './dom';
import { dateTimeLabel, qty } from './format';

const ESC = 0x1b, GS = 0x1d;

/** Thermal printers use a single-byte code page: keep plain ASCII. */
function ascii(s: string): string {
  return s.replace(/₦/g, 'N').replace(/[‘’]/g, "'").replace(/[“”]/g, '"').replace(/[–—−]/g, '-').replace(/×/g, 'x')
    .normalize('NFKD').replace(/[^\x20-\x7e\n]/g, '');
}

function amount(kobo: number): string {
  const v = (kobo / 100).toFixed(2);
  const [w, d] = v.replace('-', '').split('.');
  return `${kobo < 0 ? '-' : ''}${(store.settings.currency_symbol === '₦' || !store.settings.currency_symbol) ? 'N' : ascii(store.settings.currency_symbol)}${w.replace(/\B(?=(\d{3})+(?!\d))/g, ',')}.${d}`;
}

class Bytes {
  private out: number[] = [];
  raw(...b: number[]) { this.out.push(...b); return this; }
  text(s: string) { for (const ch of ascii(s)) this.out.push(ch.charCodeAt(0)); return this; }
  line(s = '') { return this.text(s).raw(0x0a); }
  center(on: boolean) { return this.raw(ESC, 0x61, on ? 1 : 0); }
  bold(on: boolean) { return this.raw(ESC, 0x45, on ? 1 : 0); }
  big(on: boolean) { return this.raw(GS, 0x21, on ? 0x11 : 0); }
  base64() { let s = ''; for (const b of this.out) s += String.fromCharCode(b); return btoa(s); }
}

export function receiptEscPos(r: Receipt): string {
  const s = store.settings;
  const width = s.receipt_width_mm === '58' ? 32 : 48;
  const pair = (l: string, rgt: string) => { l = ascii(l); rgt = ascii(rgt); const space = Math.max(1, width - l.length - rgt.length); return l.length + rgt.length + 1 > width ? `${l}\n${' '.repeat(Math.max(0, width - rgt.length))}${rgt}` : l + ' '.repeat(space) + rgt; };
  const rule = '-'.repeat(width);
  const b = new Bytes().raw(ESC, 0x40);
  b.center(true).bold(true).big(true).line(s.business_name || 'My Farm').big(false).bold(false);
  [s.business_address, s.business_phone, s.business_email].filter(Boolean).forEach((x) => b.line(x));
  b.line(rule).bold(true).line((s.receipt_title || 'Sales receipt').toUpperCase()).bold(false).center(false);
  b.line(pair('No.', r.receipt_no)).line(pair('Date', dateTimeLabel(r.created_at))).line(pair('Served by', r.cashier_name || '-'));
  if (r.customer_name) b.line(pair(store.label('customer'), r.customer_name));
  if (r.collection_date) b.line(pair('Collect on', r.collection_date));
  b.line(rule);
  for (const i of r.items) {
    b.bold(true).line(i.product_name).bold(false);
    b.line(pair(`  ${qty(i.quantity)} ${i.unit} x ${amount(i.unit_price_kobo)}`, amount(i.line_total_kobo)));
    if (i.returned_qty) b.line(`  Returned ${qty(i.returned_qty)} ${i.unit}`);
  }
  b.line(rule).line(pair('Subtotal', amount(r.subtotal_kobo)));
  if (r.discount_kobo) b.line(pair('Discount', `-${amount(r.discount_kobo)}`));
  if (r.tax_kobo) b.line(pair(`${s.tax_label || 'Tax'} (${s.tax_rate}%)`, amount(r.tax_kobo)));
  b.bold(true).line(pair('TOTAL', amount(r.total_kobo))).bold(false);
  if (r.returned_kobo) b.line(pair('Returned goods', `-${amount(r.returned_kobo)}`));
  b.line(rule);
  for (const p of r.payments) b.line(pair(p.method + (p.reference ? ` (${p.reference})` : ''), amount(p.amount_kobo)));
  if (r.change_kobo > 0) b.line(pair('Change', amount(r.change_kobo)));
  if (r.refunded_kobo > 0) b.line(pair('Refunded', `-${amount(r.refunded_kobo)}`));
  if (r.balance_kobo > 0) b.bold(true).line(pair('BALANCE DUE', amount(r.balance_kobo))).bold(false);
  if (r.note) b.line().line(r.note);
  if (r.payment_status === 'void') b.line().bold(true).line(`VOID: ${r.void_reason}`).bold(false);
  b.line().center(true).line(s.receipt_footer || '').center(false);
  b.raw(ESC, 0x64, 4).raw(GS, 0x56, 0x42, 0x00);
  return b.base64();
}

/** Sends a receipt to RawBT. Returns false when RawBT is not installed. */
export async function printReceiptAndroid(r: Receipt) {
  const url = `rawbt:base64,${receiptEscPos(r)}`;
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  } catch (e) {
    toast(`Install "RawBT Print Service" from the Play Store and pair your printer in it, then try again. (${errText(e)})`, 'error');
  }
}
