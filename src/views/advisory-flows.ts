import { auth, call } from '../api';
import { $, $$, errText, esc, toast, withBusy } from '../lib/dom';
import { dateTimeLabel } from '../lib/format';
import { icon } from '../lib/icons';
import { t } from '../lib/i18n';
import { STATES } from '../lib/content/nigeria';
import { pill, tableHtml } from '../lib/ui';
import { store } from '../store';
import type { Recommendation } from '../types';

interface FlowInfo { key: string; title: string; summary: string }
interface Question { id: string; kind: string; prompt: string; help: string; options: Array<[string, string]>; required: boolean }
interface HistoryRow { id: number; flow: string; results: Recommendation[]; user_name: string; created_at: string }
interface Rule { rule_key: string; title: string; description: string; enabled: boolean; priority: number; param_days: number | null }

type Answers = Record<string, string | string[]>;
const SAFETY_TONE: Record<string, 'mute' | 'warn' | 'bad'> = { general: 'mute', record_keeping: 'mute', planning: 'mute', professional_confirmation: 'warn', vet_only: 'bad', chemical_label: 'bad' };

/**
 * Ask FarmBuddy: short guided check-ups. Each answer can bring up the next
 * question; the result says what to do, why (from your answers), what kind of
 * advice it is and where it comes from. Results are saved for later.
 */
export async function askFlows(pane: HTMLElement, remind: (seed: { title?: string; related_kind?: string; source?: string }) => void, openArticle: (slug: string) => Promise<void>) {
  const flows = await call<FlowInfo[]>('list_advisory_flows');
  const history = await call<HistoryRow[]>('advisory_history');
  const preferred = (store.settings.template_questions || '').split(',').filter(Boolean);
  const ordered = [...flows].sort((a, b) => Number(!preferred.includes(a.key)) - Number(!preferred.includes(b.key)));
  pane.innerHTML = `<p class="muted small">Answer a few questions and FarmBuddy explains what to look at. It never diagnoses disease, names a medicine or gives chemical rates: for those it tells you who to ask.</p>
    <div class="subtile-grid">${ordered.map((f) => `<button type="button" class="subtile" data-flow="${f.key}"><span class="subtile-icon">${icon(f.key === 'crop' ? 'sun' : f.key === 'processing' ? 'layers' : 'bird', 22)}</span><span class="subtile-text"><b>${esc(f.title)}</b><small>${esc(f.summary)}</small></span>${icon('chevron', 16)}</button>`).join('')}</div>
    <div data-flow-box></div>
    <section class="panel"><header class="panel-head"><h2>Earlier check-ups</h2></header>
      ${tableHtml(['When', 'Check-up', 'By', 'Top result'], history.slice(0, 20).map((h) => [esc(dateTimeLabel(h.created_at)), esc(flows.find((f) => f.key === h.flow)?.title ?? h.flow), esc(h.user_name), esc(h.results?.[0]?.title ?? '')]), 'No check-ups yet.')}</section>`;
  const box = $('[data-flow-box]', pane)!;
  pane.onclick = (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-flow]');
    if (b) void runFlow(box, flows.find((f) => f.key === b.dataset.flow)!, remind, openArticle, () => void askFlows(pane, remind, openArticle));
  };
}

