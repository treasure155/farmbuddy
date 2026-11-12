import { call } from '../api';
import { $, $$, errText, esc, formValues } from '../lib/dom';
import { icon } from '../lib/icons';
import { applyTheme } from '../lib/theme';
import { store } from '../store';
import type { Branding, SessionInfo } from '../types';
import { farmQuestions, type Answers } from './onboarding';
import { questionFields, readQuestions, showRecoveryKey, wireQuestionFields } from './recovery-ui';


type SignedIn = (s: SessionInfo) => void;
interface RecoveryOptions { key_set: boolean; questions: string[] }
interface RecoverResult { session: SessionInfo; new_recovery_key: string | null }

let branding: Branding | null = null;

function brandName() {
  return branding?.business_name && !['My Business', 'My Eatery', 'My Farm'].includes(branding.business_name) ? branding.business_name : 'FarmBuddy';
}

/** The left side of every sign-in screen: brand, a line of copy and a calm field illustration. */
function brandPanel(heading: string, text: string) {
  return `<aside class="auth-side">
    <div class="auth-side-top">
      <img class="auth-logo" src="${esc(branding?.business_logo || '/logo-mark.svg')}" alt="">
      <span class="auth-brand"><b>${esc(brandName())}</b><small>FarmBuddy · Offline farm manager</small></span>
    </div>
    <div class="auth-side-copy"><h1>${esc(heading)}</h1><p>${esc(text)}</p></div>
    <ul class="auth-points">
      <li>${icon('bird', 18)}<span>Livestock, fish and crop records</span></li>
      <li>${icon('chart', 18)}<span>Costs, profit and loan-ready reports</span></li>
      <li>${icon('help', 18)}<span>Advice from your own records</span></li>
      <li>${icon('shield', 18)}<span>Works without internet</span></li>
    </ul>
    <svg class="auth-fields" viewBox="0 0 600 220" preserveAspectRatio="none" aria-hidden="true">
      <path d="M0 120 C120 90 260 95 380 112 C470 125 540 118 600 100 L600 220 L0 220 Z" fill="currentColor" opacity=".10"/>
      <path d="M0 160 C140 128 300 138 420 150 C500 158 560 152 600 140 L600 220 L0 220 Z" fill="currentColor" opacity=".14"/>
      <path d="M0 196 C160 170 330 178 460 186 C530 190 570 186 600 180 L600 220 L0 220 Z" fill="currentColor" opacity=".2"/>
    </svg>
  </aside>`;
}

const passwordPair = (label = 'Password') => `<div class="grid-2">
  <label class="field"><span>${label}</span><span class="input-affix"><input name="password" type="password" minlength="8" required autocomplete="new-password"><button type="button" class="affix-btn" data-reveal aria-label="Show password">Show</button></span><small>At least 8 characters.</small></label>
  <label class="field"><span>Confirm password</span><input name="confirm" type="password" minlength="8" required autocomplete="new-password"></label>
</div>`;

function checkPasswords(v: Record<string, string>): string {
  if ((v.password ?? '').length < 8) return 'Password must be at least 8 characters.';
  if (v.password !== v.confirm) return 'The two passwords do not match.';
  return '';
}

export async function renderAuth(root: HTMLElement, onSignedIn: SignedIn) {
  wireRevealOnce();
  const [status, b] = await Promise.all([
    call<{ needs_setup: boolean }>('setup_status'),
    call<Branding>('branding').catch(() => null),
  ]);
  branding = b;
  if (b) applyTheme(b.theme_primary, b.theme_secondary, b.theme_mode);
  if (status.needs_setup) return renderSetup(root, onSignedIn);
  renderLogin(root, onSignedIn);
}

// ---------------------------------------------------------------- sign in

