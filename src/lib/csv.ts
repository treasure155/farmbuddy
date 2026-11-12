import { call } from '../api';
import { toast, errText } from './dom';

const cell = (v: unknown) => {
  let s = String(v ?? '');
  // Neutralise spreadsheet formula injection from user-entered text.
  if (/^[=+\-@\t\r]/.test(s) && !/^-?\d/.test(s)) s = `'${s}`;
  return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
};

export function toCsv(headers: string[], rows: unknown[][]): string {
  return [headers, ...rows].map((r) => r.map(cell).join(',')).join('\r\n');
}

/** Writes a CSV into the Downloads folder via the backend. */
export async function exportCsv(fileName: string, headers: string[], rows: unknown[][]) {
  try {
    const path = await call<string>('save_export', { fileName, content: toCsv(headers, rows) });
    toast(`Saved to ${path}`);
  } catch (e) {
    toast(errText(e), 'error');
  }
}

export const nairaPlain = (kobo: number) => (kobo / 100).toFixed(2);
