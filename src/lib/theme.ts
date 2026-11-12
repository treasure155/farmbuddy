/** Builds the colour system from two business colours chosen in Settings. */

export const DEFAULT_PRIMARY = '#17845A';
export const DEFAULT_SECONDARY = '#0F2E24';

export const PRESETS: Array<{ name: string; primary: string; secondary: string }> = [
  { name: 'FarmBuddy emerald', primary: '#17845A', secondary: '#0F2E24' },
  { name: 'Harvest orange', primary: '#E07A1F', secondary: '#1E4D2B' },
  { name: 'Harvest green', primary: '#2E8B3E', secondary: '#1E3A2F' },
  { name: 'Ocean', primary: '#0E8DA6', secondary: '#12355B' },
  { name: 'Berry', primary: '#C2185B', secondary: '#2A2F5B' },
  { name: 'Gold & charcoal', primary: '#C98C0A', secondary: '#2B2D33' },
  { name: 'Brick & slate', primary: '#C0442B', secondary: '#2F3E4E' },
];

type RGB = [number, number, number];

const isHex = (v: string) => /^#[0-9a-f]{6}$/i.test(v);
const toRgb = (hex: string): RGB => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as RGB;
const toHex = ([r, g, b]: RGB) => '#' + [r, g, b].map((n) => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, '0')).join('');
const mix = (a: string, b: string, amount: number) => {
  const x = toRgb(a), y = toRgb(b);
  return toHex([0, 1, 2].map((i) => x[i] + (y[i] - x[i]) * amount) as RGB);
};

function luminance(hex: string): number {
  const c = toRgb(hex).map((v) => { const s = v / 255; return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4; });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
export function contrast(a: string, b: string): number {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
}
/** Picks white or deep ink for text placed on a colour. */
export const inkFor = (bg: string) => (contrast(bg, '#FFFFFF') >= 3 ? '#FFFFFF' : '#141B26');

export function applyTheme(primary?: string, secondary?: string, mode?: string): void {
  const p = primary && isHex(primary) ? primary : DEFAULT_PRIMARY;
  const s = secondary && isHex(secondary) ? secondary : DEFAULT_SECONDARY;
  const dark = mode === 'dark';
  const root = document.documentElement;
  root.dataset.theme = dark ? 'dark' : 'light';
  const surface = dark ? '#171C24' : '#FFFFFF';
  const vars: Record<string, string> = {
    '--brand': p,
    '--brand-hover': mix(p, '#000000', 0.1),
    '--brand-press': mix(p, '#000000', 0.2),
    '--brand-ink': inkFor(p),
    '--brand-soft': mix(p, surface, dark ? 0.82 : 0.9),
    '--brand-line': mix(p, surface, dark ? 0.6 : 0.65),
    // Text in the brand colour must stay readable on the page.
    '--brand-text': contrast(p, surface) >= 4.5 ? p : dark ? mix(p, '#FFFFFF', 0.35) : mix(p, '#000000', 0.3),
    '--nav': dark ? mix(s, '#0B0F15', 0.55) : s,
    '--nav-deep': mix(s, '#000000', dark ? 0.7 : 0.25),
    '--nav-ink': inkFor(s),
    '--nav-muted': inkFor(s) === '#FFFFFF' ? mix(s, '#FFFFFF', 0.62) : mix(s, '#000000', 0.55),
    '--nav-hover': inkFor(s) === '#FFFFFF' ? mix(s, '#FFFFFF', 0.09) : mix(s, '#000000', 0.07),
    '--accent': s,
    '--accent-text': contrast(s, surface) >= 4.5 ? s : mix(s, dark ? '#FFFFFF' : '#000000', 0.45),
    '--accent-soft': mix(s, surface, dark ? 0.8 : 0.92),
  };
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', s);
}
