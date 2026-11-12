// Shared helpers for the admin tools. These run on the admin's computer only.
import { homedir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
export const KEY_DIR = process.env.FARMBUDDY_KEY_DIR || join(homedir(), '.farmbuddy-keys');
export const CONTENT_KEY = join(KEY_DIR, 'content-signing-key.txt');
export const LICENSE_PRIVATE = join(KEY_DIR, 'license-private.pem');
export const UPDATER_KEY = join(KEY_DIR, 'updater.key');
export const UPDATER_PASSWORD = join(KEY_DIR, 'updater-password.txt');
export const ISSUED_LOG = join(KEY_DIR, 'issued-licences.csv');
export const ANDROID_KEYSTORE = join(KEY_DIR, 'android-release.p12');
export const ANDROID_PASSWORD = join(KEY_DIR, 'android-keystore-password.txt');
export const PUBLIC_KEY_FILE = join(ROOT, 'src-tauri', 'license_public_key.txt');
export const TAURI_CONF = join(ROOT, 'src-tauri', 'tauri.conf.json');

// Crockford base32, identical to src-tauri/src/license.rs.
const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
export function b32encode(bytes) {
  let out = '', buf = 0, bits = 0;
  for (const b of bytes) {
    buf = (buf << 8) | b; bits += 8;
    while (bits >= 5) { bits -= 5; out += ALPHABET[(buf >> bits) & 31]; }
    buf &= (1 << bits) - 1;
  }
  if (bits > 0) out += ALPHABET[(buf << (5 - bits)) & 31];
  return out;
}
export function b32decode(s) {
  let buf = 0, bits = 0; const out = [];
  for (let c of s.toUpperCase()) {
    if (c === 'O') c = '0'; if (c === 'I' || c === 'L') c = '1';
    const v = ALPHABET.indexOf(c);
    if (v < 0) throw new Error(`Character "${c}" is not allowed`);
    buf = (buf << 5) | v; bits += 5;
    if (bits >= 8) { bits -= 8; out.push((buf >> bits) & 255); buf &= (1 << bits) - 1; }
  }
  return Uint8Array.from(out);
}
export const EPOCH_2024 = Date.UTC(2024, 0, 1);
export const DAY_MS = 86_400_000;
export const dayNumber = (date) => Math.floor((date.getTime() - EPOCH_2024) / DAY_MS);
export const dayToDate = (n) => new Date(EPOCH_2024 + n * DAY_MS).toISOString().slice(0, 10);
