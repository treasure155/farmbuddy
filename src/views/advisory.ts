import { auth, call } from '../api';
import { $, $$, confirmDialog, debounce, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { dateLabel, isoDate } from '../lib/format';
import { icon } from '../lib/icons';
import { t } from '../lib/i18n';
import type { Article, ArticleSummary, KnowledgeSource, Recommendation, Reminder } from '../types';
import type { ViewCtx } from './shell';
import { askFlows, rulesEditor } from './advisory-flows';

type Tab = 'suggestions' | 'ask' | 'reminders' | 'library' | 'rules';
let tab: Tab = 'suggestions';
let query = '';
let showDone = false;

const SAFETY_TONE: Record<string, string> = { general: 'pill-mute', record_keeping: 'pill-mute', planning: 'pill-mute', professional_confirmation: 'pill-warn', vet_only: 'pill-bad', chemical_label: 'pill-bad' };
const safetyPill = (s: string) => `<span class="pill ${SAFETY_TONE[s] ?? 'pill-mute'}">${esc(t(`safety.${s}`))}</span>`;
const reviewPill = (s: string) => `<span class="pill ${s === 'approved' ? 'pill-ok' : 'pill-review'}">${esc(t(`review.${s}`))}</span>`;

/** Farm advisory: suggestions from your own records, reminders and the offline library. */
export async function render(ctx: ViewCtx) {
  if (ctx.params.tab) tab = ctx.params.tab as Tab;
  ctx.header(t('nav.advisory'), 'Suggestions worked out from your records, your reminders, and guides with their sources.', `<button class="btn btn-primary" data-new-reminder>${icon('plus', 16)} Reminder</button>`);
  ctx.main.innerHTML = `<div class="tabs" role="tablist">
      <button role="tab" class="tab" data-tab="suggestions">Suggestions</button>
      <button role="tab" class="tab" data-tab="ask">Ask FarmBuddy</button>
      <button role="tab" class="tab" data-tab="reminders">Reminders</button>
      <button role="tab" class="tab" data-tab="library">Knowledge library</button>
      ${auth.can('owner') ? '<button role="tab" class="tab" data-tab="rules">Suggestion rules</button>' : ''}</div>
    <div data-pane></div>`;
  const pane = $('[data-pane]', ctx.main)!;

  const show = async () => {
    $$('[data-tab]', ctx.main).forEach((b) => b.classList.toggle('is-on', b.dataset.tab === tab));
    if (tab === 'ask') { await askFlows(pane, (seed) => reminderForm(seed, () => undefined), openArticleBySlug); return; }
    if (tab === 'rules') { await rulesEditor(pane); return; }
    if (tab === 'suggestions') {
      const recs = await call<Recommendation[]>('get_recommendations');
      const cats = [...new Set(recs.map((r) => r.category))];
      const urgent = recs.filter((r) => r.priority === 1).length;
      pane.innerHTML = `<section class="engine-card">
          <div class="engine-step"><span>${icon('receipt', 20)}</span><b>Your records</b><small>Batches, crops, palm oil runs, stock and sales on this computer</small></div>
          <span class="engine-arrow">${icon('arrowRight', 18)}</span>
          <div class="engine-step"><span>${icon('sliders', 20)}</span><b>Rules engine</b><small>Checks your records offline every time you open this page</small></div>
          <span class="engine-arrow">${icon('arrowRight', 18)}</span>
          <div class="engine-step"><span>${icon('help', 20)}</span><b>Suggestion with its reason</b><small>Linked to a guide in the offline library, with its source</small></div>
        </section>
        ${recs.length ? `<section class="kpi-row kpi-4">
          <div class="kpi"><span class="kpi-icon">${icon('help', 18)}</span><small>Suggestions</small><b>${recs.length}</b><span>from your records</span></div>
          <div class="kpi ${urgent ? 'kpi-bad' : ''}"><span class="kpi-icon">${icon('alert', 18)}</span><small>Act first</small><b>${urgent}</b><span>top priority</span></div>
          ${cats.slice(0, 2).map((c) => `<div class="kpi"><span class="kpi-icon">${icon('layers', 18)}</span><small>${esc(c)}</small><b>${recs.filter((r) => r.category === c).length}</b><span>suggestion${recs.filter((r) => r.category === c).length === 1 ? '' : 's'}</span></div>`).join('')}
        </section>` : ''}
        <p class="muted small">FarmBuddy never diagnoses disease, prescribes medicine or gives chemical rates.</p>
        ${recs.length ? `<div class="rec-grid">${recs.map((r) => `<article class="rec-card prio-${r.priority}">
          <header><span class="rec-cat">${esc(r.category)}</span>${safetyPill(r.safety_level)}</header>
          <h3>${esc(r.title)}</h3><p>${esc(r.text)}</p>
          <p class="rec-why"><b>${esc(t('common.why'))}:</b> ${esc(r.why)} <small class="muted">${esc(t('common.source'))}: ${esc(r.source)}</small></p>
          <div class="row-actions">${r.related_view ? `<button class="btn btn-sm btn-primary" data-go="${esc(r.related_view)}" data-id="${r.related_id ?? ''}">${esc(t('common.open'))}</button>` : ''}
            ${r.article ? `<button class="btn btn-sm btn-ghost" data-article-slug="${esc(r.article)}">${icon('help', 14)} ${esc(t('common.read_more'))}</button>` : ''}
            <button class="btn btn-sm btn-ghost" data-remind="${esc(r.key)}">${icon('calendar', 14)} ${esc(t('common.remind_me'))}</button></div>
        </article>`).join('')}</div>` : emptyState('Nothing to suggest right now', 'Keep recording daily figures, field work and processing. Suggestions appear here when your records show something to act on.')}`;
      pane.onclick = (e) => {
        const el = e.target as HTMLElement;
        const go = el.closest<HTMLElement>('[data-go]');
        if (go) {
          const id = go.dataset.id ? Number(go.dataset.id) : undefined;
          const view = go.dataset.go!;
          ctx.go(view, view === 'health' ? { flock: id } : view === 'inventory' ? { tab: 'items', low: '1' } : { open: id });
          return;
        }
        const art = el.closest<HTMLElement>('[data-article-slug]');
        if (art) { void openArticleBySlug(art.dataset.articleSlug!); return; }
        const rem = el.closest<HTMLElement>('[data-remind]');
        if (rem) { const r = recs.find((x) => x.key === rem.dataset.remind)!; reminderForm({ title: r.title, related_kind: r.related_view, related_id: r.related_id, source: 'advisory' }, show); }
      };
    } else if (tab === 'reminders') {
      const rows = await call<Reminder[]>('list_reminders', { includeDone: showDone });
      pane.innerHTML = `<div class="toolbar"><label class="check inline"><input type="checkbox" data-show-done ${showDone ? 'checked' : ''}><span>Show done</span></label></div>
        ${rows.length ? `<ul class="reminder-list">${rows.map((r) => `<li class="${r.overdue ? 'is-overdue' : ''} ${r.status !== 'open' ? 'is-done' : ''}">
          ${r.status === 'open' ? `<button class="rem-check" data-done="${r.id}" aria-label="Mark done">${icon('check', 14)}</button>` : `<span class="rem-check is-checked">${icon('check', 14)}</span>`}
          <div><b>${esc(r.title)}</b>${r.is_demo ? ` <span class="pill pill-warn">${esc(t('common.demo'))}</span>` : ''}<small class="muted block">${esc(dateLabel(r.due_date))}${r.overdue ? ' · overdue' : ''}${r.notes ? ` · ${esc(r.notes)}` : ''}${r.source === 'advisory' ? ' · from a suggestion' : ''}</small></div>
          <span class="record-actions">${r.status === 'open' ? `<button class="icon-btn" data-dismiss="${r.id}" aria-label="Dismiss">${icon('close', 15)}</button>` : `<button class="icon-btn" data-reopen="${r.id}" aria-label="Reopen">${icon('undo', 15)}</button>`}
            ${auth.can('manager') ? `<button class="icon-btn" data-del="${r.id}" aria-label="Delete">${icon('trash', 15)}</button>` : ''}</span></li>`).join('')}</ul>`
        : emptyState('No reminders', 'Add reminders for farm tasks, or create one from a suggestion.', `<button class="btn btn-primary" data-new-reminder>${icon('plus', 16)} Reminder</button>`)}`;
      pane.onclick = async (e) => {
        const el = e.target as HTMLElement;
        const act = (attr: string, status: string) => { const b = el.closest<HTMLElement>(`[data-${attr}]`); return b ? call('set_reminder_status', { id: Number(b.getAttribute(`data-${attr}`)), status }).then(show).catch((x) => toast(errText(x), 'error')) : null; };
        if (act('done', 'done') || act('dismiss', 'dismissed') || act('reopen', 'open')) return;
        const del = el.closest<HTMLElement>('[data-del]');
        if (del && await confirmDialog({ title: 'Delete this reminder?', message: 'It is removed for everyone.', confirm: 'Delete', danger: true }) !== null) {
          try { await call('delete_reminder', { id: Number(del.dataset.del) }); void show(); } catch (x) { toast(errText(x), 'error'); }
        }
        if (el.closest('[data-new-reminder]')) reminderForm({}, show);
      };
      pane.onchange = (e) => { if ((e.target as HTMLElement).matches('[data-show-done]')) { showDone = (e.target as HTMLInputElement).checked; void show(); } };
    } else {
      pane.innerHTML = `<div class="toolbar"><label class="search-field">${icon('search', 16)}<input type="search" data-q placeholder="Search guides" value="${esc(query)}"></label>
        ${auth.can('manager') ? `<button class="btn btn-ghost" data-new-article>${icon('plus', 16)} Add guide</button>` : ''}</div>
        <p class="muted small">${esc(t('notice.offline'))} Built-in guides are drafts by the FarmBuddy team, each linked to its source. They stay “${esc(t('review.pending_review'))}” until a qualified reviewer approves them.</p>
        <div data-articles></div>`;
      const list = $('[data-articles]', pane)!;
      const load = async () => {
        const rows = await call<ArticleSummary[]>('list_articles', { query });
        const cats = [...new Set(rows.map((a) => a.category))];
        list.innerHTML = rows.length ? cats.map((c) => `<h3 class="section-title">${esc(c)}</h3><div class="card-grid">${rows.filter((a) => a.category === c).map((a) => `<button class="article-card" data-article="${a.id}">
          <b>${esc(a.title)}</b><span class="small">${esc(a.summary)}</span><span class="article-meta">${reviewPill(a.review_status)}${safetyPill(a.safety_level)}<small class="muted">${esc(a.source_name)}</small></span></button>`).join('')}</div>`).join('')
          : emptyState('No guides found', 'Try a different word.');
      };
      await load();
      $('[data-q]', pane)!.addEventListener('input', debounce((e: Event) => { query = (e.target as HTMLInputElement).value; void load(); }, 250));
      pane.onclick = (e) => {
        const el = e.target as HTMLElement;
        const a = el.closest<HTMLElement>('[data-article]');
        if (a) void openArticle(Number(a.dataset.article), load);
        if (el.closest('[data-new-article]')) void articleForm(null, load);
      };
    }
  };
  ctx.main.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-tab]');
    if (b) { tab = b.dataset.tab as Tab; void show(); }
  });
  $('[data-new-reminder]')?.addEventListener('click', () => reminderForm({}, () => { tab = 'reminders'; void show(); }));
  await show();
}

