#!/usr/bin/env node
// Compares the English and Ibibio dictionaries.
//
//   npm run translation:check            report missing Ibibio keys (always exits 0)
//   npm run translation:check -- --strict exit 1 if any key is missing (use before a release)
//
// Keys in ib.ts that do not exist in en.ts are always an error: they are typos
// or leftovers and would never be shown.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'lib', 'i18n');
const keysOf = (file) => {
  const src = readFileSync(join(root, file), 'utf8');
  const body = src.slice(src.indexOf('{', src.indexOf('export const')));
  return new Set([...body.matchAll(/^\s*["']([\w.]+)["']\s*:/gm)].map((m) => m[1]));
};

const en = keysOf('en.ts');
const ib = keysOf('ib.ts');
const missing = [...en].filter((k) => !ib.has(k));
const unknown = [...ib].filter((k) => !en.has(k));
const strict = process.argv.includes('--strict');

console.log(`English keys: ${en.size}`);
console.log(`Ibibio keys:  ${ib.size} (${Math.round((ib.size - unknown.length) * 100 / Math.max(1, en.size))}% approved)`);
if (missing.length) {
  console.log(`\n${missing.length} key(s) still need an approved Ibibio translation. First ${Math.min(25, missing.length)}:`);
  for (const k of missing.slice(0, 25)) console.log(`  - ${k}`);
}
if (unknown.length) {
  console.error(`\nError: ${unknown.length} key(s) in ib.ts do not exist in en.ts:`);
  for (const k of unknown) console.error(`  - ${k}`);
}
if (unknown.length || (strict && missing.length)) process.exit(1);
