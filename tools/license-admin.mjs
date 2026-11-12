#!/usr/bin/env node
// Issues a licence key for ONE computer. Only works on the admin's computer,
// because it needs the private key made by `npm run setup-keys`.
//
//   npm run license -- issue ABCD-EFGH-JKMN-PQRS               (never expires)
//   npm run license -- issue ABCD-EFGH-JKMN-PQRS --days 365 --note "Dearborn Farms, Etinan"
//   npm run license -- list
import { existsSync, readFileSync, appendFileSync } from 'node:fs';
import { createPrivateKey, sign } from 'node:crypto';
import { LICENSE_PRIVATE, ISSUED_LOG, b32encode, b32decode, dayNumber, dayToDate } from './common.mjs';

const [cmd, ...rest] = process.argv.slice(2);
const flag = (name) => { const i = rest.indexOf(name); return i >= 0 ? rest[i + 1] : undefined; };

function usage() {
  console.log('Usage:\n  npm run license -- issue <INSTALL-ID> [--days N] [--note "customer"]\n  npm run license -- list');
  process.exit(1);
}

if (cmd === 'list') {
  console.log(existsSync(ISSUED_LOG) ? readFileSync(ISSUED_LOG, 'utf8') : 'No keys issued yet.');
  process.exit(0);
}
if (cmd !== 'issue' || !rest[0]) usage();

if (!existsSync(LICENSE_PRIVATE)) {
  console.error(`✘ No licence signing key at ${LICENSE_PRIVATE}. Run "npm run setup-keys" first (admin computer only).`);
  process.exit(1);
}
const installId = rest[0].toUpperCase().replace(/[^0-9A-Z]/g, '');
let install;
try { install = b32decode(installId); } catch (e) { console.error(`✘ Install ID: ${e.message}`); process.exit(1); }
if (installId.length !== 16 || install.length !== 10) { console.error('✘ An Install ID has 16 characters, like ABCD-EFGH-JKMN-PQRS'); process.exit(1); }

const days = flag('--days') ? Number(flag('--days')) : 0;
if (!Number.isInteger(days) || days < 0 || days > 36500) { console.error('✘ --days must be a whole number of days'); process.exit(1); }
const note = (flag('--note') || '').replace(/[\r\n,]/g, ' ');
const today = dayNumber(new Date());
const expiry = days ? today + days : 0;

const payload = Buffer.alloc(16);
payload[0] = 1;                          // key format version
Buffer.from(install).copy(payload, 1);   // which computer
payload.writeUInt16BE(today, 11);        // issued
payload.writeUInt16BE(expiry, 13);       // expires (0 = never)
payload[15] = 1;                         // edition: standard
const signature = sign(null, payload, createPrivateKey(readFileSync(LICENSE_PRIVATE)));
const body = b32encode(Buffer.concat([payload, signature]));
const key = 'FMBD-' + body.match(/.{1,8}/g).join('-');

const pretty = installId.match(/.{4}/g).join('-');
appendFileSync(ISSUED_LOG, (existsSync(ISSUED_LOG) ? '' : 'issued,install_id,expires,note,key\n') + `${dayToDate(today)},${pretty},${expiry ? dayToDate(expiry) : 'never'},${note},${key}\n`);
console.log(`\nInstall ID : ${pretty}\nExpires    : ${expiry ? dayToDate(expiry) : 'never'}\n\n${key}\n`);
console.log('Send the key above to the customer. It only works on the computer with that Install ID.');
