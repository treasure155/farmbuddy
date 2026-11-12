/**
 * Updates that show up by themselves.
 *
 * Windows / Mac: Tauri's signed updater. The new version is downloaded quietly
 * in the background, then the user is asked to restart.
 * Android: the app asks for the newest release, the phone's own download
 * manager fetches the APK (with a notification), and Android's installer opens.
 * Nobody is sent to a web page.
 */
import type { Update } from '@tauri-apps/plugin-updater';
import { $, esc, modal } from './dom';
import { icon } from './icons';
import { isMobile, openExternal } from './platform';
import { appVersion, checkForUpdate } from './updater';

type ApkUpdate = { version: string; notes: string; url: string; file_name: string; size: number };

type UpdateBridge = {
  canInstallUpdates?: () => string;
  askInstallPermission?: () => string;
  downloadUpdate?: (url: string, name: string, title: string) => string;
  updateProgress?: (id: string) => string;
  installUpdate?: (name: string) => string;
};

export type AvailableUpdate = { version: string; notes: string; size: number; desktop?: Update; apk?: ApkUpdate };

const bridge = (): UpdateBridge | undefined => (window as unknown as { EstockNative?: UpdateBridge }).EstockNative;
const sleep = (ms: number) => new Promise((r) => window.setTimeout(r, ms));

/** The newer version, or null when this is the newest. Throws when offline. */
export async function findUpdate(): Promise<AvailableUpdate | null> {
  if (isMobile) {
    const { invoke } = await import('@tauri-apps/api/core');
    const u = await invoke<ApkUpdate | null>('check_apk_update');
    return u ? { version: u.version, notes: u.notes, size: u.size, apk: u } : null;
  }
  const u = await checkForUpdate();
  return u ? { version: u.version, notes: u.body ?? '', size: 0, desktop: u } : null;
}

// ------------------------------------------------------------------ desktop

let downloaded: Update | null = null;

async function downloadDesktop(u: Update, progress: (pct: number | null) => void) {
  if (downloaded === u) { progress(100); return; }
  let total = 0, done = 0;
  await u.download((ev) => {
    if (ev.event === 'Started') { total = ev.data.contentLength ?? 0; progress(total ? 0 : null); }
    if (ev.event === 'Progress') { done += ev.data.chunkLength; progress(total ? Math.min(100, Math.round((done / total) * 100)) : null); }
    if (ev.event === 'Finished') progress(100);
  });
  downloaded = u;
}

async function restartDesktop(u: Update) {
  await u.install();
  const { relaunch } = await import('@tauri-apps/plugin-process');
  await relaunch();
}

// ------------------------------------------------------------------ android

/** Waits until the user comes back to the app (e.g. from Android settings). */
const backToApp = () => new Promise<void>((resolve) => {
  const on = () => { if (document.visibilityState === 'visible') { document.removeEventListener('visibilitychange', on); resolve(); } };
  document.addEventListener('visibilitychange', on);
});

async function installApk(u: ApkUpdate, status: (text: string, pct: number | null) => void) {
  const n = bridge();
  if (!n?.downloadUpdate || !n.updateProgress || !n.installUpdate) {
    // Older build without the download helper: hand the APK file itself to Android.
    status('Opening the download…', null);
    await openExternal(u.url);
    return;
  }
  const id = n.downloadUpdate(u.url, u.file_name, `FarmBuddy ${u.version}`);
  if (id.startsWith('ERR:')) throw new Error(id.slice(4));
  status('Downloading… 0%', 0);
  for (;;) {
    await sleep(700);
    const [state, pctText] = n.updateProgress(id).split(':');
    const pct = Number(pctText) || 0;
    if (state === 'failed') throw new Error('The download stopped. Check the internet connection and try again.');
    if (state === 'done') break;
    status(`Downloading… ${pct}%`, pct);
  }
  status('Download complete', 100);
  if (n.canInstallUpdates?.() === '0') {
    status('Allow FarmBuddy to install updates, then come back to the app.', 100);
    n.askInstallPermission?.();
    await backToApp();
    await sleep(400);
  }
  const err = n.installUpdate(u.file_name);
  if (err) throw new Error(err);
  status('Tap Update on the screen that opened. Your data stays on the phone.', 100);
}

// ------------------------------------------------------------------ the popup

