import { startIbibioScreens } from '../lib/i18n/ib-dom';
import { call } from '../api';
import { $, $$, errText, esc, toast, withBusy } from '../lib/dom';
import { icon } from '../lib/icons';
import { LANGUAGES, t } from '../lib/i18n';
import { LGAS, STATES } from '../lib/content/nigeria';
import { applyTheme } from '../lib/theme';
import { store } from '../store';
import type { OnboardingQuestion, OnboardingResult, OnboardingStep } from '../types';
import type { ViewCtx } from './shell';

export type Answers = Record<string, string | string[]>;

export interface QuestionsOptions {
  answers: Answers;
  /** Backend commands: the visible steps for the answers, and one step's check. */
  flowCmd: string;
  checkCmd: string;
  /** Extra steps shown after the farm questions (first run: owner account, recovery). */
  extraSteps?: string[];
  cancelLabel?: string;
  onCancel?: () => void;
  onFinish: (answers: Answers) => Promise<void>;
}

/** The guided farm questions. The backend decides which questions are visible for the answers so far. */
export async function farmQuestions(host: HTMLElement, o: QuestionsOptions) {
  const answers = o.answers;
  let steps: OnboardingStep[] = [];
  let index = 0;
  const extra = o.extraSteps ?? [];

  const load = async () => { steps = (await call<OnboardingStep[]>(o.flowCmd, { answers })).filter((s) => s.questions.length); index = Math.min(index, steps.length - 1); };

  const field = (q: OnboardingQuestion): string => {
    const label = esc(t(`q.${q.id}`, {}, q.prompt));
    const help = q.help ? `<small>${esc(t(`q.${q.id}.help`, {}, q.help))}</small>` : '';
    const v = answers[q.id];
    const req = q.required ? ' <span class="req" aria-hidden="true">*</span>' : '';
    const opt = (value: string, text: string) => esc(t(`opt.${q.id}.${value}`, {}, text));
    switch (q.kind) {
      case 'single':
      case 'multi': {
        const type = q.kind === 'single' ? 'radio' : 'checkbox';
        const chosen = (val: string) => (Array.isArray(v) ? v.includes(val) : v === val);
        return `<fieldset class="q-field"><legend>${label}${req}</legend>${help}
          <div class="choice-grid">${q.options.map(([val, text]) => `<label class="choice"><input type="${type}" name="${q.id}" value="${esc(val)}" ${chosen(val) ? 'checked' : ''} data-choice><span>${icon('check', 15)}${opt(val, text)}</span></label>`).join('')}</div></fieldset>`;
      }
      case 'state':
        return `<label class="field q-field"><span>${label}${req}</span>${help}<select name="${q.id}" data-choice><option value="">Choose…</option>${STATES.map((s) => `<option ${v === s ? 'selected' : ''}>${esc(s)}</option>`).join('')}</select></label>`;
      case 'lga': {
        const list = LGAS[String(answers.state ?? '')] ?? [];
        return `<label class="field q-field"><span>${label}${req}</span>${help}<input name="${q.id}" list="lga-list" maxlength="80" value="${esc(v ?? '')}" autocomplete="off"><datalist id="lga-list">${list.map((l) => `<option value="${esc(l)}">`).join('')}</datalist></label>`;
      }
      case 'number':
        return `<label class="field q-field"><span>${label}${req}</span>${help}<input name="${q.id}" inputmode="decimal" value="${esc(v ?? '')}"></label>`;
      case 'date':
        return `<label class="field q-field"><span>${label}${req}</span>${help}<input type="date" name="${q.id}" value="${esc(v ?? '')}"></label>`;
      default:
        return `<label class="field q-field"><span>${label}${req}</span>${help}<input name="${q.id}" maxlength="200" value="${esc(v ?? '')}"></label>`;
    }
  };

  const collect = (form: HTMLFormElement) => {
    for (const q of steps[index].questions) {
      if (q.kind === 'multi') answers[q.id] = $$<HTMLInputElement>(`input[name="${q.id}"]:checked`, form).map((i) => i.value);
      else if (q.kind === 'single') answers[q.id] = $<HTMLInputElement>(`input[name="${q.id}"]:checked`, form)?.value ?? '';
      else answers[q.id] = ($<HTMLInputElement>(`[name="${q.id}"]`, form)?.value ?? '').trim();
    }
  };

  const draw = () => {
    const step = steps[index];
    const last = index === steps.length - 1;
    const total = steps.length + extra.length;
    host.innerHTML = `<div class="wizard">
      <ol class="wizard-steps" aria-label="Steps">${[...steps.map((s) => t(`step.${s.step}`, {}, s.title)), ...extra].map((title, i) => `<li class="${i < index ? 'is-done' : i === index ? 'is-on' : ''}"><span>${i + 1}</span>${esc(title)}</li>`).join('')}</ol>
      <form class="wizard-card stack" novalidate>
        <header class="wizard-head"><p class="eyebrow">${esc(`Step ${index + 1} of ${total}`)}</p><h2>${esc(t(`step.${step.step}`, {}, step.title))}</h2>
          ${step.step === 1 ? '' : `<div class="seg lang-switch">${LANGUAGES.map((l) => `<label><input type="radio" name="__lang" value="${l.code}" ${answers.language === l.code ? 'checked' : ''}><span>${esc(l.name)}</span></label>`).join('')}</div>`}</header>
        ${step.questions.map(field).join('')}
        ${step.step === 1 ? `<p class="muted small">Ibibio text is being prepared with fluent Ibibio reviewers. Until a screen is translated, it shows in English. Your own farm names and notes are never changed.</p>` : ''}
        <p class="form-error" role="alert" data-err></p>
        <div class="form-actions wizard-actions">
          ${index > 0 ? `<button type="button" class="btn btn-ghost" data-back>${icon('back', 16)} ${esc(t('common.back'))}</button>` : o.onCancel ? `<button type="button" class="btn btn-ghost" data-skip>${esc(o.cancelLabel ?? t('common.cancel'))}</button>` : '<span></span>'}
          <button class="btn btn-primary btn-lg">${esc(last && !extra.length ? t('common.finish') : t('common.continue'))} ${icon('arrowRight', 16)}</button>
        </div>
      </form></div>`;
    const form = $<HTMLFormElement>('form', host)!;
    // Choices can change which questions follow, so the step is redrawn from the backend.
    form.addEventListener('change', async (e) => {
      const el = e.target as HTMLInputElement;
      if (el.name === '__lang' || (el.name === 'language' && el.type === 'radio')) {
        answers.language = el.value;
        store.settings.language = el.value;
        document.documentElement.lang = el.value === 'ib' ? 'ibb' : 'en';
        draw();
        return;
      }
      if (!el.hasAttribute('data-choice')) return;
      collect(form);
      const before = steps[index].questions.map((q) => q.id).join();
      await load();
      if (steps[index] && steps[index].questions.map((q) => q.id).join() !== before) draw();
    });
    $('[data-back]', form)?.addEventListener('click', () => { collect(form); index = Math.max(0, index - 1); draw(); });
    $('[data-skip]', form)?.addEventListener('click', () => o.onCancel?.());
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      collect(form);
      const err = $('[data-err]', form)!;
      withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
        try {
          await call(o.checkCmd, { answers, step: steps[index].step });
          await load();
          if (index < steps.length - 1) { index++; draw(); window.scrollTo({ top: 0 }); return; }
          await o.onFinish(answers);
        } catch (x) { err.textContent = errText(x); }
      });
    });
    $<HTMLElement>('input:not([type=radio]):not([type=checkbox]), select, input', form)?.focus();
  };

  try { await load(); draw(); } catch (e) { toast(errText(e), 'error'); }
}

