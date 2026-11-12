import { describe, expect, it } from 'vitest';
import { money, qty, toKobo, isoDate } from '../src/lib/format';

describe('money', () => {
  it('stores naira as whole kobo without float drift', () => {
    expect(toKobo('1,250.50')).toBe(125050);
    expect(toKobo('0.1')).toBe(10);
    expect(toKobo(19.99)).toBe(1999);
  });
  it('formats kobo as naira', () => {
    expect(money(125050)).toContain('1,250.50');
    expect(money(125000, { whole: true })).toContain('1,250');
    expect(money(125000, { whole: true })).not.toContain('.00');
  });
  it('formats quantities without trailing zeros', () => {
    expect(qty(12)).toBe('12');
    expect(qty(2.5)).toBe('2.5');
  });
  it('gives local ISO dates', () => {
    expect(isoDate(new Date(2026, 0, 5))).toBe('2026-01-05');
  });
});