function renderLogin(root: HTMLElement, onSignedIn: SignedIn) {
  root.innerHTML = `<main class="auth">
    ${brandPanel('Your farm, in one place', 'Each person on the farm signs in with their own account, so every record carries a name.')}
    <section class="auth-main">
      <div class="auth-form stack">
        <form class="stack" id="login-form" novalidate>
          <div><p class="eyebrow">Sign in</p><h2>Welcome back</h2><p class="muted">Enter your username and password.</p></div>
          <label class="field"><span>Username</span><input name="username" autocomplete="username" required autofocus spellcheck="false" autocapitalize="off"></label>
          <label class="field"><span>Password</span>
            <span class="input-affix"><input name="password" type="password" autocomplete="current-password" required><button type="button" class="affix-btn" data-reveal aria-label="Show password">Show</button></span>
          </label>
          <p class="form-error" role="alert" id="login-error"></p>
          <button class="btn btn-primary btn-lg btn-block" type="submit">Sign in</button>
        </form>
        <div class="auth-links">
          <button type="button" class="link-btn" data-recover="reset">Forgot password?</button>
          <button type="button" class="btn btn-ghost btn-block" data-recover="create_owner">${icon('plus', 16)} Create a new owner account</button>
          <p class="fineprint">Staff who forget their password can also ask the owner to reset it under Staff. Creating an owner account needs the recovery key or the owner's security answers.</p>
        </div>
      </div>
    </section>
  </main>`;
  const form = $<HTMLFormElement>('#login-form', root)!;
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const v = formValues(form);
    const err = $('#login-error', root)!;
    err.textContent = '';
    if (!v.username || !v.password) { err.textContent = 'Enter your username and password.'; return; }
    const btn = $<HTMLButtonElement>('button[type=submit]', form)!;
    btn.disabled = true; btn.textContent = 'Signing in…';
    try {
      onSignedIn(await call<SessionInfo>('login', { username: v.username, password: v.password }));
    } catch (ex) {
      err.textContent = errText(ex);
      btn.disabled = false; btn.textContent = 'Sign in';
      ($('input[name=password]', form) as HTMLInputElement).select();
    }
  });
  $$<HTMLButtonElement>('[data-recover]', root).forEach((b) => b.addEventListener('click', () =>
    renderRecover(root, b.dataset.recover as 'reset' | 'create_owner', onSignedIn, (form.elements.namedItem('username') as HTMLInputElement).value)));
}

// ---------------------------------------------------------------- forgot password / create owner

