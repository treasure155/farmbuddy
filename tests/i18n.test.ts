import { describe, expect, it } from 'vitest';
import { en } from '../src/lib/i18n/en';
import { ib } from '../src/lib/i18n/ib';
import { t, tl } from '../src/lib/i18n';
import { store } from '../src/store';

describe('translations', () => {
  it('English is the fallback for every key', () => {
    store.settings.language = 'ib';
    expect(t('nav.settings')).toBe(en['nav.settings']);
    store.settings.language = 'en';
  });
  it('Ibibio never has keys English does not', () => {
    for (const k of Object.keys(ib)) expect(k in en).toBe(true);
  });
  it('fills placeholders and falls back for coded values', () => {
    expect(t('missing.key', {}, 'Hello {name}').replace('{name}', 'x')).toBe('Hello x');
    expect(t('missing.key', { name: 'Ada' }, 'Hello {name}')).toBe('Hello Ada');
    expect(tl('species', 'catfish_other')).toBe('catfish other');
  });
});

describe('Ibibio on screen', () => {
  it('uses approved Ibibio for keys and falls back to English for the rest', async () => {
    store.settings.language = 'ib';
    expect(t('nav.home')).toBe('Mfịn');
    expect(t('nav.settings')).toBe(en['nav.settings']);
    store.settings.language = 'en';
    expect(t('nav.home')).toBe('Today');
  });
  it('swaps whole interface words, never table cells or partial sentences', async () => {
    const { _forTests } = await import('../src/lib/i18n/ib-dom');
    document.body.innerHTML = '<button>Close</button><p>Close the batch when done</p><table><tr><td>Yam</td></tr></table><input placeholder="Search"><h3> Livestock </h3>';
    _forTests.walk(document.body);
    expect(document.querySelector('button')!.textContent).toBe('Kʌk');
    expect(document.querySelector('p')!.textContent).toBe('Close the batch when done');
    expect(document.querySelector('td')!.textContent).toBe('Yam');
    expect(document.querySelector('input')!.getAttribute('placeholder')).toBe('Yom');
    expect(document.querySelector('h3')!.textContent).toBe(' Unam ufọk ');
  });
  it('has no Ibibio that is just the English again', async () => {
    const { IB_TEXT } = await import('../src/lib/i18n/ib-text');
    for (const [e, i] of Object.entries(IB_TEXT)) expect(i).not.toBe(e);
  });
});
