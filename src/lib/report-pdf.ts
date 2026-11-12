import { call } from '../api';
import { errText, toast } from './dom';
import { pdfBase64, pdfSafe, wrapText, type PdfLine } from './pdf';

/** A section of a printable report: a heading, optional notes and an optional table. */
export interface ReportSection { title: string; notes?: string[]; headers?: string[]; rows?: string[][]; numeric?: boolean[] }

const A4 = { widthPt: 595.28, marginX: 36, marginY: 40, fontSize: 8, cols: 107, pageHeightPt: 841.89 };

/** Lays out sections as fixed-width text, shrinking wide columns to fit the page. */
export function reportLines(title: string, subtitle: string[], sections: ReportSection[]): PdfLine[] {
  const W = A4.cols;
  const L: PdfLine[] = [{ t: pdfSafe(title), b: true }, ...subtitle.flatMap((s) => wrapText(pdfSafe(s), W).map((t) => ({ t }))), { t: '' }];
  for (const s of sections) {
    L.push({ t: pdfSafe(s.title.toUpperCase()), b: true });
    for (const n of s.notes ?? []) L.push(...wrapText(pdfSafe(n), W).map((t) => ({ t })));
    if (s.headers && s.rows) {
      const cols = s.headers.length;
      const widths = s.headers.map((h, i) => Math.max(h.length, ...s.rows!.map((r) => pdfSafe(r[i] ?? '').length)));
      const gap = cols - 1;
      let total = widths.reduce((a, b) => a + b, 0) + gap;
      // Shrink the widest text columns until the table fits.
      while (total > W) {
        const i = widths.indexOf(Math.max(...widths));
        if (widths[i] <= 6) break;
        widths[i]--; total--;
      }
      const fmt = (cells: string[]) => cells.map((c, i) => {
        const t = pdfSafe(c ?? '');
        const cut = t.length > widths[i] ? `${t.slice(0, Math.max(1, widths[i] - 1))}~` : t;
        return s.numeric?.[i] ? cut.padStart(widths[i]) : cut.padEnd(widths[i]);
      }).join(' ');
      L.push({ t: fmt(s.headers), b: true });
      L.push({ t: widths.map((w) => '-'.repeat(w)).join(' ') });
      for (const r of s.rows) L.push({ t: fmt(r) });
      if (!s.rows.length) L.push({ t: '(none)' });
    }
    L.push({ t: '' });
  }
  return L;
}

/** Builds the PDF and saves it into Downloads. */
export async function savePdfReport(fileName: string, title: string, subtitle: string[], sections: ReportSection[]) {
  try {
    const path = await call<string>('save_pdf_export', { fileName, base64Pdf: pdfBase64(reportLines(title, subtitle, sections), A4) });
    toast(`Saved to ${path}`);
  } catch (e) { toast(errText(e), 'error'); }
}
