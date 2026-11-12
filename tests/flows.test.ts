import { describe, expect, it } from 'vitest';
import { globSync, readFileSync } from 'node:fs';

// Every command the UI calls must be registered in the Rust command list.
describe('backend command registry', () => {
  it('registers every command the frontend calls', () => {
    const lib = readFileSync('src-tauri/src/lib.rs', 'utf8');
    const handler = lib.slice(lib.indexOf('generate_handler!['), lib.indexOf('])', lib.indexOf('generate_handler![')));
    const registered = new Set([...handler.matchAll(/(\w+)\s*[,\]]/g)].map((m) => m[1]));
    const files = globSync('src/**/*.ts');
    expect(files.length).toBeGreaterThan(20);
    const called = new Set<string>();
    for (const f of files) for (const m of readFileSync(f, 'utf8').matchAll(/call(?:<[^>]*>)?\('([a-z_]+)'/g)) called.add(m[1]);
    const missing = [...called].filter((c) => !registered.has(c) && !c.startsWith('plugin'));
    expect(missing).toEqual([]);
  });
});
