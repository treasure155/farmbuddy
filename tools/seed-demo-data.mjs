#!/usr/bin/env node
// Loads the FarmBuddy demo farm into a database file (for training, demos and screenshots).
//
//   npm run seed:demo                         -> the installed app's database on this computer
//   npm run seed:demo -- --db ./demo.sqlite3  -> any database file (created if missing)
//   FB_OWNER_PASSWORD=secret123 npm run seed:demo -- --db ./demo.sqlite3
//                                             -> also creates the owner account "owner" in a new file
//
// Close FarmBuddy before seeding its own database. Demo records are marked Demo,
// excluded from reports by default, and removable from My Farm → Remove demo data.
import { spawnSync } from 'node:child_process';
import { homedir, platform } from 'node:os';
import { join, resolve } from 'node:path';

const ID = 'online.techalphahub.farmbuddy';
const arg = process.argv.indexOf('--db');
const defaultDb = () => {
  const p = platform();
  if (p === 'win32') return join(process.env.APPDATA ?? join(homedir(), 'AppData', 'Roaming'), ID, 'farmbuddy.sqlite3');
  if (p === 'darwin') return join(homedir(), 'Library', 'Application Support', ID, 'farmbuddy.sqlite3');
  return join(process.env.XDG_DATA_HOME ?? join(homedir(), '.local', 'share'), ID, 'farmbuddy.sqlite3');
};
const db = resolve(arg > -1 ? process.argv[arg + 1] : defaultDb());
console.log(`Seeding demo data into ${db}`);
const r = spawnSync('cargo', ['test', '--manifest-path', 'src-tauri/Cargo.toml', '--lib', 'seed_demo_db', '--', '--ignored', '--nocapture'], { stdio: 'inherit', env: { ...process.env, FB_DB: db }, shell: platform() === 'win32' });
process.exit(r.status ?? 1);