async function renderRecover(root: HTMLElement, action: 'reset' | 'create_owner', onSignedIn: SignedIn, username = '') {
  const opts = await call<RecoveryOptions>('recovery_options').catch(() => ({ key_set: false, questions: [] }));
  const creating = action === 'create_owner';
  const hasKey = opts.key_set, hasQuestions = opts.questions.length === 3;
  const title = creating ? 'Create a new owner account' : 'Reset a password';
  const lead = creating
    ? 'Prove you own this business with the recovery key or the security answers, then choose the new owner\'s details.'
    : 'Prove you own this business with the recovery key or the security answers, then set a new password for any account.';
  const body = !hasKey && !hasQuestions
    ? `<div class="callout callout-warn"><div><b>Account recovery is not set up on this computer.</b><span>The owner can turn it on after signing in, under Settings → Security. Until then, another owner can reset passwords under Staff.</span></div></div>
       <button type="button" class="btn btn-ghost" data-back>${icon('back', 16)} Back to sign in</button>`
    : `<form class="stack" id="recover-form" novalidate>
        <div class="field"><span>How will you prove it's you?</span>
          <div class="seg seg-wide" role="radiogroup">
            ${hasKey ? `<label><input type="radio" name="method" value="key" checked><span><b>Recovery key</b><small>The 16-character key from setup</small></span></label>` : ''}
            ${hasQuestions ? `<label><input type="radio" name="method" value="questions" ${hasKey ? '' : 'checked'}><span><b>Security questions</b><small>Answer all three</small></span></label>` : ''}
          </div>
        </div>
        <div data-pane="key" ${hasKey ? '' : 'hidden'}>
          <label class="field"><span>Recovery key</span><input name="recovery_key" class="key-input" placeholder="XXXX-XXXX-XXXX-XXXX" maxlength="24" autocomplete="off" spellcheck="false" autocapitalize="characters"><small>Dashes and capital letters are optional.</small></label>
        </div>
        <div data-pane="questions" class="stack" ${hasKey ? 'hidden' : ''}>
          ${opts.questions.map((q, i) => `<label class="field"><span>${esc(q)}</span><input name="answer${i}" autocomplete="off" spellcheck="false"></label>`).join('')}
          <small class="muted">Answers are not case-sensitive.</small>
        </div>
        <hr class="rule">
        ${creating
          ? `<label class="field"><span>New owner's full name</span><input name="full_name" maxlength="80" autocomplete="name" required></label>
             <label class="field"><span>New username</span><input name="username" minlength="3" maxlength="64" spellcheck="false" autocapitalize="off" required><small>Letters, numbers, dots, dashes or underscores.</small></label>`
          : `<label class="field"><span>Username of the account to reset</span><input name="username" value="${esc(username)}" spellcheck="false" autocapitalize="off" required><small>Any account: owner, manager or cashier. A deactivated account is switched back on.</small></label>`}
        ${passwordPair(creating ? 'Password' : 'New password')}
        <p class="form-error" role="alert" data-err></p>
        <div class="form-actions"><button type="button" class="btn btn-ghost" data-back>${icon('back', 16)} Back</button><button class="btn btn-primary btn-lg">${creating ? 'Create owner and sign in' : 'Reset password and sign in'}</button></div>
      </form>`;
  root.innerHTML = `<main class="auth">
    ${brandPanel(creating ? 'Owner access' : 'Forgot password', 'Recovery works offline. No email or internet is needed.')}
    <section class="auth-main"><div class="auth-form stack"><div><h2>${title}</h2><p class="muted">${esc(lead)}</p></div>${body}</div></section>
  </main>`;
  $$('[data-back]', root).forEach((b) => b.addEventListener('click', () => renderLogin(root, onSignedIn)));
  const form = $<HTMLFormElement>('#recover-form', root);
  if (!form) return;
  form.addEventListener('change', (e) => {
    const el = e.target as HTMLInputElement;
    if (el.name !== 'method') return;
    $$<HTMLElement>('[data-pane]', form).forEach((p) => (p.hidden = p.dataset.pane !== el.value));
  });
  ($('input[name=recovery_key], input[name=answer0]', form) as HTMLInputElement | null)?.focus();
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const v = formValues(form);
    const err = $('[data-err]', form)!;
    err.textContent = '';
    const method = v.method as 'key' | 'questions';
    if (method === 'key' && v.recovery_key.replace(/[^a-z0-9]/gi, '').length !== 16) { err.textContent = 'Enter the full 16-character recovery key.'; return; }
    if (method === 'questions' && [v.answer0, v.answer1, v.answer2].some((a) => !a?.trim())) { err.textContent = 'Answer all three questions.'; return; }
    if (!v.username?.trim()) { err.textContent = 'Enter the username.'; return; }
    if (creating && !v.full_name?.trim()) { err.textContent = 'Enter the new owner\'s full name.'; return; }
    const pwErr = checkPasswords(v);
    if (pwErr) { err.textContent = pwErr; return; }
    const btn = $<HTMLButtonElement>('.btn-primary', form)!;
    btn.disabled = true;
    try {
      const r = await call<RecoverResult>('recover_account', { input: {
        method, recovery_key: method === 'key' ? v.recovery_key : null, answers: method === 'questions' ? [v.answer0, v.answer1, v.answer2] : null,
        action, username: v.username, full_name: creating ? v.full_name : null, new_password: v.password,
      } });
      if (r.new_recovery_key) {
        showRecoveryKey(root, r.new_recovery_key, brandName(), {
          title: 'Here is your new recovery key',
          intro: 'The old key has just been used, so it no longer works. Save this new one before you continue.',
        }, () => onSignedIn(r.session));
      } else onSignedIn(r.session);
    } catch (ex) {
      err.textContent = errText(ex);
      btn.disabled = false;
    }
  });
}

// ---------------------------------------------------------------- first-run setup

async function renderSetup(root: HTMLElement, onSignedIn: SignedIn) {
  const answers: Answers = { language: 'en', features: ['purchases', 'sales', 'stock', 'costs', 'reminders', 'contacts', 'reports'] };
  root.innerHTML = `<main class="auth auth-setup">
    ${brandPanel('Set up your farm', 'A few questions about your farm, then your owner account. FarmBuddy only shows what your farm needs.')}
    <section class="auth-main"><div class="auth-form auth-form-wide" data-host></div></section>
  </main>`;
  const host = $<HTMLElement>('[data-host]', root)!;
  const extra = ['Owner account', 'Account recovery'];
  await farmQuestions(host, {
    answers, flowCmd: 'setup_flow', checkCmd: 'setup_check_step', extraSteps: extra,
    onFinish: async () => { ownerSteps(host, answers, onSignedIn, root); },
  });
}

