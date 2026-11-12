import { esc, toast } from '../lib/dom';
import { icon } from '../lib/icons';
import { printHtml } from '../lib/receipt';

export const QUESTION_PRESETS = [
  'What was the name of your first primary school?',
  'In which town or village was your mother born?',
  'What is the first name of your oldest sibling?',
  'What was your childhood nickname?',
  'What was the make of your first mobile phone?',
  'What street did you live on as a child?',
  'What is the first name of your best friend in secondary school?',
  'What was the name of your first pet?',
];

/** Three question pickers (preset or own wording) with answer boxes. */
export function questionFields(existing: string[] = []): string {
  return [0, 1, 2].map((i) => {
    const current = existing[i] ?? QUESTION_PRESETS[i];
    const isPreset = QUESTION_PRESETS.includes(current);
    return `<div class="q-block" data-q="${i}">
      <label class="field"><span>Question ${i + 1}</span>
        <select data-qsel>
          ${QUESTION_PRESETS.map((q) => `<option ${q === current ? 'selected' : ''}>${esc(q)}</option>`).join('')}
          <option value="__own" ${isPreset ? '' : 'selected'}>Write my own question…</option>
        </select>
      </label>
      <input data-qown maxlength="140" placeholder="Type your own question" value="${isPreset ? '' : esc(current)}" ${isPreset ? 'hidden' : ''} aria-label="Your own question ${i + 1}">
      <label class="field"><span>Answer</span><input data-qans maxlength="120" autocomplete="off" spellcheck="false" required></label>
    </div>`;
  }).join('');
}

export function wireQuestionFields(root: HTMLElement) {
  root.addEventListener('change', (e) => {
    const sel = (e.target as HTMLElement).closest<HTMLSelectElement>('[data-qsel]');
    if (!sel) return;
    const own = sel.closest('.q-block')!.querySelector<HTMLInputElement>('[data-qown]')!;
    own.hidden = sel.value !== '__own';
    if (!own.hidden) own.focus();
  });
}

/** Reads the three question/answer pairs, or returns an error message. */
export function readQuestions(root: HTMLElement): { questions: string[]; answers: string[] } | string {
  const questions: string[] = [];
  const answers: string[] = [];
  for (const block of Array.from(root.querySelectorAll<HTMLElement>('.q-block'))) {
    const sel = block.querySelector<HTMLSelectElement>('[data-qsel]')!;
    const q = sel.value === '__own' ? block.querySelector<HTMLInputElement>('[data-qown]')!.value.trim() : sel.value;
    const a = block.querySelector<HTMLInputElement>('[data-qans]')!.value.trim();
    if (q.length < 5) return 'Type each of your own questions (at least 5 characters).';
    if (a.length < 2) return 'Answer all three security questions (at least 2 characters each).';
    questions.push(q); answers.push(a);
  }
  if (new Set(questions.map((q) => q.toLowerCase())).size < 3) return 'Choose three different questions.';
  return { questions, answers };
}

function keySheet(key: string, businessName: string): string {
  const date = new Date().toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric' });
  return `<article class="key-sheet">
    <h1>FarmBuddy recovery key</h1>
    <p><b>${esc(businessName)}</b> · created ${esc(date)}</p>
    <div class="key-sheet-key">${esc(key)}</div>
    <p>Use this key on the sign-in screen with <b>Forgot password</b> or <b>Create a new owner account</b> if the owner password is lost. It works once; the app then gives you a new key.</p>
    <p>Keep this sheet somewhere safe and private, away from the shop counter. Anyone holding it can take owner control of the app.</p>
  </article>`;
}

/** Full-screen panel that shows a recovery key once and waits for the owner to confirm they saved it. */
export function showRecoveryKey(root: HTMLElement, key: string, businessName: string, opts: { title: string; intro: string }, onDone: () => void) {
  root.innerHTML = `<main class="key-screen">
    <section class="key-card stack">
      <div class="key-icon">${icon('key', 26)}</div>
      <div><h2>${esc(opts.title)}</h2><p class="muted">${esc(opts.intro)}</p></div>
      <div class="key-value" aria-label="Recovery key">${esc(key)}</div>
      <div class="key-actions">
        <button class="btn btn-ghost" data-print>${icon('print', 16)} Print key</button>
        <button class="btn btn-ghost" data-copy>${icon('check', 16)} Copy</button>
      </div>
      <ul class="key-points">
        <li>It is shown <b>only now</b>. The app keeps a scrambled copy it cannot read back.</li>
        <li>With it, or with your three security answers, you can reset any password or create a new owner account from the sign-in screen.</li>
        <li>Each time the key is used, it is replaced with a new one.</li>
      </ul>
      <label class="check"><input type="checkbox" data-ack><span><b>I have written down or printed this key and stored it safely.</b></span></label>
      <button class="btn btn-primary btn-lg btn-block" data-done disabled>Continue to FarmBuddy</button>
    </section>
  </main>`;
  const done = root.querySelector<HTMLButtonElement>('[data-done]')!;
  root.querySelector<HTMLInputElement>('[data-ack]')!.addEventListener('change', (e) => { done.disabled = !(e.target as HTMLInputElement).checked; });
  root.querySelector('[data-print]')!.addEventListener('click', () => printHtml(keySheet(key, businessName)));
  root.querySelector('[data-copy]')!.addEventListener('click', async () => {
    try { await navigator.clipboard.writeText(key); toast('Recovery key copied'); }
    catch { toast('Copy is not available here. Write the key down instead.', 'error'); }
  });
  done.addEventListener('click', onDone);
}
