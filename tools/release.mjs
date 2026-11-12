#!/usr/bin/env node
// Publishes a release in one step:  npm run release
// Checks the version numbers agree and the public keys are set, commits,
// tags v<version> and pushes. GitHub then builds Windows and Mac installers
// and publishes the release (installed apps pick it up as an update).
import { readFileSync } from 'node:fs';
import { execSync } from 'node:child_process';
import { join } from 'node:path';
import { ROOT, PUBLIC_KEY_FILE, TAURI_CONF } from './common.mjs';

const run = (c) => execSync(c, { cwd: ROOT, stdio: 'inherit' });
const out = (c) => execSync(c, { cwd: ROOT }).toString().trim();
const fail = (m) => { console.error('✘ ' + m); process.exit(1); };

const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8')).version;
const conf = JSON.parse(readFileSync(TAURI_CONF, 'utf8'));
const cargo = readFileSync(join(ROOT, 'src-tauri', 'Cargo.toml'), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (pkg !== conf.version || pkg !== cargo) fail(`Versions differ: package.json ${pkg}, tauri.conf.json ${conf.version}, Cargo.toml ${cargo}. Make them the same.`);
if (readFileSync(PUBLIC_KEY_FILE, 'utf8').trim() === 'UNSET') fail('Licence public key not set. Run: npm run setup-keys');
if (!conf.plugins?.updater?.pubkey || conf.plugins.updater.pubkey === 'UNSET') fail('Update public key not set. Run: npm run setup-keys');

// First release from this folder: make it a git repository linked to GitHub.
const REPO = process.env.FARMBUDDY_REPO || 'https://github.com/treasure155/farmbuddy.git';
let isRepo = true;
try { out('git rev-parse --is-inside-work-tree'); } catch { isRepo = false; }
if (!isRepo) {
  console.log('• First release: creating the git repository');
  run('git init');
  run('git checkout -B main');
}
let remote = '';
try { remote = out('git remote get-url origin'); } catch { /* none yet */ }
if (!remote) { run(`git remote add origin ${REPO}`); console.log(`• GitHub repository: ${REPO}`); }

const tag = `v${pkg}`;
if (out(`git tag --list ${tag}`)) fail(`${tag} was already released. Raise the version in package.json, src-tauri/tauri.conf.json and src-tauri/Cargo.toml first.`);
run('git add -A');
try { run(`git commit -m "Release ${tag}"`); } catch { console.log('• Nothing new to commit'); }
const branch = out('git rev-parse --abbrev-ref HEAD');
run(`git tag -a ${tag} -m "FarmBuddy ${tag}"`);
run(`git push -u origin ${branch} ${tag}`);
console.log(`\n✔ Pushed ${tag}. Watch the build: GitHub → Actions. The release appears under Releases when both installers are done.`);