/** After the farm questions: the owner account and account recovery, then one call creates everything. */
function ownerSteps(host: HTMLElement, answers: Answers, onSignedIn: SignedIn, root: HTMLElement) {
  const farmName = String(answers.farm_name ?? '').trim();
  host.innerHTML = `<div class="wizard">
    <form class="wizard-card stack" id="setup-form" novalidate>
      <fieldset class="stack" data-step="1">
        <header class="wizard-head"><p class="eyebrow">Almost done</p><h2>Owner account</h2></header>
        <p class="muted">The owner adds managers, farm workers and advisers, changes settings and sees every report.</p>
        <label class="field"><span>Your full name</span><input name="full_name" required maxlength="80" autocomplete="name" placeholder="e.g. Treasure Idem"></label>
        <label class="field"><span>Username</span><input name="username" required minlength="3" maxlength="64" autocomplete="username" spellcheck="false" autocapitalize="off" placeholder="e.g. treasure"><small>Letters, numbers, dots, dashes or underscores.</small></label>
        ${passwordPair()}
        <p class="form-error" role="alert" data-error="1"></p>
        <div class="form-actions"><button type="button" class="btn btn-ghost" data-restart>${icon('back', 16)} Farm questions</button><button type="button" class="btn btn-primary btn-lg" data-next>Continue ${icon('arrowRight', 16)}</button></div>
      </fieldset>
      <fieldset class="stack" data-step="2" hidden>
        <header class="wizard-head"><p class="eyebrow">Last step</p><h2>Account recovery</h2></header>
        <p class="muted">If the owner password is ever forgotten, these answers (or the recovery key shown next) let you reset it, without internet. Pick answers you will remember but others will not guess.</p>
        ${questionFields()}
        <p class="form-error" role="alert" data-error="2"></p>
        <div class="form-actions"><button type="button" class="btn btn-ghost" data-back>${icon('back', 16)} Back</button><button class="btn btn-primary btn-lg" type="submit">Create my farm</button></div>
      </fieldset>
    </form></div>`;
  const form = $<HTMLFormElement>('#setup-form', host)!;
  wireQuestionFields(form);
  const show = (n: number) => $$<HTMLElement>('[data-step]', form).forEach((f) => (f.hidden = f.dataset.step !== String(n)));
  ($('input[name=full_name]', form) as HTMLInputElement).focus();
  form.addEventListener('click', (e) => {
    const t = e.target as HTMLElement;
    if (t.closest('[data-restart]')) { void renderSetupAgain(); return; }
    if (t.closest('[data-back]')) { show(1); return; }
    if (t.closest('[data-next]')) {
      const v = formValues(form);
      const msg = !v.full_name.trim() ? 'Enter your full name.' : v.username.trim().length < 3 ? 'Username must be at least 3 characters.' : checkPasswords(v);
      $('[data-error="1"]', form)!.textContent = msg;
      if (!msg) show(2);
    }
  });
  const renderSetupAgain = () => farmQuestions(host, {
    answers, flowCmd: 'setup_flow', checkCmd: 'setup_check_step', extraSteps: ['Owner account', 'Account recovery'],
    onFinish: async () => { ownerSteps(host, answers, onSignedIn, root); },
  });
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const err = $('[data-error="2"]', form)!;
    err.textContent = '';
    const qa = readQuestions(form);
    if (typeof qa === 'string') { err.textContent = qa; return; }
    const v = formValues(form);
    const btn = $<HTMLButtonElement>('button[type=submit]', form)!;
    btn.disabled = true; btn.textContent = 'Creating your farm…';
    try {
      const r = await call<{ session: SessionInfo; recovery_key: string }>('setup_farm', { input: {
        answers, full_name: v.full_name, username: v.username, password: v.password,
        security_questions: qa.questions, security_answers: qa.answers,
      } });
      showRecoveryKey(root, r.recovery_key, farmName, {
        title: 'Save your recovery key',
        intro: 'If the owner password is ever lost, this key lets you reset it or create a new owner account, without internet.',
      }, () => onSignedIn(r.session));
    } catch (ex) {
      err.textContent = errText(ex);
      btn.disabled = false; btn.textContent = 'Create my farm';
    }
  });
}

// Wired once on the document so re-rendering the sign-in screen never doubles it.
let revealWired = false;
function wireRevealOnce() {
  if (revealWired) return;
  revealWired = true;
  document.addEventListener('click', (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLButtonElement>('[data-reveal]');
    if (!btn) return;
    const input = btn.parentElement!.querySelector('input')!;
    const reveal = input.type === 'password';
    input.type = reveal ? 'text' : 'password';
    btn.textContent = reveal ? 'Hide' : 'Show';
    btn.setAttribute('aria-label', reveal ? 'Hide password' : 'Show password');
  });
}
