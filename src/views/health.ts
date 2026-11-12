import { auth, call } from '../api';
import { $, $$, confirmDialog, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { dateLabel, isoDate } from '../lib/format';
import { icon } from '../lib/icons';
import { t, tl } from '../lib/i18n';
import type { Flock, HealthRecord, Vaccination, VaccStatus } from '../types';
import type { ViewCtx } from './shell';
import { fieldsHtml, formModal, readForm, reviewPill, tableHtml } from '../lib/ui';
import { SPECIES } from '../lib/content/farm';

type Tab = 'vaccinations' | 'records' | 'schedules';
let tab: Tab = 'vaccinations';
let openOnly = true;
let flockFilter: number | null = null;

const TONE: Record<VaccStatus, string> = { upcoming: 'pill-mute', due: 'pill-warn', overdue: 'pill-bad', completed: 'pill-ok', skipped: 'pill-mute', needs_review: 'pill-review' };
export const vaccPill = (s: VaccStatus) => `<span class="pill ${TONE[s]}">${esc(t(`vacc.${s}`))}</span>`;

/** Health observations and vaccinations. Records only: FarmBuddy never diagnoses or prescribes. */
export async function render(ctx: ViewCtx) {
  if (typeof ctx.params.flock === 'number') flockFilter = ctx.params.flock;
  if (ctx.params.tab) tab = ctx.params.tab as Tab;
  const manager = auth.can('manager');
  const flocks = await call<Flock[]>('list_flocks', { includeClosed: true });
  ctx.header(t('nav.health'), 'Vaccination plans and what you observed, treated and who advised you.', `
    <button class="btn btn-ghost" data-new-record>${icon('plus', 16)} Health record</button>
    ${manager ? `<button class="btn btn-primary" data-new-vacc>${icon('plus', 16)} Plan vaccination</button>` : ''}`);
  ctx.main.innerHTML = `<div class="callout callout-info">${icon('shield', 18)}<div><b>Records, not diagnosis</b><span>${esc(t('notice.no_diagnosis'))} A schedule from a general reference shows “${esc(t('vacc.needs_review'))}” until you record who confirmed it.</span></div></div>
    <div class="tabs" role="tablist">
      <button role="tab" class="tab" data-tab="vaccinations">Vaccinations</button>
      <button role="tab" class="tab" data-tab="records">Health records</button>
      <button role="tab" class="tab" data-tab="schedules">Schedule templates</button>
    </div>
    <div class="toolbar"><label class="field inline"><span>Batch</span><select data-flock><option value="">All batches</option>${flocks.map((f) => `<option value="${f.id}" ${flockFilter === f.id ? 'selected' : ''}>${esc(f.code)} · ${esc(tl('species', f.species))}${f.status === 'closed' ? ' (closed)' : ''}</option>`).join('')}</select></label>
      <label class="check inline" data-open-wrap><input type="checkbox" data-open-only ${openOnly ? 'checked' : ''}><span>Only planned</span></label></div>
    <div data-pane></div>`;
  const pane = $('[data-pane]', ctx.main)!;

  const show = async () => {
    $$('[data-tab]', ctx.main).forEach((b) => { b.classList.toggle('is-on', b.dataset.tab === tab); b.setAttribute('aria-selected', String(b.dataset.tab === tab)); });
    $('[data-open-wrap]', ctx.main)!.hidden = tab !== 'vaccinations';
    if (tab === 'schedules') { await schedules(pane, flocks); return; }
    if (tab === 'vaccinations') {
      const rows = await call<Vaccination[]>('list_vaccinations', { flockId: flockFilter, openOnly });
      pane.innerHTML = rows.length ? `<div class="table-scroll"><table class="table"><thead><tr><th>Date</th><th>Batch</th><th>Vaccine</th><th>Schedule from</th><th>Status</th><th></th></tr></thead><tbody>
        ${rows.map((v) => `<tr><td>${esc(dateLabel(v.status === 'completed' ? v.administered_date : v.due_date))}</td><td>${esc(v.flock_code)}</td>
          <td><b>${esc(v.vaccine_name)}</b>${v.disease ? `<small class="muted block">${esc(v.disease)}</small>` : ''}${v.lot_no ? `<small class="muted block">Lot ${esc(v.lot_no)}</small>` : ''}</td>
          <td>${esc(t(`vsource.${v.source}`))}${v.reviewed_by ? `<small class="muted block">${esc(v.reviewed_by)}</small>` : ''}</td>
          <td>${vaccPill(v.display_status)}${v.skip_reason ? `<small class="muted block">${esc(v.skip_reason)}</small>` : ''}${v.administered_by ? `<small class="muted block">by ${esc(v.administered_by)}</small>` : ''}</td>
          <td class="actions-col">${v.status === 'planned' ? `<button class="btn btn-sm btn-primary" data-give="${v.id}">Given</button>${manager ? `<button class="icon-btn" data-edit-vacc="${v.id}" aria-label="Edit">${icon('edit', 15)}</button><button class="icon-btn" data-skip="${v.id}" aria-label="Skip">${icon('close', 15)}</button><button class="icon-btn" data-del-vacc="${v.id}" aria-label="Delete">${icon('trash', 15)}</button>` : ''}` : ''}</td></tr>`).join('')}
        </tbody></table></div>` : emptyState(openOnly ? 'Nothing planned' : 'No vaccinations recorded', 'Plan each vaccination from your vet’s instructions, the vaccine label or the hatchery record.', manager ? `<button class="btn btn-primary" data-new-vacc>${icon('plus', 16)} Plan vaccination</button>` : '');
      pane.onclick = async (e) => {
        const el = e.target as HTMLElement;
        const give = el.closest<HTMLElement>('[data-give]');
        if (give) giveForm(rows.find((v) => v.id === Number(give.dataset.give))!, show);
        const ed = el.closest<HTMLElement>('[data-edit-vacc]');
        if (ed) vaccForm(flocks, rows.find((v) => v.id === Number(ed.dataset.editVacc))!, show);
        const sk = el.closest<HTMLElement>('[data-skip]');
        if (sk) {
          const reason = await confirmDialog({ title: 'Skip this vaccination?', message: 'It stays in the history as skipped.', confirm: 'Skip', reason: { label: 'Reason', required: true, placeholder: 'e.g. Vet advised against it' } });
          if (reason === null) return;
          try { await call('skip_vaccination', { id: Number(sk.dataset.skip), reason }); toast('Marked as skipped'); void show(); } catch (x) { toast(errText(x), 'error'); }
        }
        const del = el.closest<HTMLElement>('[data-del-vacc]');
        if (del) {
          if (await confirmDialog({ title: 'Delete this plan?', message: 'Only planned vaccinations can be deleted.', confirm: 'Delete', danger: true }) === null) return;
          try { await call('delete_vaccination', { id: Number(del.dataset.delVacc) }); toast('Deleted'); void show(); } catch (x) { toast(errText(x), 'error'); }
        }
        if (el.closest('[data-new-vacc]')) vaccForm(flocks, null, show);
      };
    } else {
      const rows = await call<HealthRecord[]>('list_health_records', { flockId: flockFilter });
      pane.innerHTML = rows.length ? `<div class="stack">${rows.map((h) => `<article class="record-card">
          <header><b>${esc(h.issue)}</b><span class="muted small">${esc(h.flock_code)} · ${esc(dateLabel(h.observed_date))}${h.affected ? ` · ${h.affected} affected` : ''}</span>${manager ? `<span class="record-actions"><button class="icon-btn" data-edit-rec="${h.id}" aria-label="Edit">${icon('edit', 15)}</button><button class="icon-btn" data-del-rec="${h.id}" aria-label="Delete">${icon('trash', 15)}</button></span>` : ''}</header>
          ${h.symptoms ? `<p><small class="muted">Observed:</small> ${esc(h.symptoms)}</p>` : ''}
          ${h.action_taken ? `<p><small class="muted">Action taken:</small> ${esc(h.action_taken)}</p>` : ''}
          ${h.vet_consulted || h.treatment ? `<p><small class="muted">Advice:</small> ${esc([h.vet_consulted && `from ${h.vet_consulted}`, h.treatment, h.dosage_as_prescribed && `as prescribed: ${h.dosage_as_prescribed}`, h.route].filter(Boolean).join(' · '))}</p>` : ''}
          ${h.withdrawal_until ? `<p class="warn-text">${icon('alert', 14)} Withdrawal period until ${esc(dateLabel(h.withdrawal_until))}: check the product label or prescriber before selling meat, eggs or milk.</p>` : ''}
          ${h.follow_up_date ? `<p class="small">Follow up on ${esc(dateLabel(h.follow_up_date))}</p>` : ''}
          <small class="muted">Recorded by ${esc(h.user_name)}</small></article>`).join('')}</div>`
        : emptyState('No health records', 'Record anything unusual you notice: what you saw, how many animals, what you did and who advised you.', `<button class="btn btn-primary" data-new-record>${icon('plus', 16)} Health record</button>`);
      pane.onclick = async (e) => {
        const el = e.target as HTMLElement;
        const ed = el.closest<HTMLElement>('[data-edit-rec]');
        if (ed) recordForm(flocks, rows.find((h) => h.id === Number(ed.dataset.editRec))!, show);
        const del = el.closest<HTMLElement>('[data-del-rec]');
        if (del) {
          if (await confirmDialog({ title: 'Delete this record?', message: 'The health history for this batch loses this entry.', confirm: 'Delete', danger: true }) === null) return;
          try { await call('delete_health_record', { id: Number(del.dataset.delRec) }); toast('Deleted'); void show(); } catch (x) { toast(errText(x), 'error'); }
        }
        if (el.closest('[data-new-record]')) recordForm(flocks, null, show);
      };
    }
  };

  ctx.main.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-tab]');
    if (b) { tab = b.dataset.tab as Tab; void show(); }
  });
  ctx.main.addEventListener('change', (e) => {
    const el = e.target as HTMLInputElement;
    if (el.matches('[data-flock]')) { flockFilter = el.value ? Number(el.value) : null; void show(); }
    if (el.matches('[data-open-only]')) { openOnly = el.checked; void show(); }
  });
  $('[data-new-vacc]')?.addEventListener('click', () => vaccForm(flocks, null, show));
  $('[data-new-record]')?.addEventListener('click', () => recordForm(flocks, null, show));
  await show();
}