async function runFlow(box: HTMLElement, flow: FlowInfo, remind: (seed: { title?: string; related_kind?: string; source?: string }) => void, openArticle: (slug: string) => Promise<void>, refresh: () => void) {
  const answers: Answers = {};
  if (flow.key === 'crop' && store.settings.business_state) answers.state = store.settings.business_state;
  const read = () => {
    $$<HTMLElement>('[data-q]', box).forEach((el) => {
      const id = el.dataset.q!;
      const kind = el.dataset.kind!;
      if (kind === 'multi') answers[id] = $$<HTMLInputElement>(`input[name="${id}"]:checked`, el).map((i) => i.value);
      else if (kind === 'single') answers[id] = $<HTMLInputElement>(`input[name="${id}"]:checked`, el)?.value ?? '';
      else answers[id] = ($<HTMLInputElement | HTMLSelectElement>(`[name="${id}"]`, el)?.value ?? '').trim();
    });
  };
  const qHtml = (q: Question) => {
    const v = answers[q.id];
    let input = '';
    if (q.kind === 'single' || q.kind === 'multi') {
      const type = q.kind === 'single' ? 'radio' : 'checkbox';
      input = `<div class="choice-grid">${q.options.map(([k, l]) => `<label class="choice"><input type="${type}" name="${q.id}" value="${esc(k)}" ${(Array.isArray(v) ? v.includes(k) : v === k) ? 'checked' : ''}><span>${esc(l)}</span></label>`).join('')}</div>`;
    } else if (q.kind === 'state') input = `<select name="${q.id}"><option value="">—</option>${STATES.map((s) => `<option ${v === s ? 'selected' : ''}>${esc(s)}</option>`).join('')}</select>`;
    else if (q.kind === 'date') input = `<input type="date" name="${q.id}" value="${esc(v ?? '')}">`;
    else if (q.kind === 'number') input = `<input name="${q.id}" inputmode="decimal" value="${esc(v ?? '')}">`;
    else input = `<textarea name="${q.id}" rows="2" maxlength="500">${esc(v ?? '')}</textarea>`;
    return `<fieldset class="flow-q" data-q="${q.id}" data-kind="${q.kind}"><legend>${esc(q.prompt)}${q.required ? '' : ` <small class="muted">(${esc(t('common.optional'))})</small>`}</legend>${q.help ? `<small class="muted">${esc(q.help)}</small>` : ''}${input}</fieldset>`;
  };
  const draw = async () => {
    const qs = await call<Question[]>('advisory_questions', { flow: flow.key, answers });
    box.innerHTML = `<section class="panel"><header class="panel-head"><h2>${esc(flow.title)}</h2><button type="button" class="btn btn-ghost btn-sm" data-close-flow>Close</button></header>
      <form class="stack" data-flow-form>${qs.map(qHtml).join('')}<p class="form-error" role="alert" data-err></p>
      <div class="form-actions"><button class="btn btn-primary">See what to do</button></div></form><div data-results></div></section>`;
    box.scrollIntoView({ behavior: 'smooth', block: 'start' });
    const form = $<HTMLFormElement>('[data-flow-form]', box)!;
    // Answers can bring up follow-up questions.
    form.addEventListener('change', () => { read(); void call<Question[]>('advisory_questions', { flow: flow.key, answers }).then((next) => { if (next.length !== qs.length) void draw(); }); });
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      read();
      void withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
        try {
          const r = await call<{ id: number; results: Recommendation[] }>('run_advisory', { flow: flow.key, answers });
          $('[data-err]', form)!.textContent = '';
          $('[data-results]', box)!.innerHTML = `<h3 class="section-title">What to do</h3><div class="stack">${r.results.map((x) => `<article class="rec-card prio-${x.priority}">
            <header><span class="rec-cat">${esc(x.category)}</span>${pill(t(`safety.${x.safety_level}`, {}, x.safety_level.replace(/_/g, ' ')), SAFETY_TONE[x.safety_level] ?? 'mute')}</header>
            <h3>${esc(x.title)}</h3><p>${esc(x.text)}</p>
            <p class="rec-why"><b>${esc(t('common.why'))}:</b> ${esc(x.why)} <small class="muted">${esc(t('common.source'))}: ${esc(x.source)}</small></p>
            <div class="row-actions">${x.article ? `<button type="button" class="btn btn-sm btn-ghost" data-art="${esc(x.article)}">${icon('help', 14)} ${esc(t('common.read_more'))}</button>` : ''}<button type="button" class="btn btn-sm btn-ghost" data-rem="${esc(x.key)}">${icon('calendar', 14)} ${esc(t('common.remind_me'))}</button></div></article>`).join('')}</div>
            <p class="muted small">Saved to your check-up history.</p>`;
          $('[data-results]', box)!.onclick = (ev) => {
            const el = ev.target as HTMLElement;
            const a = el.closest<HTMLElement>('[data-art]');
            if (a) void openArticle(a.dataset.art!);
            const rm = el.closest<HTMLElement>('[data-rem]');
            if (rm) { const x = r.results.find((y) => y.key === rm.dataset.rem)!; remind({ title: x.title, related_kind: x.related_view, source: 'advisory' }); }
          };
        } catch (x) { $('[data-err]', form)!.textContent = errText(x); }
      });
    });
    $('[data-close-flow]', box)!.addEventListener('click', () => { box.innerHTML = ''; refresh(); });
  };
  await draw().catch((x) => toast(errText(x), 'error'));
}

/** Owner: switch suggestion rules on or off, set their priority and day thresholds. Stored in the database, not in code. */
export async function rulesEditor(pane: HTMLElement) {
  if (!auth.can('owner')) { pane.innerHTML = '<p class="muted">Only the owner can change suggestion rules.</p>'; return; }
  const rules = await call<Rule[]>('list_rules');
  pane.innerHTML = `<p class="muted small">Each suggestion comes from one of these rules. Switch off what does not fit your farm, raise or lower its priority (1 = most urgent), and change the number of days where a rule has one. Changes are recorded in the activity log.</p>
    <div class="table-scroll"><table class="table compact"><thead><tr><th>On</th><th>Rule</th><th>Priority</th><th>Days</th><th></th></tr></thead><tbody>
    ${rules.map((r) => `<tr data-rule="${esc(r.rule_key)}"><td><input type="checkbox" name="enabled" ${r.enabled ? 'checked' : ''} aria-label="Rule on"></td><td><b>${esc(r.title)}</b><small class="muted block">${esc(r.description)}</small></td>
      <td><select name="priority" aria-label="Priority">${[1, 2, 3].map((p) => `<option ${r.priority === p ? 'selected' : ''}>${p}</option>`).join('')}</select></td>
      <td>${r.param_days === null ? '<span class="muted">—</span>' : `<input name="days" inputmode="numeric" value="${r.param_days}" class="w-num" aria-label="Days">`}</td>
      <td><button class="btn btn-ghost btn-sm" data-save-rule>${esc(t('common.save'))}</button></td></tr>`).join('')}</tbody></table></div>`;
  pane.onclick = (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>('[data-save-rule]');
    if (!b) return;
    const row = b.closest<HTMLElement>('[data-rule]')!;
    const days = row.querySelector<HTMLInputElement>('[name=days]');
    void withBusy(b, async () => {
      try {
        await call('save_rule', { ruleKey: row.dataset.rule, enabled: row.querySelector<HTMLInputElement>('[name=enabled]')!.checked, priority: Number(row.querySelector<HTMLSelectElement>('[name=priority]')!.value), paramDays: days ? Math.round(Number(days.value)) : null });
        toast('Rule saved');
      } catch (x) { toast(errText(x), 'error'); }
    });
  };
}
