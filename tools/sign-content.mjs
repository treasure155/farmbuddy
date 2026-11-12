#!/usr/bin/env node
// Signs a FarmBuddy content package so the app accepts it with
// "Only accept signed packages" switched on.
//
//   npm run content:sign -- content/package.json [out.json]
//
// The input is the package itself ({ package_id, version, articles, ... }).
// The output wraps it as { payload: "<json>", signature: "<base64 Ed25519>" }.
// The private key is read from FB_CONTENT_KEY or secrets/content_signing_key.txt.
// Keep that key private: anyone holding it can publish content the app trusts.
import { createPrivateKey, sign } from 'node:crypto';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { checkPackage } from './content-check.mjs';
import { CONTENT_KEY } from './common.mjs';

const [input, output] = process.argv.slice(2);
if (!input) { console.error('Usage: npm run content:sign -- <package.json> [out.json]'); process.exit(1); }
const pkg = JSON.parse(readFileSync(input, 'utf8'));
const problems = checkPackage(pkg);
if (problems.length) { console.error(`Not signed. Fix these first:\n- ${problems.join('\n- ')}`); process.exit(1); }
const keyFile = [CONTENT_KEY, 'secrets/content_signing_key.txt'].find((f) => existsSync(f));
const keyText = process.env.FB_CONTENT_KEY || (keyFile ? readFileSync(keyFile, 'utf8') : '');
const seedB64 = keyText.split(/\r?\n/).map((l) => l.trim()).find((l) => l && !l.startsWith('#'));
if (!seedB64) { console.error('No signing key: set FB_CONTENT_KEY or add secrets/content_signing_key.txt'); process.exit(1); }
const seed = Buffer.from(seedB64, 'base64');
if (seed.length !== 32) { console.error('The signing key must be a base64 32-byte Ed25519 seed'); process.exit(1); }
// PKCS#8 wrapper for a raw Ed25519 seed.
const der = Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), seed]);
const key = createPrivateKey({ key: der, format: 'der', type: 'pkcs8' });
const payload = JSON.stringify(pkg);
const signature = sign(null, Buffer.from(payload, 'utf8'), key).toString('base64');
const out = output ?? input.replace(/\.json$/, '') + '.signed.json';
writeFileSync(out, JSON.stringify({ payload, signature }, null, 1));
console.log(`Signed ${pkg.package_id} v${pkg.version} → ${out}`);
