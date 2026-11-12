import { call } from '../api';
import type { LicenseStatus } from '../types';
import { $, errText, esc, modal, toast, withBusy } from './dom';
import { dateLabel } from './format';
import { icon } from './icons';
import { keyRequestText, VENDOR } from './vendor';
import { shareWhatsApp } from './whatsapp';
import { openExternal } from './platform';

let current: LicenseStatus | null = null;
export const licence = {
  get status() { return current; },
  async refresh(): Promise<LicenseStatus | null> {
    try { current = await call<LicenseStatus>('license_status'); } catch { current = null; }
    return current;
  },
};

const idBlock = (s: LicenseStatus) => `<div class="install-id">
  <span class="field-label">This computer's Install ID</span>
  <div class="install-id-row"><code data-iid>${esc(s.install_id)}</code><button type="button" class="btn btn-ghost btn-sm" data-copy>${icon('check', 15)} Copy</button></div>
  <small class="muted">Send this ID to ${esc(VENDOR.name)}. They will send back a licence key made for this computer only.</small>
</div>`;

/** How to buy a key: prices and one-tap WhatsApp / email with the Install ID already filled in. */
export const vendorBlock = () => `<div class="vendor-box">
  <div class="vendor-head"><b>Get a licence key from ${esc(VENDOR.name)}</b><span>${VENDOR.prices.map(esc).join(' · ')}</span></div>
  <div class="vendor-actions">
    <button type="button" class="btn btn-primary btn-sm" data-req-wa>${icon('chat', 15)} Request key on WhatsApp</button>
    <button type="button" class="btn btn-ghost btn-sm" data-req-mail>${icon('mail', 15)} Email us</button>
  </div>
  <small class="muted">WhatsApp ${esc(VENDOR.phoneDisplay)} · ${esc(VENDOR.email)}</small>
</div>`;

export function wireVendor(root: HTMLElement, installId: string) {
  const business = async () => {
    try { return (await call<{ business_name: string }>('branding')).business_name || ''; } catch { return ''; }
  };
  $('[data-req-wa]', root)?.addEventListener('click', async () => shareWhatsApp(VENDOR.whatsapp, keyRequestText(installId, await business())));
  $('[data-req-mail]', root)?.addEventListener('click', async () => {
    const url = `mailto:${VENDOR.email}?subject=${encodeURIComponent(`FarmBuddy licence key: ${installId}`)}&body=${encodeURIComponent(keyRequestText(installId, await business()))}`;
    await openExternal(url);
  });
}

const keyForm = (s: LicenseStatus) => `<form class="stack" data-activate>
  ${idBlock(s)}
  ${vendorBlock()}
  <label class="field"><span>Licence key</span><textarea name="key" rows="3" required spellcheck="false" autocapitalize="characters" placeholder="ESTK-XXXXXXXX-XXXXXXXX-…" class="mono-input"></textarea></label>
  ${s.can_activate ? '' : '<p class="form-error">This copy was built without the provider\'s public key and cannot be activated. Install the official version.</p>'}
  <p class="form-error" role="alert" data-err></p>
  <button class="btn btn-primary btn-lg btn-block" ${s.can_activate ? '' : 'disabled'}>${icon('key', 18)} Activate</button>
</form>`;

function wire(root: HTMLElement, done: (s: LicenseStatus) => void) {
  wireVendor(root, $('[data-iid]', root)?.textContent ?? '');
  $('[data-copy]', root)?.addEventListener('click', async () => {
    const id = $('[data-iid]', root)!.textContent ?? '';
    try { await navigator.clipboard.writeText(id); toast('Install ID copied'); }
    catch { const r = document.createRange(); r.selectNodeContents($('[data-iid]', root)!); getSelection()?.removeAllRanges(); getSelection()?.addRange(r); toast('Press Ctrl+C (or Cmd+C) to copy', 'info'); }
  });
  const form = $<HTMLFormElement>('[data-activate]', root)!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const err = $('[data-err]', form)!;
    err.textContent = '';
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        const s = await call<LicenseStatus>('activate_license', { key: (form.elements.namedItem('key') as HTMLTextAreaElement).value });
        current = s;
        toast(s.expires_on ? `Activated until ${dateLabel(s.expires_on)}` : 'FarmBuddy is activated. Thank you!');
        done(s);
      } catch (x) { err.textContent = errText(x); }
    });
  });
}

/** Full-page lock shown when the trial or a licence has ended. Data is kept safe; nothing is deleted. */
export function renderLicenceScreen(root: HTMLElement, s: LicenseStatus, onActivated: () => void) {
  root.innerHTML = `<main class="auth">
    <aside class="auth-side">
      <div class="auth-side-top"><img class="auth-logo" src="/logo-mark.svg" alt=""><span>FarmBuddy</span></div>
      <div class="auth-side-copy"><h1>Activate to keep selling</h1><p>Your sales, stock and customers are safe on this computer. Enter a licence key to carry on where you stopped.</p></div>
      <div></div>
    </aside>
    <section class="auth-main"><div class="auth-form stack">
      <div><h2>${s.state === 'expired' && s.expires_on ? 'Your licence has ended' : 'Your free trial has ended'}</h2><p class="muted">${esc(s.message)}</p></div>
      ${keyForm(s)}
    </div></section></main>`;
  wire(root, () => onActivated());
}

/** Activation dialog used from Settings and the trial banner. */
export function activationDialog(s: LicenseStatus, after?: (s: LicenseStatus) => void) {
  const m = modal({ title: 'Activate FarmBuddy', subtitle: s.state === 'trial' ? `${s.trial_days_left} day${s.trial_days_left === 1 ? '' : 's'} of the free trial left` : '', size: 'md', body: keyForm(s) });
  wire(m.el, (next) => { m.close(); after?.(next); });
}
