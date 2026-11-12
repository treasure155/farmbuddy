#!/usr/bin/env node
// Checks a FarmBuddy content package before it is published, using the same
// rules the app applies on import, plus the advisory safety rules:
// - every article, schedule, guide and outlook names its source;
// - no article text gives medicine doses or chemical application rates;
// - review status is only "approved" with a named reviewer and date.
//
//   npm run content:check -- content/package.json [more.json ...]
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const SAFETY = ['general', 'record_keeping', 'planning', 'professional_confirmation', 'vet_only', 'chemical_label'];
const DOSE = /\b\d+(\.\d+)?\s*(mg|ml|g|l|litres?|cc)\s*(\/|per)\s*(kg|bird|animal|litre|l|ha|hectare|acre)\b/i;
const isDate = (s) => /^\d{4}-\d{2}-\d{2}$/.test(s) && !Number.isNaN(Date.parse(s));

export function checkPackage(raw) {
  let pkg = raw;
  const p = [];
  if (pkg && typeof pkg.payload === 'string') { try { pkg = JSON.parse(pkg.payload); } catch { return ['payload is not valid JSON']; } }
  else if (pkg && typeof pkg.payload === 'object') pkg = pkg.payload;
  if (!pkg?.package_id) p.push('package_id is missing');
  if (!Number.isInteger(pkg?.version) || pkg.version < 1) p.push('version must be a whole number from 1');
  if (pkg?.published_date && !isDate(pkg.published_date)) p.push('published_date must be YYYY-MM-DD');
  const slugs = new Set();
  for (const a of pkg?.articles ?? []) {
    const id = `article "${a.slug ?? a.title ?? '?'}"`;
    if (!a.slug || !/^[a-z0-9-]+$/.test(a.slug)) p.push(`${id}: slug must be lowercase letters, numbers and dashes`);
    if (slugs.has(a.slug)) p.push(`${id}: duplicate slug`); slugs.add(a.slug);
    if (!a.title?.trim() || !a.body?.trim()) p.push(`${id}: title and body are required`);
    if (!a.source?.trim()) p.push(`${id}: source is required`);
    if (a.safety_level && !SAFETY.includes(a.safety_level)) p.push(`${id}: unknown safety_level ${a.safety_level}`);
    if (a.language && !['en', 'ib'].includes(a.language)) p.push(`${id}: language must be en or ib`);
    if (a.review_status === 'approved' && (!a.reviewer?.trim() || !isDate(a.review_date ?? ''))) p.push(`${id}: approved content needs a reviewer and review_date`);
    if (DOSE.test(a.body ?? '')) p.push(`${id}: body looks like it gives a dose or application rate; refer to the vet or the product label instead`);
  }
  for (const s of pkg?.protocols ?? []) {
    const id = `schedule "${s.name ?? '?'}"`;
    if (!s.name?.trim() || !s.species?.trim() || !s.source_text?.trim()) p.push(`${id}: name, species and source_text are required`);
    if (!Array.isArray(s.items) || !s.items.length) p.push(`${id}: needs at least one vaccination`);
    for (const i of s.items ?? []) if (!i.vaccine_name?.trim() || !(i.day_offset >= 0)) p.push(`${id}: every item needs vaccine_name and day_offset ≥ 0`);
  }
  for (const g of pkg?.feed_guides ?? []) {
    if (!g.source?.trim() || !(g.grams_per_animal_day > 0) || !(g.age_to_day >= g.age_from_day)) p.push(`feed guide for ${g.species ?? '?'}: needs source, grams_per_animal_day > 0 and a valid age range`);
  }
  for (const o of pkg?.outlooks ?? []) {
    if (!o.source?.trim() || !o.state?.trim() || !(o.year >= 2000)) p.push(`outlook ${o.state ?? '?'} ${o.year ?? ''}: needs year, state and source`);
    for (const d of ['onset_date', 'cessation_date', 'published_date']) if (o[d] && !isDate(o[d])) p.push(`outlook ${o.state} ${o.year}: ${d} must be YYYY-MM-DD`);
  }
  return p;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const files = process.argv.slice(2);
  if (!files.length) { console.error('Usage: npm run content:check -- <package.json> [...]'); process.exit(1); }
  let bad = 0;
  for (const f of files) {
    const problems = checkPackage(JSON.parse(readFileSync(f, 'utf8')));
    if (problems.length) { bad++; console.log(`✗ ${f}\n  - ${problems.join('\n  - ')}`); } else console.log(`✓ ${f}`);
  }
  process.exit(bad ? 1 : 0);
}