function reminderForm(seed: { title?: string; related_kind?: string; related_id?: number | null; source?: string }, done: () => void) {
  const tomorrow = isoDate(new Date(Date.now() + 86400000));
  const m = modal({ title: 'New reminder', size: 'sm', body: `<form class="stack">
    <label class="field"><span>What to do</span><input name="title" required maxlength="200" value="${esc(seed.title ?? '')}" autofocus></label>
    <label class="field"><span>When</span><input type="date" name="due_date" required value="${tomorrow}"></label>
    <label class="field"><span>Notes</span><input name="notes" maxlength="500"></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const kinds = ['livestock', 'health', 'crops', 'processing', 'inventory', 'customers'];
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('save_reminder', { input: { title: v.title, due_date: v.due_date, notes: v.notes, related_kind: kinds.includes(seed.related_kind ?? '') ? seed.related_kind : '', related_id: seed.related_id ?? null, source: seed.source ?? 'manual' } }); }
      catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Reminder saved'); m.close(); done();
    });
  });
}

/** Renders plain-text article bodies: blank lines separate paragraphs, "- " starts a bullet. */
function bodyHtml(text: string): string {
  return text.split(/\n{2,}/).map((block) => {
    const lines = block.split('\n');
    if (lines.every((l) => /^\s*-\s/.test(l) || !l.trim())) return `<ul>${lines.filter((l) => l.trim()).map((l) => `<li>${esc(l.replace(/^\s*-\s/, ''))}</li>`).join('')}</ul>`;
    const head = lines.filter((l) => !/^\s*-\s/.test(l));
    const bullets = lines.filter((l) => /^\s*-\s/.test(l));
    return `<p>${head.map(esc).join('<br>')}</p>${bullets.length ? `<ul>${bullets.map((l) => `<li>${esc(l.replace(/^\s*-\s/, ''))}</li>`).join('')}</ul>` : ''}`;
  }).join('');
}

async function openArticleBySlug(slug: string) {
  const rows = await call<ArticleSummary[]>('list_articles', {});
  const a = rows.find((r) => r.slug === slug);
  if (a) await openArticle(a.id, () => undefined);
}

async function openArticle(id: number, onChange: () => void) {
  const a = await call<Article>('get_article', { id });
  const m = modal({ title: a.title, subtitle: a.category, size: 'lg', body: `<article class="stack article-body">
    <div class="article-meta">${reviewPill(a.review_status)}${safetyPill(a.safety_level)}</div>
    ${a.review_status !== 'approved' ? `<div class="callout callout-warn"><div><b>${esc(t('review.pending_review'))}</b><span>This guide has not yet been approved by a qualified reviewer. Use it as general information and confirm anything important with a professional.</span></div></div>` : ''}
    <div class="prose">${bodyHtml(a.body)}</div>
    <dl class="meta-list">
      <dt>${esc(t('common.source'))}</dt><dd>${esc(a.source_name || '—')}${a.source_url ? `<br><small class="muted">${esc(a.source_url)}</small>` : ''}${a.source_ref ? `<br><small>${esc(a.source_ref)}</small>` : ''}</dd>
      <dt>Written by</dt><dd>${esc(a.author || '—')}</dd>
      <dt>Review</dt><dd>${a.review_status === 'approved' ? `Reviewed by ${esc(a.reviewer)} on ${esc(dateLabel(a.review_date))}` : 'Not reviewed yet'}</dd>
      <dt>Version</dt><dd>${a.version} · updated ${esc(dateLabel(a.updated_at.slice(0, 10)))}</dd>
    </dl>
    <div class="form-actions">
      ${auth.can('owner') && !a.is_builtin ? `<button class="btn btn-ghost btn-danger-text push-left" data-delete>${icon('trash', 16)} Delete</button>` : ''}
      ${auth.can('manager') ? `<button class="btn btn-ghost" data-edit>${icon('edit', 16)} Edit</button>` : ''}
      ${auth.can('owner') ? `<button class="btn btn-primary" data-review>${icon('shield', 16)} ${a.review_status === 'approved' ? 'Change review' : 'Record review'}</button>` : ''}
    </div></article>` });
  m.el.addEventListener('click', async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-edit]')) { m.close(); void articleForm(a, onChange); }
    if (el.closest('[data-review]')) { m.close(); reviewForm(a, onChange); }
    if (el.closest('[data-delete]')) {
      if (await confirmDialog({ title: 'Delete this guide?', message: 'It is removed from the library.', confirm: 'Delete', danger: true }) === null) return;
      try { await call('delete_article', { id: a.id }); toast('Deleted'); m.close(); onChange(); } catch (x) { toast(errText(x), 'error'); }
    }
  });
}

function reviewForm(a: Article, done: () => void) {
  const m = modal({ title: 'Record a review', subtitle: a.title, size: 'sm', body: `<form class="stack">
    <p class="muted small">Approve a guide only after a qualified person (for example a veterinary doctor, agronomist or extension officer) has checked it. Their name is shown with the guide.</p>
    <div class="seg seg-wide">${(['pending_review', 'approved'] as const).map((s) => `<label><input type="radio" name="status" value="${s}" ${a.review_status === s || (s === 'pending_review' && a.review_status === 'draft') ? 'checked' : ''}><span>${esc(t(`review.${s}`))}</span></label>`).join('')}</div>
    <label class="field"><span>Reviewer name and qualification</span><input name="reviewer" maxlength="120" value="${esc(a.reviewer)}" placeholder="e.g. Dr A. Okon, DVM"></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('set_article_review', { id: a.id, status: v.status, reviewer: v.reviewer }); } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Review saved'); m.close(); done();
    });
  });
}

