import '@fontsource/plus-jakarta-sans/400.css';
import '@fontsource/plus-jakarta-sans/500.css';
import '@fontsource/plus-jakarta-sans/600.css';
import '@fontsource/plus-jakarta-sans/700.css';
import '@fontsource/plus-jakarta-sans/800.css';
import '@fontsource/fraunces/500.css';
import '@fontsource/fraunces/600.css';
import '@fontsource/courier-prime/400.css';
import '@fontsource/courier-prime/700.css';
import './styles/base.css';
import './styles/app.css';
import './styles/views.css';
import './styles/print.css';
import './styles/v2.css';
import './styles/mobile.css';
import './styles/mobile-fixes.css';
import './styles/app2.css';
import './styles/farm.css';
import './styles/fb.css';
import './lib/insets';
import './lib/enhance';

import { auth } from './api';
import { esc, toast } from './lib/dom';
import { licence, renderLicenceScreen } from './lib/license-ui';
import { autoUpdate } from './lib/app-update';
import { dailyLicenceReminder } from './lib/licence-reminder';
import { applyTheme } from './lib/theme';
import { startIbibioScreens } from './lib/i18n/ib-dom';
import { store } from './store';
import type { SessionInfo } from './types';
import { renderAuth } from './views/auth';
import { mountShell } from './views/shell';

const app = document.querySelector<HTMLDivElement>('#app')!;
let shell: ReturnType<typeof mountShell> | null = null;

async function signedIn(session: SessionInfo) {
  auth.set(session);
  await Promise.all([store.loadSettings(), store.loadTemplates(), store.loadPaymentMethods(), store.loadCategories()]);
  applyTheme(store.settings.theme_primary, store.settings.theme_secondary, store.settings.theme_mode);
  startIbibioScreens();
  // A new owner starts with the guided farm setup; everyone else starts at Home.
  const start = auth.can('owner') && store.settings.onboarding_done !== '1' ? 'onboarding' : 'home';
  shell = mountShell(app, signOut, start);
  showTrialBanner();
  // One popup at a time: today's licence reminder first, then any new version.
  dailyLicenceReminder().then(() => autoUpdate()).catch(() => undefined);
}

function showTrialBanner() {
  const s = licence.status;
  if (!shell || !s || s.state !== 'trial') return;
  const left = `Free trial: <b>${s.trial_days_left} day${s.trial_days_left === 1 ? '' : 's'} left</b>.`;
  shell.setBanner('trial', auth.can('owner')
    ? `<span>${left} Activate any time to keep using FarmBuddy after the trial.</span><button class="btn btn-sm btn-primary" data-banner-go="settings" data-section="licence">Activate</button>`
    : `<span>${left} Ask the owner to activate FarmBuddy.</span>`);
}

function signOut() {
  shell?.dispose();
  shell = null;
  auth.set(null);
  store.products = [];
  document.querySelectorAll('.modal-wrap').forEach((m) => m.remove());
  document.body.classList.remove('has-modal');
  start();
}

auth.onLicenceRequired(async () => {
  const s = await licence.refresh();
  if (s && s.state === 'expired') { shell?.dispose(); shell = null; auth.set(null); document.querySelectorAll('.modal-wrap').forEach((m) => m.remove()); document.body.classList.remove('has-modal'); renderLicenceScreen(app, s, start); }
});

auth.onExpired(() => {
  if (!auth.session) return;
  toast('Your session ended. Sign in again to continue.', 'info');
  signOut();
});

async function start() {
  try {
    const lic = await licence.refresh();
    if (lic && lic.state === 'expired') { renderLicenceScreen(app, lic, start); return; }
    await renderAuth(app, signedIn);
  } catch (e) {
    app.innerHTML = `<div class="fatal"><h2>The local database could not be opened</h2><p>${esc(e instanceof Error ? e.message : e)}</p><p>Close and reopen the app. If this keeps happening, check that your user account can write to the app data folder.</p></div>`;
  }
}

// Disable the webview's default context menu outside text fields for a native feel.
document.addEventListener('contextmenu', (e) => {
  if (!(e.target as HTMLElement).closest('input, textarea, [contenteditable]')) e.preventDefault();
});

start();
