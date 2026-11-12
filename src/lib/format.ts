import { store } from '../store';

const grouped = new Intl.NumberFormat('en-NG', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const groupedWhole = new Intl.NumberFormat('en-NG', { maximumFractionDigits: 0 });

export const currency = () => store.settings.currency_symbol || '₦';

/** 1250000 kobo → "₦12,500.00" */
export function money(kobo: number, opts: { whole?: boolean } = {}): string {
  const value = (kobo || 0) / 100;
  const sign = value < 0 ? '−' : '';
  const body = opts.whole && Number.isInteger(value) ? groupedWhole.format(Math.abs(value)) : grouped.format(Math.abs(value));
  return `${sign}${currency()}${body}`;
}

/** Compact money for tight spaces: ₦1.2m, ₦85k. */
export function moneyShort(kobo: number): string {
  const v = Math.abs(kobo / 100);
  const sign = kobo < 0 ? '−' : '';
  if (v >= 1_000_000) return `${sign}${currency()}${(v / 1_000_000).toFixed(v >= 10_000_000 ? 0 : 1)}m`;
  if (v >= 10_000) return `${sign}${currency()}${Math.round(v / 1000)}k`;
  return `${sign}${currency()}${groupedWhole.format(v)}`;
}

/** Parses a money string typed by a person ("12,500.5") into kobo. */
export function toKobo(input: string | number): number {
  const n = typeof input === 'number' ? input : Number(String(input).replace(/[^\d.\-]/g, ''));
  return Number.isFinite(n) ? Math.round(n * 100) : 0;
}

export const nairaInput = (kobo: number) => (kobo ? (kobo / 100).toFixed(2).replace(/\.00$/, '') : '');

export function qty(q: number): string {
  if (Number.isInteger(q)) return groupedWhole.format(q);
  return q.toFixed(3).replace(/0+$/, '').replace(/\.$/, '');
}

const dateFmt = new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: 'numeric' });
const timeFmt = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit' });

/** Backend timestamps are local "YYYY-MM-DD HH:MM:SS". */
export function parseLocal(ts: string): Date {
  const [d, t = '00:00:00'] = ts.split(' ');
  const [y, m, day] = d.split('-').map(Number);
  const [hh, mm, ss] = t.split(':').map(Number);
  return new Date(y, (m || 1) - 1, day || 1, hh || 0, mm || 0, ss || 0);
}

export const dateLabel = (ts: string) => (ts ? dateFmt.format(parseLocal(ts)) : '');
export const timeLabel = (ts: string) => (ts ? timeFmt.format(parseLocal(ts)) : '');
export const dateTimeLabel = (ts: string) => (ts ? `${dateLabel(ts)}, ${timeLabel(ts)}` : '');

export function isoDate(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function relativeDay(ts: string): string {
  const d = parseLocal(ts);
  const today = new Date();
  const diff = Math.round((new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime() - new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()) / 86400000);
  if (diff === 0) return `Today, ${timeLabel(ts)}`;
  if (diff === 1) return `Yesterday, ${timeLabel(ts)}`;
  return dateTimeLabel(ts);
}

export function greeting(): string {
  const h = new Date().getHours();
  return h < 12 ? 'Good morning' : h < 17 ? 'Good afternoon' : 'Good evening';
}