const flockOptions = (flocks: Flock[], selected: number | null) => flocks.filter((f) => f.status === 'active' || f.id === selected)
  .map((f) => `<option value="${f.id}" ${selected === f.id ? 'selected' : ''}>${esc(f.code)} · ${esc(tl('species', f.species))}</option>`).join('');

function vaccForm(flocks: Flock[], v: Vaccination | null, done: () => void) {
  const m = modal({ title: v ? 'Edit vaccination plan' : 'Plan a vaccination', size: 'md', body: `<form class="stack">
    <div class="grid-2"><label class="field"><span>Batch</span><select name="flock_id" required><option value="">Choose…</option>${flockOptions(flocks, v?.flock_id ?? flockFilter)}</select></label>
      <label class="field"><span>Planned date</span><input type="date" name="due_date" required value="${esc(v?.due_date ?? isoDate(new Date()))}"></label></div>
    <div class="grid-2"><label class="field"><span>Vaccine name</span><input name="vaccine_name" required maxlength="120" value="${esc(v?.vaccine_name ?? '')}"></label>
      <label class="field"><span>Disease it protects against</span><input name="disease" maxlength="120" value="${esc(v?.disease ?? '')}"></label></div>
    <div class="grid-2"><label class="field"><span>Dose (as instructed)</span><input name="dose" maxlength="120" value="${esc(v?.dose ?? '')}"></label>
      <label class="field"><span>How it is given</span><input name="route" maxlength="60" list="routes" value="${esc(v?.route ?? '')}"><datalist id="routes"><option value="Drinking water"><option value="Eye drop"><option value="Injection"><option value="Spray"><option value="Wing web"></datalist></label></div>
    <div class="grid-3"><label class="field"><span>Manufacturer</span><input name="manufacturer" maxlength="120" value="${esc(v?.manufacturer ?? '')}"></label>
      <label class="field"><span>Lot number</span><input name="lot_no" maxlength="60" value="${esc(v?.lot_no ?? '')}"></label>
      <label class="field"><span>Vaccine expiry</span><input type="date" name="vaccine_expiry" value="${esc(v?.vaccine_expiry ?? '')}"></label></div>
    <fieldset class="field"><legend>Where did this schedule come from?</legend>
      <div class="seg seg-wide">${(['vet', 'label', 'hatchery', 'pending'] as const).map((s) => `<label><input type="radio" name="source" value="${s}" ${(v?.source ?? 'vet') === s ? 'checked' : ''}><span>${esc(t(`vsource.${s}`))}</span></label>`).join('')}</div></fieldset>
    <label class="field" data-reviewed><span>Name of the vet, hatchery or product</span><input name="reviewed_by" maxlength="120" value="${esc(v?.reviewed_by ?? '')}" placeholder="e.g. Dr Etim, Uyo vet clinic"></label>
    <label class="field"><span>Notes</span><textarea name="notes" rows="2" maxlength="1000">${esc(v?.notes ?? '')}</textarea></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  const sync = () => { $('[data-reviewed]', form)!.hidden = (form.elements.namedItem('source') as RadioNodeList).value === 'pending'; };
  form.addEventListener('change', sync);
  sync();
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const x = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('save_vaccination', { input: { id: v?.id ?? null, flock_id: Number(x.flock_id) || 0, due_date: x.due_date, vaccine_name: x.vaccine_name, disease: x.disease, manufacturer: x.manufacturer,
          lot_no: x.lot_no, vaccine_expiry: x.vaccine_expiry, dose: x.dose, route: x.route, source: x.source, reviewed_by: x.source === 'pending' ? '' : x.reviewed_by, notes: x.notes } });
      } catch (err) { $('[data-err]', form)!.textContent = errText(err); return; }
      toast('Vaccination saved'); m.close(); done();
    });
  });
}

function giveForm(v: Vaccination, done: () => void) {
  const m = modal({ title: `Record ${v.vaccine_name} as given`, subtitle: v.flock_code, size: 'sm', body: `<form class="stack">
    ${v.source === 'pending' ? `<div class="callout callout-warn"><div><b>${esc(t('vacc.needs_review'))}</b><span>This plan has not been confirmed by a vet. Record it only if it was actually given.</span></div></div>` : ''}
    <label class="field"><span>Date given</span><input type="date" name="date" required value="${isoDate(new Date())}" max="${isoDate(new Date())}"></label>
    <label class="field"><span>Given by</span><input name="by" required maxlength="120"></label>
    <label class="field"><span>Lot number</span><input name="lot" maxlength="60" value="${esc(v.lot_no)}"></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const x = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('complete_vaccination', { id: v.id, date: x.date, givenBy: x.by, lotNo: x.lot }); } catch (err) { $('[data-err]', form)!.textContent = errText(err); return; }
      toast('Recorded'); m.close(); done();
    });
  });
}

