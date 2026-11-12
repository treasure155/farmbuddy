import { store } from '../../store';
import { en, type TKey } from './en';
import { ib } from './ib';

export type Lang = 'en' | 'ib';
export type { TKey };

export const LANGUAGES: Array<{ code: Lang; name: string }> = [{ code: 'en', name: 'English' }, { code: 'ib', name: 'Ibibio' }];

/** The language chosen in Settings (or during setup). English until set. */
export function lang(): Lang { return store.settings.language === 'ib' ? 'ib' : 'en'; }

/**
 * Translates a key into the current language. Falls back to English when the
 * Ibibio text has not been approved yet, then to `fallback` for keys that are
 * not in the dictionary (such as text supplied by the backend). `{name}`
 * placeholders are filled from `vars`. User-entered data is never passed
 * through here, so farm names, product names and notes are never changed.
 */
export function t(key: TKey | string, vars: Record<string, string | number> = {}, fallback?: string): string {
  const k = key as TKey;
  const text = (lang() === 'ib' ? ib[k] : undefined) ?? en[k] ?? fallback ?? String(key);
  return text.replace(/\{(\w+)\}/g, (_, v: string) => (v in vars ? String(vars[v]) : `{${v}}`));
}

/** Label for a coded value, e.g. tl('species', 'broiler') → "Broilers". */
export const tl = (prefix: string, value: string) => t(`${prefix}.${value}`, {}, value.replace(/_/g, ' '));

/** How many keys still need an approved Ibibio translation. */
export function ibibioCoverage(): { total: number; done: number } {
  const keys = Object.keys(en);
  return { total: keys.length, done: keys.filter((k) => !!ib[k as TKey]).length };
}
