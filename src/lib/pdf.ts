import type { Receipt } from '../types';

/**
 * A tiny PDF writer for plain monospaced text (receipts). It uses the PDF standard
 * Courier fonts, so nothing is embedded and no library is needed; a 3-line
 * receipt is about 1.5 KB. Used on Android to share receipts on WhatsApp and to
 * hand them to the phone's print / Save-as-PDF screen.
 */
export interface PdfLine { t: string; b?: boolean }
export interface PdfLayout { widthPt: number; marginX: number; marginY: number; fontSize: number; cols: number; pageHeightPt?: number }

const CP1252: Record<string, number> = { '€': 0x80, '‚': 0x82, '„': 0x84, '…': 0x85, '‘': 0x91, '’': 0x92, '“': 0x93, '”': 0x94, '•': 0x95, '–': 0x96, '—': 0x97, '™': 0x99 };

/** The standard PDF fonts only know Western characters: swap the naira sign for "N" and anything unknown for "?". */
export function pdfSafe(s: string): string {
  return s.replace(/₦/g, 'N').replace(/[−‒]/g, '-').replace(/\u00a0/g, ' ').replace(/[\r\n\t]+/g, ' ')
    .split('').map((ch) => {
      const c = ch.charCodeAt(0);
      if (c >= 32 && c < 127) return ch;
      if (c >= 160 && c < 256) return ch;
      const m = CP1252[ch];
      return m ? String.fromCharCode(m) : '?';
    }).join('');
}

const pdfEsc = (s: string) => s.replace(/[\\()]/g, (c) => `\\${c}`);