async function articleForm(a: Article | null, done: () => void) {
  const sources = await call<KnowledgeSource[]>('list_sources');
  const levels = ['general', 'record_keeping', 'planning', 'professional_confirmation', 'vet_only', 'chemical_label'];
  const m = modal({ title: a ? 'Edit guide' : 'Add a guide', size: 'lg', body: `<form class="stack">
    ${a ? '<p class="callout callout-info">Saving changes sends the guide back for review.</p>' : ''}
    <div class="grid-2"><label class="field"><span>Title</span><input name="title" required maxlength="150" value="${esc(a?.title ?? '')}"></label>
      <label class="field"><span>Category</span><input name="category" required maxlength="60" list="art-cats" value="${esc(a?.category ?? '')}"><datalist id="art-cats"><option value="Record keeping"><option value="Health"><option value="Biosecurity"><option value="Feed"><option value="Crops"><option value="Crop health"><option value="Processing"><option value="Weather"><option value="Business"></datalist></label></div>
    <label class="field"><span>Summary</span><input name="summary" maxlength="400" value="${esc(a?.summary ?? '')}"></label>
    <label class="field"><span>Text</span><textarea name="body" rows="10" maxlength="20000" required>${esc(a?.body ?? '')}</textarea><small>Leave a blank line between paragraphs. Start a line with “- ” for a bullet.</small></label>
    <div class="grid-2"><label class="field"><span>Source</span><select name="source_id"><option value="">—</option>${sources.map((s) => `<option value="${s.id}" ${a?.source_id === s.id ? 'selected' : ''}>${esc(s.name)}</option>`).join('')}</select></label>
      <label class="field"><span>Source reference (page, date, link)</span><input name="source_ref" maxlength="300" value="${esc(a?.source_ref ?? '')}"></label></div>
    <div class="grid-3"><label class="field"><span>Kind of advice</span><select name="safety_level">${levels.map((l) => `<option value="${l}" ${a?.safety_level === l ? 'selected' : ''}>${esc(t(`safety.${l}`))}</option>`).join('')}</select></label>
      <label class="field"><span>Applies to</span><input name="applies_to" maxlength="300" value="${esc(a?.applies_to ?? '')}" placeholder="e.g. poultry,cassava"></label>
      <label class="field"><span>Language</span><select name="language"><option value="en" ${a?.language !== 'ib' ? 'selected' : ''}>English</option><option value="ib" ${a?.language === 'ib' ? 'selected' : ''}>Ibibio</option></select></label></div>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('save_article', { input: { id: a?.id ?? null, title: v.title, category: v.category, applies_to: v.applies_to, summary: v.summary, body: v.body, source_id: v.source_id ? Number(v.source_id) : null, source_ref: v.source_ref, safety_level: v.safety_level, language: v.language } }); }
      catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Guide saved'); m.close(); done();
    });
  });
}
