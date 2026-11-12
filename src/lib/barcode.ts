/** Barcode drawing for shelf and product labels: EAN-13 for 13-digit codes, Code 128 for anything else. */

const L = ['0001101', '0011001', '0010011', '0111101', '0100011', '0110001', '0101111', '0111011', '0110111', '0001011'];
const G = ['0100111', '0110011', '0011011', '0100001', '0011101', '0111001', '0000101', '0010001', '0001001', '0010111'];
const R = ['1110010', '1100110', '1101100', '1000010', '1011100', '1001110', '1010000', '1000100', '1001000', '1110100'];
const PARITY = ['LLLLLL', 'LLGLGG', 'LLGGLG', 'LLGGGL', 'LGLLGG', 'LGGLLG', 'LGGGLL', 'LGLGLG', 'LGLGGL', 'LGGLGL'];

export function ean13Valid(code: string): boolean {
  if (!/^\d{13}$/.test(code)) return false;
  const sum = code.slice(0, 12).split('').reduce((a, c, i) => a + Number(c) * (i % 2 ? 3 : 1), 0);
  return (10 - (sum % 10)) % 10 === Number(code[12]);
}

/** Module string (1 = bar) for an EAN-13 code. */
export function ean13Modules(code: string): string {
  const first = Number(code[0]);
  let bits = '101';
  for (let i = 1; i <= 6; i++) bits += (PARITY[first][i - 1] === 'L' ? L : G)[Number(code[i])];
  bits += '01010';
  for (let i = 7; i <= 12; i++) bits += R[Number(code[i])];
  return bits + '101';
}

// Code 128 bar/space widths for values 0–106 (106 = stop).
const C128 = ('212222 222122 222221 121223 121322 131222 122213 122312 132212 221213 221312 231212 112232 122132 122231 113222 123122 123221 223211 221132 ' +
  '221231 213212 223112 312131 311222 321122 321221 312212 322112 322211 212123 212321 232121 111323 131123 131321 112313 132113 132311 211313 ' +
  '231113 231311 112133 112331 132131 113123 113321 133121 313121 211331 231131 213113 213311 213131 311123 311321 331121 312113 312311 332111 ' +
  '314111 221411 431111 111224 111422 121124 121421 141122 141221 112214 112412 122114 122411 142112 142211 241211 221114 413111 241112 134111 ' +
  '111242 121142 121241 114212 124112 124211 411212 421112 421211 212141 214121 412121 111143 111341 131141 114113 114311 411113 411311 113141 ' +
  '114131 311141 411131 211412 211214 211232 2331112').split(' ');

/** Module string for Code 128 set B (printable ASCII). */
export function code128Modules(text: string): string {
  const values = [104];
  for (const ch of text) {
    const c = ch.charCodeAt(0);
    if (c < 32 || c > 126) throw new Error('Barcode text must be plain letters, numbers and symbols');
    values.push(c - 32);
  }
  const check = values.reduce((a, v, i) => a + v * (i === 0 ? 1 : i), 0) % 103;
  values.push(check, 106);
  return values.map((v) => C128[v].split('').map((w, i) => (i % 2 ? '0' : '1').repeat(Number(w))).join('')).join('');
}

/** An SVG barcode sized in millimetres (bars scale to fit the width). */
export function barcodeSvg(code: string, opts: { widthMm?: number; heightMm?: number; showText?: boolean } = {}): string {
  const isEan = ean13Valid(code);
  const bits = isEan ? ean13Modules(code) : code128Modules(code);
  const quiet = isEan ? 9 : 10;
  const modules = bits.length + quiet * 2;
  const widthMm = opts.widthMm ?? Math.max(28, modules * 0.3);
  const unit = widthMm / modules;                       // mm per module
  const barH = (opts.heightMm ?? 14) / unit;            // in modules
  const textH = opts.showText === false ? 0 : 3.4 / unit;
  let rects = '';
  for (let i = 0; i < bits.length;) {
    if (bits[i] !== '1') { i++; continue; }
    let j = i;
    while (j < bits.length && bits[j] === '1') j++;
    // EAN guard bars run a little longer, as on retail packaging.
    const guard = isEan && (i < 3 || (i >= 45 && i < 50) || i >= 92);
    rects += `<rect x="${quiet + i}" y="0" width="${j - i}" height="${(guard ? barH + textH * 0.5 : barH).toFixed(2)}"/>`;
    i = j;
  }
  const safe = code.replace(/[<&>"]/g, '');
  const text = opts.showText === false ? '' : `<text x="${modules / 2}" y="${(barH + textH * 0.85).toFixed(2)}" font-size="${(2.6 / unit).toFixed(2)}" text-anchor="middle" font-family="Courier Prime, monospace">${safe}</text>`;
  const totalH = barH + textH;
  return `<svg class="barcode" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${modules} ${totalH.toFixed(2)}" width="${widthMm.toFixed(2)}mm" height="${(totalH * unit).toFixed(2)}mm" role="img" aria-label="Barcode ${safe}"><g fill="#000">${rects}</g>${text}</svg>`;
}
