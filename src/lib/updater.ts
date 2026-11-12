/** Automatic updates from the official GitHub releases (signed; unsigned files are refused). */
import type { Update } from '@tauri-apps/plugin-updater';

let pending: Update | null = null;

export async function appVersion(): Promise<string> {
  try { const { getVersion } = await import('@tauri-apps/api/app'); return await getVersion(); } catch { return ''; }
}

/** Returns the waiting update, or null when this is the newest version. Throws when offline. */
export async function checkForUpdate(): Promise<Update | null> {
  const { check } = await import('@tauri-apps/plugin-updater');
  pending = await check({ timeout: 20_000 });
  return pending;
}

export const pendingUpdate = () => pending;

/** Downloads, installs and restarts. `progress` receives 0–100 when the size is known. */
export async function installUpdate(update: Update, progress: (pct: number | null) => void) {
  let total = 0, done = 0;
  await update.downloadAndInstall((ev) => {
    if (ev.event === 'Started') { total = ev.data.contentLength ?? 0; progress(total ? 0 : null); }
    if (ev.event === 'Progress') { done += ev.data.chunkLength; progress(total ? Math.min(100, Math.round((done / total) * 100)) : null); }
    if (ev.event === 'Finished') progress(100);
  });
  const { relaunch } = await import('@tauri-apps/plugin-process');
  await relaunch();
}