const mb = (bytes: number) => (bytes ? `${(bytes / 1048576).toFixed(1)} MB` : '');

/** Shows the update and installs it when the user agrees. Resolves when closed. */
export function showUpdatePrompt(u: AvailableUpdate): Promise<void> {
  return new Promise((resolve) => {
    const where = isMobile ? 'phone' : 'computer';
    const m = modal({
      title: 'A new version is ready',
      size: 'sm',
      onClose: () => resolve(),
      body: `<div class="stack update-prompt">
        <div class="update-head">${icon('refresh', 26)}<div><b>FarmBuddy ${esc(u.version)}</b><span class="muted small" data-cur></span></div></div>
        ${u.notes ? `<div class="update-notes pre-wrap">${esc(u.notes)}</div>` : ''}
        <div class="update-progress" data-progress hidden><div class="bar"><i data-bar></i></div><span class="small" data-status></span></div>
        <p class="muted small">Your sales, stock and settings stay on this ${where}.${u.size ? ` Download size: ${mb(u.size)}.` : ''}</p>
        <p class="form-error" role="alert" data-err></p>
        <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>Later</button><button type="button" class="btn btn-primary" data-go>${icon('download', 16)} ${!isMobile && downloaded === u.desktop ? 'Restart and update' : 'Update now'}</button></div>
      </div>`,
    });
    appVersion().then((v) => { const el = $('[data-cur]', m.el); if (el && v) el.textContent = `You have ${v}`; });
    const go = $<HTMLButtonElement>('[data-go]', m.el)!;
    const prog = $('[data-progress]', m.el)!, bar = $<HTMLElement>('[data-bar]', m.el)!, st = $('[data-status]', m.el)!, err = $('[data-err]', m.el)!;
    const status = (text: string, pct: number | null) => { prog.hidden = false; st.textContent = text; bar.style.width = `${pct ?? 30}%`; bar.classList.toggle('busy', pct === null); };
    go.addEventListener('click', async () => {
      go.disabled = true; err.textContent = '';
      try {
        if (u.apk) {
          await installApk(u.apk, status);
          go.disabled = false; go.innerHTML = `${icon('download', 16)} Install again`;
        } else if (u.desktop) {
          await downloadDesktop(u.desktop, (pct) => status(pct === null ? 'Downloading…' : pct >= 100 ? 'Downloaded' : `Downloading… ${pct}%`, pct));
          status('Installing. The app will restart…', 100);
          await restartDesktop(u.desktop);
        }
      } catch (x) {
        err.textContent = x instanceof Error ? x.message : String(x);
        go.disabled = false;
      }
    });
  });
}

let promptedThisSession = false;

/**
 * Called after sign-in. Looks for a newer version and, if there is one, shows it.
 * On computers the file is downloaded first, so "Restart and update" is instant.
 * Quiet when offline.
 */
export async function autoUpdate(): Promise<void> {
  if (promptedThisSession) return;
  let u: AvailableUpdate | null = null;
  try { u = await findUpdate(); } catch { return; }
  if (!u) return;
  if (u.desktop) {
    try { await downloadDesktop(u.desktop, () => undefined); } catch { /* the popup downloads again */ }
  }
  promptedThisSession = true;
  // Never interrupt a sale or another popup: wait until nothing else is open.
  for (let i = 0; i < 120 && document.querySelector('.modal-wrap'); i++) await sleep(1000);
  await showUpdatePrompt(u);
}

/** For Settings → Updates. */
export async function checkNow(out: HTMLElement): Promise<void> {
  out.innerHTML = '<p class="muted small">Checking…</p>';
  try {
    const u = await findUpdate();
    if (!u) { out.innerHTML = '<p class="ok-text"><b>You have the newest version.</b></p>'; return; }
    out.innerHTML = `<div class="callout callout-info">${icon('refresh', 18)}<div><b>FarmBuddy ${esc(u.version)} is available</b><div><button class="btn btn-primary" data-open-update>${icon('download', 16)} Update now</button></div></div></div>`;
    $('[data-open-update]', out)!.addEventListener('click', () => { showUpdatePrompt(u!); });
  } catch (x) {
    out.innerHTML = `<p class="muted small">Could not check for updates (${esc(x instanceof Error ? x.message : String(x))}). Check the internet connection and try again.</p>`;
  }
}