/** Builds the PDF as a "binary string" (one char = one byte). */
export function buildPdfBinary(lines: PdfLine[], layout: PdfLayout): string {
  const { widthPt, marginX, marginY, fontSize } = layout;
  const lh = fontSize * 1.28;
  const perPage = layout.pageHeightPt ? Math.max(1, Math.floor((layout.pageHeightPt - marginY * 2) / lh)) : Math.max(1, lines.length);
  const pages: PdfLine[][] = [];
  for (let i = 0; i < lines.length; i += perPage) pages.push(lines.slice(i, i + perPage));
  if (!pages.length) pages.push([{ t: '' }]);

  let out = '%PDF-1.4\n%\xE2\xE3\xCF\xD3\n';
  const offsets: number[] = [];
  const add = (n: number, body: string) => { offsets[n] = out.length; out += `${n} 0 obj\n${body}\nendobj\n`; };

  add(1, '<< /Type /Catalog /Pages 2 0 R >>');
  add(2, `<< /Type /Pages /Kids [${pages.map((_, i) => `${5 + i * 2} 0 R`).join(' ')}] /Count ${pages.length} >>`);
  add(3, '<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>');
  add(4, '<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Bold /Encoding /WinAnsiEncoding >>');

  pages.forEach((page, i) => {
    const pageH = layout.pageHeightPt ?? Math.max(120, marginY * 2 + page.length * lh);
    let stream = '';
    page.forEach((ln, idx) => {
      if (!ln.t.trim()) return;
      const y = (pageH - marginY - fontSize - idx * lh).toFixed(2);
      stream += `BT /${ln.b ? 'F2' : 'F1'} ${fontSize} Tf ${marginX.toFixed(2)} ${y} Td (${pdfEsc(ln.t)}) Tj ET\n`;
    });
    add(5 + i * 2, `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${widthPt.toFixed(2)} ${pageH.toFixed(2)}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents ${6 + i * 2} 0 R >>`);
    add(6 + i * 2, `<< /Length ${stream.length} >>\nstream\n${stream}endstream`);
  });

  const count = 5 + pages.length * 2;
  const xref = out.length;
  out += `xref\n0 ${count}\n0000000000 65535 f \n`;
  for (let n = 1; n < count; n++) out += `${String(offsets[n]).padStart(10, '0')} 00000 n \n`;
  out += `trailer\n<< /Size ${count} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return out;
}

export const pdfBase64 = (lines: PdfLine[], layout: PdfLayout) => btoa(buildPdfBinary(lines, layout));

// ------------------------------------------------------------------ text layout (monospaced columns)

export function wrapText(text: string, cols: number): string[] {
  const out: string[] = [];
  let line = '';
  for (let word of pdfSafe(text).split(' ')) {
    while (word.length > cols) { if (line) { out.push(line); line = ''; } out.push(word.slice(0, cols)); word = word.slice(cols); }
    if (!line) line = word;
    else if (line.length + 1 + word.length <= cols) line += ` ${word}`;
    else { out.push(line); line = word; }
  }
  out.push(line);
  return out;
}

const centre = (s: string, cols: number) => ' '.repeat(Math.max(0, Math.floor((cols - s.length) / 2))) + s;

/** "label ........ value" on one line, wrapping the label when it is long. */
function pairLines(left: string, right: string, cols: number): string[] {
  const l = pdfSafe(left), r = pdfSafe(right);
  if (l.length + 1 + r.length <= cols) return [l + ' '.repeat(cols - l.length - r.length) + r];
  const parts = wrapText(l, cols);
  const last = parts[parts.length - 1];
  if (last.length + 1 + r.length <= cols) { parts[parts.length - 1] = last + ' '.repeat(cols - last.length - r.length) + r; return parts; }
  return [...parts, ' '.repeat(Math.max(0, cols - r.length)) + r];
}

export interface ReceiptCfg {
  name: string; contact: string[]; title: string; footer: string; taxLabel: string; taxRate: string; customerLabel: string;
  money: (kobo: number) => string; qty: (n: number) => string; dateTime: (iso: string) => string;
}
const STATUS: Record<string, string> = { paid: 'Paid', partial: 'Part paid', unpaid: 'Not paid', void: 'Void' };

export function receiptLines(r: Receipt, c: ReceiptCfg, cols: number): PdfLine[] {
  const L: PdfLine[] = [];
  const text = (t: string, b = false) => L.push({ t, b });
  const wrapC = (t: string, b = false) => wrapText(t, cols).forEach((x) => text(centre(x, cols), b));
  const rule = () => text('-'.repeat(cols));
  const pair = (l: string, v: string, b = false) => pairLines(l, v, cols).forEach((x) => text(x, b));

  wrapC(c.name.toUpperCase(), true);
  c.contact.filter(Boolean).forEach((x) => wrapC(x));
  text('');
  wrapC(c.title.toUpperCase(), true);
  rule();
  pair('No.', r.receipt_no);
  pair('Date', c.dateTime(r.created_at));
  pair('Served by', r.cashier_name || '-');
  if (r.customer_name) pair(c.customerLabel, r.customer_name);
  if (r.customer_phone) pair('Phone', r.customer_phone);
  if (r.collection_date) pair('Collect on', r.collection_date);
  rule();
  for (const i of r.items) {
    wrapText(i.product_name, cols).forEach((x) => text(x, true));
    pair(`${c.qty(i.quantity)} ${i.unit} x ${c.money(i.unit_price_kobo)}`, c.money(i.line_total_kobo));
    if (i.returned_qty) text(`  Returned ${c.qty(i.returned_qty)} ${i.unit}`);
  }
  rule();
  pair('Subtotal', c.money(r.subtotal_kobo));
  if (r.discount_kobo) pair('Discount', `-${c.money(r.discount_kobo)}`);
  if (r.tax_kobo) pair(`${c.taxLabel} (${c.taxRate}%)`, c.money(r.tax_kobo));
  pair('TOTAL', c.money(r.total_kobo), true);
  if (r.returned_kobo) pair('Returned goods', `-${c.money(r.returned_kobo)}`);
  rule();
  r.payments.forEach((p) => pair(`${p.method}${p.reference ? ` (${p.reference})` : ''}`, c.money(p.amount_kobo)));
  if (r.change_kobo > 0) pair('Change', c.money(r.change_kobo));
  if (r.refunded_kobo > 0) pair('Refunded', `-${c.money(r.refunded_kobo)}`);
  if (r.balance_kobo > 0) pair('BALANCE DUE', c.money(r.balance_kobo), true);
  pair('Status', STATUS[r.payment_status] ?? r.payment_status);
  if (r.note) { text(''); wrapText(r.note, cols).forEach((x) => text(x)); }
  if (r.payment_status === 'void') { text(''); wrapText(`VOID: ${r.void_reason}`, cols).forEach((x) => text(x, true)); }
  if (c.footer) { text(''); wrapC(c.footer); }
  return L;
}

/** Page size and number of text columns for the receipt paper width chosen in Settings ('58', '80' or 'a4'). */
export function layoutFor(width: string): PdfLayout {
  const MM = 72 / 25.4;
  if (width === 'a4') {
    const cols = 46, fontSize = 11, widthPt = 595.28;
    return { widthPt, marginX: (widthPt - cols * fontSize * 0.6) / 2, marginY: 50, fontSize, cols, pageHeightPt: 841.89 };
  }
  const mm = width === '58' ? 58 : 80;
  const widthPt = mm * MM, marginX = mm === 58 ? 6 : 8, fontSize = mm === 58 ? 7.4 : 8.4;
  return { widthPt, marginX, marginY: 12, fontSize, cols: Math.floor((widthPt - marginX * 2) / (fontSize * 0.6)) };
}