/** My Farm → Set up (or change) the farm: the same questions for a signed-in owner. */
export async function render(ctx: ViewCtx) {
  ctx.header(t('nav.onboarding'), t('notice.offline'));
  let answers: Answers = {};
  try { answers = JSON.parse(store.settings.onboarding_answers || '{}'); } catch { answers = {}; }
  answers.language ??= store.settings.language || 'en';
  if (!answers.farm_name && store.settings.business_name && !['My Farm', 'My Business', 'My Eatery'].includes(store.settings.business_name)) answers.farm_name = store.settings.business_name;
  if (!answers.phone && store.settings.business_phone) answers.phone = store.settings.business_phone;
  if (!answers.features) answers.features = ['purchases', 'sales', 'stock', 'costs', 'reminders', 'contacts', 'reports'];
  const rerun = store.settings.onboarding_done === '1';
  const done = (res: OnboardingResult) => {
    toast('Your farm is set up');
    ctx.main.innerHTML = `<div class="wizard"><section class="wizard-card stack done-card">
      <div class="done-mark">${icon('check', 30)}</div>
      <h2>Your farm is ready</h2>
      ${res.created.length ? `<p class="lead-text">FarmBuddy created:</p><ul class="done-list">${res.created.map((c) => `<li>${icon('check', 14)} ${esc(c)}</li>`).join('')}</ul>` : '<p class="lead-text">Your profile and modules were updated.</p>'}
      <p class="muted">Pages switched on: ${res.modules.map((m) => esc(m)).join(', ') || 'none'}. Change this any time in My Farm.</p>
      <div class="callout callout-info">${icon('info', 18)}<div><b>Want to look around first?</b><span>Load the demo farm from My Farm: a 500-bird broiler batch, a cassava plot and a palm oil batch, all clearly marked Demo and removable in one step.</span></div></div>
      <div class="form-actions"><button class="btn btn-ghost" data-go="farm">Open My Farm</button><button class="btn btn-primary btn-lg" data-go="home">Go to Home ${icon('arrowRight', 16)}</button></div>
    </section></div>`;
    ctx.main.onclick = (e) => { const b = (e.target as HTMLElement).closest<HTMLElement>('[data-go]'); if (b) ctx.go(b.dataset.go!); };
  };
  await farmQuestions(ctx.main, {
    answers, flowCmd: 'onboarding_flow', checkCmd: 'onboarding_check_step',
    onCancel: rerun ? () => ctx.go('farm') : undefined,
    onFinish: async (a) => {
      const res = await call<OnboardingResult>('complete_onboarding', { answers: a });
      await store.loadSettings();
      startIbibioScreens();
      applyTheme(store.settings.theme_primary, store.settings.theme_secondary, store.settings.theme_mode);
      ctx.refreshChrome();
      done(res);
    },
  });
}
