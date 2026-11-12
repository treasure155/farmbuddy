import { auth, call } from '../api';
import { $, confirmDialog, emptyState, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { icon } from '../lib/icons';
import { ibibioCoverage, LANGUAGES, t } from '../lib/i18n';
import { LGAS, STATES } from '../lib/content/nigeria';
import { store } from '../store';
import type { Farm, FarmOverview } from '../types';
import type { ViewCtx } from './shell';
import { tableHtml } from '../lib/ui';

const MODULES: Array<{ key: string; title: string; hint: string }> = [
  { key: 'livestock', title: 'Livestock, poultry and fish', hint: 'Batches, daily records, health and vaccination.' },
  { key: 'crops', title: 'Crop farming', hint: 'Plots, planting, field work and harvests.' },
  { key: 'processing', title: 'Processing', hint: 'Palm oil, garri, flour and feed batches with cost per unit.' },
  { key: 'palm', title: 'Palm oil business', hint: 'Buy or harvest palm fruit, process it, sell oil and kernel, with profit for each run.' },
  { key: 'sales', title: 'Selling produce', hint: 'Sell produce and shifts. Receipts, buyers and reports stay available.' },
];

/** My Farm: farm profiles, which modules are on, language, setup and demo data. */
export async function render(ctx: ViewCtx) {
  const owner = auth.can('owner');
  const manager = auth.can('manager');
  ctx.header(t('nav.farm'), 'Your farms, what FarmBuddy shows, language and demo data.', manager ? `<button class="btn btn-primary" data-new-farm>${icon('plus', 16)} Add farm</button>` : '');
  const [farms, overview] = await Promise.all([call<Farm[]>('list_farms', { includeArchived: true }), call<FarmOverview>('farm_overview')]);
  const cov = ibibioCoverage();

  ctx.main.innerHTML = `<div class="stack-lg">
    <section class="panel"><header class="panel-head"><h2>Farms</h2><p class="muted small">Every livestock batch, plot and processing batch can belong to one of these farms.</p></header>
      ${farms.length ? `<div class="card-grid">${farms.map((f) => `<article class="farm-tile ${f.status === 'archived' ? 'is-closed' : ''}">
        <div class="farm-tile-top"><b>${esc(f.name)}</b>${f.is_demo ? `<span class="pill pill-warn">${esc(t('common.demo'))}</span>` : ''}${f.status === 'archived' ? '<span class="pill pill-mute">Archived</span>' : ''}</div>
        <span class="muted small">${esc([f.community, f.lga, f.state].filter(Boolean).join(', ') || 'No location yet')}</span>
        ${f.activities ? `<span class="small">${esc(f.activities.split(',').map((a) => t(`opt.manage.${a}`, {}, a)).join(' · '))}</span>` : ''}
        ${manager && !f.is_demo ? `<div class="row-actions"><button class="btn btn-ghost btn-sm" data-edit-farm="${f.id}">${icon('edit', 14)} Edit</button>${owner ? `<button class="btn btn-ghost btn-sm" data-archive="${f.id}" data-restore="${f.status === 'archived' ? 1 : 0}">${f.status === 'archived' ? 'Restore' : 'Archive'}</button>` : ''}</div>` : ''}
      </article>`).join('')}</div>` : emptyState('No farm yet', owner ? 'Run the guided setup to create your farm.' : 'Ask the owner to set up the farm.', owner ? `<button class="btn btn-primary" data-setup>${icon('sun', 16)} Set up my farm</button>` : '')}
    </section>

    ${owner ? `<section class="panel"><header class="panel-head"><h2>What FarmBuddy shows</h2><p class="muted small">Switch off what you do not use. Nothing is deleted; switch it back on any time.</p></header>
      <form class="stack" data-modules><div class="toggle-tiles">${MODULES.map((m) => `<label class="check toggle-tile"><input type="checkbox" name="module_${m.key}" ${store.settings[`module_${m.key}`] !== '0' ? 'checked' : ''}><span><b>${esc(m.title)}</b><br><small class="muted">${esc(m.hint)}</small></span></label>`).join('')}</div>
        <div class="form-actions"><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form></section>

    <section class="panel"><header class="panel-head"><h2>Language</h2><p class="muted small">Ibibio: ${cov.done} of ${cov.total} screen texts approved so far. Untranslated text shows in English until fluent Ibibio reviewers approve it. Your own names and notes are never changed.</p></header>
      <div class="seg" data-lang>${LANGUAGES.map((l) => `<label><input type="radio" name="language" value="${l.code}" ${(store.settings.language || 'en') === l.code ? 'checked' : ''}><span>${esc(l.name)}</span></label>`).join('')}</div></section>

    <section class="panel"><header class="panel-head"><h2>Farm template</h2><p class="muted small">Start from one of 14 farm types: it adds categories, units and starter item names (no prices or stock), switches on the modules you need, and picks the home cards, check-ups and reports to show first. It never removes anything.${store.settings.farm_template ? ` Current: <b>${esc(store.settings.farm_template.replace(/_/g, ' '))}</b>.` : ''}</p></header>
      <div><button class="btn btn-ghost" data-templates>${icon('layers', 16)} Choose a farm template</button></div></section>

    <section class="panel"><header class="panel-head"><h2>Knowledge updates</h2><p class="muted small">Import a FarmBuddy content package (articles, sources, vaccination schedules, feed guides, season outlooks) without reinstalling. Your own articles and records are never overwritten. ${store.settings.last_content_update ? `Last update: ${esc(store.settings.last_content_update)}.` : 'No update imported yet.'}</p></header>
      <form class="stack" data-content-form><label class="check"><input type="checkbox" name="content_require_signature" ${store.settings.content_require_signature === '1' ? 'checked' : ''}><span><b>Only accept signed packages</b><br><small class="muted">Recommended. Unsigned packages are refused; signed ones are checked against the FarmBuddy key.</small></span></label>
        <div class="row-actions"><button type="button" class="btn btn-primary" data-import>${icon('download', 16)} Import a content package</button><button type="button" class="btn btn-ghost" data-versions>Update history</button><button class="btn btn-ghost">Save setting</button></div></form></section>

    <section class="panel"><header class="panel-head"><h2>Guided setup</h2><p class="muted small">Answer the setup questions again to update your farm profile and modules. Records you already have are kept and not duplicated.</p></header>
      <div><button class="btn btn-ghost" data-setup>${icon('refresh', 16)} Run setup again</button></div></section>

    <section class="panel"><header class="panel-head"><h2>Demo data</h2><p class="muted small">A fictional demo farm to explore FarmBuddy: a 500-bird broiler batch, 300 layers with eggs sold and used, goats with tags, a mating and twins, a catfish pond with water readings, cassava and maize plots with a harvest in store, a palm oil batch (100 bunches, ₦265,000 in costs), a feed purchase with ₦30,000 still owed, a sale on part credit, two stores with a transfer and spoilage, weather readings, illustrative market prices, and five suppliers and five buyers. Every record is marked Demo, figures are illustrative, not market prices, and demo money is left out of your reports unless you ask for it.</p></header>
      <div class="row-actions">${overview.demo_loaded ? `<span class="pill pill-warn">Demo data is loaded</span><button class="btn btn-danger" data-clear-demo>${icon('trash', 16)} Remove demo data</button>` : `<button class="btn btn-ghost" data-load-demo>${icon('download', 16)} Load demo data</button>`}</div></section>` : ''}
  </div>`;

  const refresh = () => render(ctx);
  ctx.main.onclick = async (e) => {
    const el = e.target as HTMLElement;
    if (el.closest('[data-setup]')) { ctx.go('onboarding'); return; }
    const ed = el.closest<HTMLElement>('[data-edit-farm]');
    if (ed) { farmForm(farms.find((f) => f.id === Number(ed.dataset.editFarm)) ?? null, refresh); return; }
    const ar = el.closest<HTMLElement>('[data-archive]');
    if (ar) {
      const restore = ar.dataset.restore === '1';
      if (!restore && await confirmDialog({ title: 'Archive this farm?', message: 'It disappears from lists, but every record stays for reports. You can restore it later.', confirm: 'Archive' }) === null) return;
      try { await call('archive_farm', { id: Number(ar.dataset.archive), restore }); toast(restore ? 'Farm restored' : 'Farm archived'); refresh(); } catch (x) { toast(errText(x), 'error'); }
      return;
    }
    if (el.closest('[data-load-demo]')) {
      withBusy(el.closest('button'), async () => { await call('load_demo_data'); toast('Demo data loaded. Look for the Demo labels.'); refresh(); });
      return;
    }
    if (el.closest('[data-clear-demo]')) {
      if (await confirmDialog({ title: 'Remove all demo data?', message: 'Every record marked Demo is deleted. Your own records are not touched.', confirm: 'Remove demo data', danger: true }) === null) return;
      try {
        const r = await call<{ kept_suppliers: number; kept_customers: number }>('clear_demo_data');
        toast(r.kept_suppliers || r.kept_customers ? 'Demo data removed. Demo contacts you used in real sales or purchases were kept.' : 'Demo data removed');
        refresh();
      } catch (x) { toast(errText(x), 'error'); }
    }
  };
  $('[data-new-farm]')?.addEventListener('click', () => farmForm(null, refresh));
  $('[data-templates]', ctx.main)?.addEventListener('click', () => void templatePicker(refresh));
  $('[data-content-form]', ctx.main)?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const on = ((e.currentTarget as HTMLFormElement).elements.namedItem('content_require_signature') as HTMLInputElement).checked;
    try { await call('save_settings', { values: { content_require_signature: on ? '1' : '0' } }); await store.loadSettings(); toast('Saved'); } catch (x) { toast(errText(x), 'error'); }
  });
  $('[data-import]', ctx.main)?.addEventListener('click', (e) => withBusy(e.currentTarget as HTMLButtonElement, async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const path = await open({ multiple: false, title: 'Choose a FarmBuddy content package', filters: [{ name: 'Content package', extensions: ['json', 'fbpkg'] }] });
      if (typeof path !== 'string') return;
      const r = await call<{ package_id: string; version: number; signed: boolean; sources: number; articles_added: number; articles_updated: number; articles_skipped: number; protocols: number; feed_guides: number; outlooks: number }>('import_content_package', { path });
      await store.loadSettings();
      modal({ title: `Imported ${r.package_id} version ${r.version}`, size: 'sm', body: `<ul class="stack-sm"><li>${r.signed ? 'Signed and verified' : 'Unsigned: everything arrives as “not yet reviewed”'}</li><li>${r.articles_added} new articles, ${r.articles_updated} updated, ${r.articles_skipped} kept as they were (written on this farm or already current)</li><li>${r.sources} sources, ${r.protocols} vaccination schedules, ${r.feed_guides} feed guides, ${r.outlooks} season outlooks</li></ul>` });
      refresh();
    } catch (x) { toast(errText(x), 'error'); }
  }));
  $('[data-versions]', ctx.main)?.addEventListener('click', async () => {
    const v = await call<Array<{ package_id: string; version: number; title: string; published_date: string; signed: boolean; summary: string; imported_at: string; user_name: string }>>('list_content_versions');
    modal({ title: 'Knowledge update history', size: 'lg', body: tableHtml(['Imported', 'Package', 'Version#', 'Published', 'Signed', 'What changed', 'By'], v.map((x) => [esc(x.imported_at), esc(x.title || x.package_id), String(x.version), esc(x.published_date), x.signed ? 'Yes' : 'No', esc(x.summary), esc(x.user_name)]), 'No content packages imported yet.') });
  });
  $('[data-modules]', ctx.main)?.addEventListener('submit', (e) => {
    e.preventDefault();
    const form = e.currentTarget as HTMLFormElement;
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      const values = Object.fromEntries(MODULES.map((m) => [`module_${m.key}`, v[`module_${m.key}`]]));
      await call('save_settings', { values });
      await store.loadSettings();
      toast('Saved');
      ctx.refreshChrome();
    });
  });
  $('[data-lang]', ctx.main)?.addEventListener('change', async (e) => {
    const value = (e.target as HTMLInputElement).value;
    try { await call('save_settings', { values: { language: value } }); await store.loadSettings(); toast(value === 'ib' ? 'Ibibio selected' : 'English selected'); setTimeout(() => location.reload(), 600); } catch (x) { toast(errText(x), 'error'); }
  });
}

