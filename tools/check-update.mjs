#!/usr/bin/env node
// Finds out why an update says "signature verification failed".
// Compares: the public key in this project, the public key inside the
// installed app, the key that signed the published update, and the file itself.
//
//   npm run check-update
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { createHash, createPublicKey, verify } from 'node:crypto';
import { join } from 'node:path';
import { TAURI_CONF, UPDATER_KEY } from './common.mjs';

const b64 = (s) => Buffer.from(s.trim(), 'base64');
const lines = (buf) => buf.toString('utf8').split(/\r?\n/).filter(Boolean);

function parsePub(text) {
  const raw = b64(lines(b64(text))[1]);            // 'Ed' + keyid(8) + pk(32)
  return { id: raw.subarray(2, 10).toString('hex').toUpperCase(), pk: raw.subarray(10, 42) };
}
function parseSig(text) {
  const l = lines(b64(text));
  const raw = b64(l[1]);                           // alg(2) + keyid(8) + sig(64)
  return { alg: raw.subarray(0, 2).toString(), id: raw.subarray(2, 10).toString('hex').toUpperCase(), sig: raw.subarray(10, 74), trusted: l[2] ?? '' };
}
const ed = (pk) => createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), pk]), format: 'der', type: 'spki' });

const conf = JSON.parse(readFileSync(TAURI_CONF, 'utf8'));
const projectPub = parsePub(conf.plugins.updater.pubkey);
console.log(`Project public key ID      : ${projectPub.id}`);
if (existsSync(UPDATER_KEY + '.pub')) console.log(`~/.farmbuddy-keys public key ID: ${parsePub(readFileSync(UPDATER_KEY + '.pub', 'utf8')).id}`);

// Public key baked into the installed app (macOS / Windows default locations).
const candidates = ['/Applications/FarmBuddy.app/Contents/MacOS'];
if (process.env.LOCALAPPDATA) candidates.push(join(process.env.LOCALAPPDATA, 'FarmBuddy'));
if (process.env.ProgramFiles) candidates.push(join(process.env.ProgramFiles, 'FarmBuddy'));
for (const dir of candidates) {
  if (!existsSync(dir)) continue;
  for (const f of readdirSync(dir)) {
    if (!/(farmbuddy)/i.test(f)) continue;
    const bin = readFileSync(join(dir, f)).toString('latin1');
    const m = bin.match(/dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6[A-Za-z0-9+/=]+/);
    if (m) console.log(`Installed app key ID       : ${parsePub(m[0]).id}   (${join(dir, f)})`);
  }
}

const url = conf.plugins.updater.endpoints[0];
const res = await fetch(url);
if (!res.ok) { console.log(`\nCould not download ${url} (${res.status}). Is the release published?`); process.exit(1); }
const latest = await res.json();
console.log(`\nPublished version          : ${latest.version}`);
const platform = process.platform === 'darwin' ? (process.arch === 'arm64' ? 'darwin-aarch64' : 'darwin-x86_64') : 'windows-x86_64';
const entry = latest.platforms?.[platform] ?? latest.platforms?.[`${platform}-app`] ?? latest.platforms?.[`${platform}-nsis`];
if (!entry) { console.log(`No entry for ${platform}. Platforms listed: ${Object.keys(latest.platforms ?? {}).join(', ')}`); process.exit(1); }
const sig = parseSig(entry.signature);
console.log(`Update signed with key ID   : ${sig.id}   (${platform})`);
console.log(`File                        : ${entry.url}`);

if (sig.id !== projectPub.id) {
  console.log('\n✘ The update was signed with a DIFFERENT key than the app trusts.');
  console.log('  Fix: put the right key in GitHub secrets (see README, Keys and releases) and release a new version.');
  process.exit(1);
}
const file = Buffer.from(await (await fetch(entry.url)).arrayBuffer());
const msg = sig.alg === 'ED' ? createHash('blake2b512').update(file).digest() : file;
const ok = verify(null, msg, ed(projectPub.pk), sig.sig);
console.log(ok ? '\n✔ Signature matches the file. Updates should install.' : '\n✘ Same key, but the signature does not match the file on GitHub (the file was replaced after signing).');
console.log(ok ? '' : '  Fix: release a new version number; do not re-run an already published release.');
