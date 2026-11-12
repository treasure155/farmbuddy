import { describe, expect, it } from 'vitest';
import { fieldsHtml, num, tableHtml, tabsHtml } from '../src/lib/ui';
import { reportLines } from '../src/lib/report-pdf';

describe('form and table helpers', () => {
  it('reads empty numbers as not measured, never as zero', () => {
    expect(num('')).toBeNull();
    expect(num('  ')).toBeNull();
    expect(num('1,200.5')).toBe(1200.5);
    expect(num('abc')).toBeNull();
    expect(num('0')).toBe(0);
  });
  it('escapes user text in fields and tables', () => {
    expect(fieldsHtml([{ name: 'x', label: '<b>', value: '"><script>' }])).not.toContain('<script>');
    const html = tableHtml(['Name', 'Qty#'], [['a', '1']]);
    expect(html).toContain('class="num"');
    expect(tableHtml(['A'], [], 'Empty here')).toContain('Empty here');
  });
  it('marks the active tab', () => {
    expect(tabsHtml([['a', 'A'], ['b', 'B']], 'b')).toMatch(/is-on" aria-selected="true" data-tab="b"/);
  });
});

describe('PDF report layout', () => {
  it('keeps tables within the page width', () => {
    const lines = reportLines('Title', ['Period'], [{ title: 'S', headers: ['Name', 'Amount'], numeric: [false, true], rows: [['x'.repeat(300), '₦1,000.00']] }]);
    for (const l of lines) expect(l.t.length).toBeLessThanOrEqual(107);
  });
});