export function farmForm(f: Farm | null, done: () => void) {
  const lgas = (state: string) => (LGAS[state] ?? []).map((l) => `<option value="${esc(l)}">`).join('');
  const m = modal({ title: f ? `Edit ${f.name}` : 'Add a farm', size: 'md', body: `<form class="stack">
    <label class="field"><span>Farm name</span><input name="name" required maxlength="120" value="${esc(f?.name ?? '')}" autofocus></label>
    <div class="grid-2"><label class="field"><span>State</span><select name="state"><option value="">Choose…</option>${STATES.map((s) => `<option ${f?.state === s ? 'selected' : ''}>${esc(s)}</option>`).join('')}</select></label>
      <label class="field"><span>LGA</span><input name="lga" list="farm-lgas" maxlength="80" value="${esc(f?.lga ?? '')}"><datalist id="farm-lgas">${lgas(f?.state ?? '')}</datalist></label></div>
    <div class="grid-2"><label class="field"><span>Community or town</span><input name="community" maxlength="120" value="${esc(f?.community ?? '')}"></label>
      <label class="field"><span>Phone</span><input name="phone" maxlength="40" inputmode="tel" value="${esc(f?.phone ?? '')}"></label></div>
    <label class="field"><span>Address</span><input name="address" maxlength="300" value="${esc(f?.address ?? '')}"></label>
    <label class="field"><span>Notes</span><textarea name="notes" rows="2" maxlength="1000">${esc(f?.notes ?? '')}</textarea></label>
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  (form.elements.namedItem('state') as HTMLSelectElement).addEventListener('change', (e) => { $('#farm-lgas', form)!.innerHTML = lgas((e.target as HTMLSelectElement).value); });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('save_farm', { input: { id: f?.id ?? null, name: v.name, state: v.state, lga: v.lga, community: v.community, address: v.address, phone: v.phone, activities: f?.activities ?? '', notes: v.notes } }); }
      catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Farm saved'); m.close(); done();
    });
  });
}

interface FarmTemplate { key: string; name: string; summary: string; modules: string[]; categories: string[]; units: string[]; items: Array<[string, string]>; questions: string[]; cards: string[]; reports: string[] }

/** The 14 starter farm templates. Applying one only adds; nothing is removed. */
async function templatePicker(done: () => void) {
  const list = await call<FarmTemplate[]>('list_farm_templates');
  const m = modal({ title: 'Choose a farm template', subtitle: 'Adds starter categories and item names without prices. You can apply more than one.', size: 'lg', body: `<div class="card-grid">${list.map((t) => `<article class="farm-tile ${store.settings.farm_template === t.key ? 'is-on' : ''}">
      <div class="farm-tile-top"><b>${esc(t.name)}</b>${store.settings.farm_template === t.key ? '<span class="pill pill-ok">Current</span>' : ''}</div>
      <span class="small">${esc(t.summary)}</span>
      <span class="muted small">Starter items: ${esc(t.items.map((i) => i[0]).join(', '))}</span>
      <div class="row-actions"><button class="btn btn-primary btn-sm" data-apply-tpl="${t.key}">Use this template</button></div></article>`).join('')}</div>` });
  m.el.addEventListener('click', async (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>('[data-apply-tpl]');
    if (!b) return;
    await withBusy(b, async () => {
      try {
        const r = await call<{ categories: number; items: number; modules: string[] }>('apply_farm_template', { key: b.dataset.applyTpl });
        await store.loadSettings();
        toast(`Template applied: ${r.items} starter items and ${r.categories} categories added. Set prices and stock in Farm stock.`);
        m.close(); done();
      } catch (x) { toast(errText(x), 'error'); }
    });
  });
}