function recordForm(flocks: Flock[], h: HealthRecord | null, done: () => void) {
  const m = modal({ title: h ? 'Edit health record' : 'Health record', size: 'md', body: `<form class="stack">
    <div class="grid-2"><label class="field"><span>Batch</span><select name="flock_id" required><option value="">Choose…</option>${flockOptions(flocks, h?.flock_id ?? flockFilter)}</select></label>
      <label class="field"><span>Date noticed</span><input type="date" name="observed_date" required value="${esc(h?.observed_date ?? isoDate(new Date()))}"></label></div>
    <div class="grid-2"><label class="field"><span>What did you notice?</span><input name="issue" required maxlength="150" value="${esc(h?.issue ?? '')}" placeholder="e.g. Birds not eating"></label>
      <label class="field"><span>Number affected</span><input name="affected" inputmode="numeric" value="${h?.affected ?? 0}"></label></div>
    <label class="field"><span>Signs observed</span><textarea name="symptoms" rows="2" maxlength="1000">${esc(h?.symptoms ?? '')}</textarea></label>
    <label class="field"><span>Action taken</span><input name="action_taken" maxlength="500" value="${esc(h?.action_taken ?? '')}" placeholder="e.g. Separated sick birds, called vet"></label>
    <details class="stack" ${h?.treatment || h?.vet_consulted ? 'open' : ''}><summary><b>Treatment advised by a vet</b> <small class="muted">(record only what was prescribed)</small></summary>
      <div class="grid-2"><label class="field"><span>Vet or professional consulted</span><input name="vet_consulted" maxlength="120" value="${esc(h?.vet_consulted ?? '')}"></label>
        <label class="field"><span>Product prescribed</span><input name="treatment" maxlength="150" value="${esc(h?.treatment ?? '')}"></label></div>
      <div class="grid-2"><label class="field"><span>Dose as prescribed</span><input name="dosage_as_prescribed" maxlength="200" value="${esc(h?.dosage_as_prescribed ?? '')}"></label>
        <label class="field"><span>How given</span><input name="route" maxlength="60" value="${esc(h?.route ?? '')}"></label></div>
      <div class="grid-3"><label class="field"><span>Start</span><input type="date" name="start_date" value="${esc(h?.start_date ?? '')}"></label>
        <label class="field"><span>End</span><input type="date" name="end_date" value="${esc(h?.end_date ?? '')}"></label>
        <label class="field"><span>Withdrawal (days)</span><input name="withdrawal_days" inputmode="numeric" value="${h?.withdrawal_days ?? 0}"><small>From the label or prescriber.</small></label></div>
    </details>
    <div class="grid-2"><label class="field"><span>Follow-up date</span><input type="date" name="follow_up_date" value="${esc(h?.follow_up_date ?? '')}"></label>
      <label class="field"><span>Notes</span><input name="notes" maxlength="1000" value="${esc(h?.notes ?? '')}"></label></div>
    <p class="muted small">${esc(t('notice.no_diagnosis'))}</p>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const x = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('save_health_record', { input: { id: h?.id ?? null, flock_id: Number(x.flock_id) || 0, observed_date: x.observed_date, issue: x.issue, symptoms: x.symptoms, affected: Math.round(Number(x.affected) || 0),
          action_taken: x.action_taken, vet_consulted: x.vet_consulted, treatment: x.treatment, dosage_as_prescribed: x.dosage_as_prescribed, route: x.route, start_date: x.start_date, end_date: x.end_date,
          withdrawal_days: Math.round(Number(x.withdrawal_days) || 0), follow_up_date: x.follow_up_date, notes: x.notes } });
      } catch (err) { $('[data-err]', form)!.textContent = errText(err); return; }
      toast('Health record saved'); m.close(); done();
    });
  });
}

interface ProtocolItem { day_offset: number; vaccine_name: string; disease: string; route: string; dose: string; notes: string }
interface Protocol { id: number; name: string; species: string; source_text: string; review_status: string; reviewer: string; review_date: string; notes: string; items: ProtocolItem[] }

/**
 * Reusable vaccination schedule templates. A schedule is entered from the
 * vet's plan, the hatchery record or a vaccine label, and stays "not yet
 * reviewed" until an owner or adviser approves it. Applying it to a batch
 * plans each vaccination on placement date + day offset.
 */
async function schedules(pane: HTMLElement, flocks: Flock[]) {
  const list = await call<Protocol[]>('list_protocols');
  const advise = auth.canAdvise();
  const review = auth.canReview();
  const active = flocks.filter((f) => f.status === 'active');
  pane.innerHTML = `<p class="muted small">Enter a schedule once from your vet or hatchery, then apply it to each new batch. Vaccinations planned from an approved schedule are marked as vet-confirmed; others show “needs review”.</p>
    ${advise ? `<div><button class="btn btn-primary" data-new-proto>${icon('plus', 16)} New schedule</button></div>` : ''}
    ${list.length ? list.map((p) => `<section class="panel"><header class="panel-head"><div><h2>${esc(p.name)}</h2><p class="muted small">${esc(tl('species', p.species))} · source: ${esc(p.source_text)} ${reviewPill(p.review_status)}${p.reviewer ? ` <small>${esc(p.reviewer)} ${esc(p.review_date)}</small>` : ''}</p></div>
      <div class="row-actions">${advise && active.length ? `<button class="btn btn-ghost btn-sm" data-apply="${p.id}">Apply to a batch</button>` : ''}${review && p.review_status !== 'approved' ? `<button class="btn btn-ghost btn-sm" data-approve="${p.id}">Approve</button>` : ''}${advise ? `<button class="icon-btn" data-edit-proto="${p.id}" aria-label="Edit">${icon('edit', 15)}</button>` : ''}${auth.can('manager') ? `<button class="icon-btn" data-del-proto="${p.id}" aria-label="Delete">${icon('trash', 15)}</button>` : ''}</div></header>
      ${tableHtml(['Day#', 'Vaccine', 'Disease', 'Route', 'Dose (as on label or prescription)', 'Notes'], p.items.map((i) => [String(i.day_offset), esc(i.vaccine_name), esc(i.disease), esc(i.route), esc(i.dose), esc(i.notes)]))}</section>`).join('')
      : emptyState('No schedule templates yet', 'Add the vaccination schedule your vet or hatchery gave you, so each new batch gets the same plan.')}`;
  const form = (p: Protocol | null) => {
    const itemRow = (i?: ProtocolItem) => `<tr data-item><td><input name="day" inputmode="numeric" value="${i?.day_offset ?? ''}" aria-label="Day"></td><td><input name="vaccine" value="${esc(i?.vaccine_name ?? '')}" aria-label="Vaccine"></td><td><input name="disease" value="${esc(i?.disease ?? '')}" aria-label="Disease"></td><td><input name="route" value="${esc(i?.route ?? '')}" aria-label="Route" placeholder="e.g. drinking water"></td><td><input name="dose" value="${esc(i?.dose ?? '')}" aria-label="Dose"></td><td><button type="button" class="icon-btn" data-rm aria-label="Remove">${icon('trash', 15)}</button></td></tr>`;
    const m = modal({ title: p ? `Edit ${p.name}` : 'New vaccination schedule', size: 'lg', body: `<form class="stack">
      ${fieldsHtml([{ name: 'name', label: 'Name', value: p?.name ?? '', required: true, pair: true }, { name: 'species', label: 'For', type: 'select', options: SPECIES.map((x): [string, string] => [x, tl('species', x)]), value: p?.species ?? 'broiler' },
        { name: 'source_text', label: 'Where this schedule comes from', value: p?.source_text ?? '', required: true, placeholder: 'e.g. Dr Etim (vet), hatchery leaflet, vaccine label' }])}
      <div class="table-scroll"><table class="table compact form-table"><thead><tr><th>Day of age</th><th>Vaccine</th><th>Disease</th><th>Route</th><th>Dose</th><th></th></tr></thead><tbody data-items>${(p?.items.length ? p.items : [undefined, undefined]).map((i) => itemRow(i)).join('')}</tbody></table></div>
      <div><button type="button" class="btn btn-ghost btn-sm" data-more>${icon('plus', 14)} Add a row</button></div>
      ${fieldsHtml([{ name: 'notes', label: 'Notes', type: 'textarea', value: p?.notes ?? '' }])}
      <p class="muted small">Saving changes sets the schedule back to "not yet reviewed".</p>
      <p class="form-error" role="alert" data-err></p>
      <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
    const f = m.el.querySelector('form')!;
    f.addEventListener('click', (e) => {
      const el = e.target as HTMLElement;
      if (el.closest('[data-more]')) $('[data-items]', f)!.insertAdjacentHTML('beforeend', itemRow());
      if (el.closest('[data-rm]')) el.closest('tr')?.remove();
    });
    f.addEventListener('submit', (e) => {
      e.preventDefault();
      const v = readForm(f);
      const items = $$<HTMLTableRowElement>('[data-item]', f).map((r) => {
        const g = (n: string) => (r.querySelector(`[name=${n}]`) as HTMLInputElement).value.trim();
        return { day_offset: Math.round(Number(g('day')) || 0), vaccine_name: g('vaccine'), disease: g('disease'), route: g('route'), dose: g('dose'), notes: '' };
      }).filter((i) => i.vaccine_name);
      void withBusy($<HTMLButtonElement>('.btn-primary', f), async () => {
        try { await call('save_protocol', { input: { id: p?.id ?? null, name: v.name, species: v.species, source_text: v.source_text, notes: v.notes, items } }); }
        catch (x) { $('[data-err]', f)!.textContent = errText(x); return; }
        toast('Schedule saved'); m.close(); void schedules(pane, flocks);
      });
    });
  };
  pane.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-new-proto]')) form(null);
    const ed = el.closest<HTMLElement>('[data-edit-proto]');
    if (ed) form(list.find((p) => p.id === Number(ed.dataset.editProto)) ?? null);
    const ap = el.closest<HTMLElement>('[data-approve]');
    if (ap) {
      if (await confirmDialog({ title: 'Approve this schedule?', message: 'Approve only if a vet has confirmed it for your birds or animals. Your name and today\'s date are saved.', confirm: 'Approve' }) === null) return;
      try { await call('review_protocol', { id: Number(ap.dataset.approve), approved: true, reviewer: auth.session?.full_name || auth.session?.username || '' }); toast('Approved'); void schedules(pane, flocks); } catch (x) { toast(errText(x), 'error'); }
    }
    const del = el.closest<HTMLElement>('[data-del-proto]');
    if (del && await confirmDialog({ title: 'Delete this schedule?', message: 'Vaccinations already planned from it stay.', confirm: 'Delete', danger: true }) !== null) {
      try { await call('delete_protocol', { id: Number(del.dataset.delProto) }); void schedules(pane, flocks); } catch (x) { toast(errText(x), 'error'); }
    }
    const a = el.closest<HTMLElement>('[data-apply]');
    if (a) {
      const p = list.find((x) => x.id === Number(a.dataset.apply))!;
      const match = active.filter((f) => f.species === p.species);
      formModal({ title: `Apply ${p.name}`, intro: `Each vaccination is planned on the batch's placement date plus its day. Vaccinations already planned with the same name and date are not added twice.`, fields: [
        { name: 'flock', label: 'Batch', type: 'select', options: (match.length ? match : active).map((f): [string, string] => [String(f.id), `${f.code} · ${tl('species', f.species)} · placed ${f.placed_date}`]), required: true },
      ], submitLabel: 'Apply', submit: async (v) => {
        const n = await call<number>('apply_protocol', { protocolId: p.id, flockId: Number(v.flock) });
        toast(`${n} vaccination${n === 1 ? '' : 's'} planned`);
      } });
    }
  };
}
