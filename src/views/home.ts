import { auth, call } from '../api';
import { esc } from '../lib/dom';
import { greeting, money } from '../lib/format';
import { icon } from '../lib/icons';
import { store } from '../store';
import { t } from '../lib/i18n';
import type { Dashboard, FarmOverview } from '../types';
import { dateLabel } from '../lib/format';
import type { ViewCtx } from './shell';
import { openGroupSheet, visibleGroups } from './shell';

/** Home: today's figures on a card, then the app's sections as tiles. Fits one phone screen. */
export async function render(ctx: ViewCtx) {
  const s = auth.session!;
  const first = (s.full_name || s.username).split(/\s+/)[0];
  ctx.header(t('page.home.title', {}, 'Today'), store.settings.business_name || '');
  const seller = auth.canSell();
  const [d, farm] = await Promise.all([seller ? call<Dashboard>('dashboard').catch(() => null) : Promise.resolve(null), call<FarmOverview>('farm_overview').catch(() => null)]);
  const mine = d?.scope === 'mine';
  const groups = visibleGroups();
  const alerts = [
    d && d.low_stock_count ? `<button type="button" class="home-alert" data-go="inventory" data-tab="items" data-low="1">${icon('alert', 16)}<span>${d.low_stock_count} running low</span></button>` : '',
    d && (d.expired_count || d.expiring_count) && !mine ? `<button type="button" class="home-alert" data-go="inventory" data-tab="expiry">${icon('calendar', 16)}<span>${d.expired_count ? `${d.expired_count} expired` : `${d.expiring_count} expiring`}</span></button>` : '',
    d && d.ready_collections ? `<button type="button" class="home-alert" data-go="receipts" data-fulfilment="ready">${icon('check', 16)}<span>${d.ready_collections} ready for pickup</span></button>` : '',
  ].filter(Boolean).join('');

  const today = new Intl.DateTimeFormat('en-NG', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }).format(new Date());
  const place = [store.settings.business_address].filter(Boolean).join('');
  const action = (go: string, ic: string, label: string, primary = false, tab = '') => `<button type="button" class="hero-btn${primary ? ' primary' : ''}" data-go="${go}"${tab ? ` data-tab="${tab}"` : ''}>${icon(ic, 19)}<span>${esc(label)}</span></button>`;
  const actions = [
    mod('livestock') ? action('livestock', 'bird', 'Daily records', true) : '',
    seller ? action('pos', 'till', t('nav.pos'), !mod('livestock')) : '',
    mod('crops') ? action('crops', 'sun', 'Crops') : '',
    (mod('palm') || mod('processing')) && !mod('livestock') ? action('palm-oil', 'mill', 'Processing', !seller) : '',
    auth.can('manager') ? action('expenses', 'wallet', 'Add expense') : mod('livestock') ? action('health', 'shield', 'Health') : '',
    action('advisory', 'chat', 'Ask FarmBuddy', false, 'ask'),
  ].filter(Boolean).join('');
  const kpi = (label: string, value: string, note: string, ic: string, go: string) => `<button type="button" class="kpi" data-go="${go}"><span class="kpi-icon">${icon(ic, 18)}</span><small>${esc(label)}</small><b>${esc(value)}</b><span>${esc(note)}</span></button>`;

  ctx.main.innerHTML = `
    <section class="hero-card today-hero">
      <div class="today-copy">
        <p class="today-date">${esc(today)}</p>
        <h2 class="today-hello">${esc(greeting())}, ${esc(first)}</h2>
        <p class="today-farm"><b>${esc(store.settings.business_name || 'My Farm')}</b>${place ? `<span>${esc(place)}</span>` : ''}${seller ? '' : `<span class="pill pill-mute">${s.role === 'adviser' ? 'Adviser' : 'Farm worker'}</span>`}</p>
      </div>
      <div class="hero-actions">${actions}</div>
      <svg class="today-art" viewBox="0 0 400 160" preserveAspectRatio="none" aria-hidden="true"><path d="M0 110 C80 90 170 92 250 104 C320 114 360 108 400 96 L400 160 L0 160Z" class="h1"/><path d="M0 132 C100 112 210 120 300 128 C350 132 380 128 400 122 L400 160 L0 160Z" class="h2"/></svg>
    </section>
    ${seller && d ? `<section class="kpi-row" aria-label="${mine ? 'Your sales today' : 'Sales today'}">
      ${kpi(mine ? 'Your sales today' : 'Sales today', money(d.today_sales_kobo, { whole: true }), `${d.today_receipts} receipt${d.today_receipts === 1 ? '' : 's'}`, 'till', 'dashboard')}
      ${kpi('Received today', money(d.today_collected_kobo, { whole: true }), 'cash, transfer and POS', 'cash', 'receipts')}
      ${kpi('Owed to you', money(d.outstanding_kobo, { whole: true }), d.owing_receipts ? `${d.owing_receipts} unpaid receipt${d.owing_receipts === 1 ? '' : 's'}` : 'all paid', 'people', 'customers')}
    </section>` : ''}
    ${alerts ? `<div class="home-alerts">${alerts}</div>` : ''}
    <div class="today-grid">
      ${farm ? farmCard(farm) : ''}
      ${farm?.money ? moneyCard(farm.money) : ''}
    </div>
    <section class="explore">
      <h2 class="section-title">Everything in FarmBuddy</h2>
      <div class="group-grid" aria-label="Sections">
      ${groups.map((g) => `<button type="button" class="group-tile tone-${g.tone}" data-group="${g.id}">
        <span class="group-icon">${icon(g.icon, 22)}</span>
        <span class="group-text"><b>${esc(t(g.label))}</b><small>${esc(t(g.hint))}</small></span>
        <span class="group-count">${g.tiles.length}</span>
      </button>`).join('')}
      </div>
    </section>`;

  ctx.main.onclick = (e) => {
    const t = e.target as HTMLElement;
    const g = t.closest<HTMLElement>('[data-group]');
    if (g) { openGroupSheet(ctx.go, g.dataset.group); return; }
    const done = t.closest<HTMLElement>('[data-rem-done]');
    if (done) { void call('set_reminder_status', { id: Number(done.dataset.remDone), status: 'done' }).then(() => render(ctx)); return; }
    const b = t.closest<HTMLElement>('[data-go]');
    if (b) ctx.go(b.dataset.go!, { tab: b.dataset.tab, low: b.dataset.low, fulfilment: b.dataset.fulfilment });
  };
}

const STAT_ICON: Record<string, string> = { livestock: 'bird', health: 'shield', crops: 'sun', processing: 'layers', advisory: 'help', 'palm-oil': 'mill', 'farm-reports': 'chart', expenses: 'wallet', storage: 'store', purchases: 'truck', customers: 'people' };
const mod = (k: string) => store.settings[`module_${k}`] !== '0';

/** The farm at a glance: each figure opens the page behind it. */
function farmCard(o: FarmOverview): string {
  const setup = !o.onboarding_done && auth.can('owner')
    ? `<div class="callout callout-info farm-setup">${icon('sun', 18)}<div><b>${esc(t('home.setup'))}</b><span>${esc(t('home.setup.hint'))}</span></div><button class="btn btn-primary btn-sm" data-go="onboarding">${esc(t('common.continue'))}</button></div>` : '';
  const vacc = o.vaccinations_due + o.vaccinations_overdue + o.vaccinations_need_review;
  const stat = (show: boolean, view: string, label: string, value: string | number, note = '', tone = '', tab = '') => show
    ? `<button type="button" class="farm-stat ${tone}" data-go="${view}" ${tab ? `data-tab="${tab}"` : ''}><span class="stat-icon">${icon(STAT_ICON[view] ?? 'info', 17)}</span><small>${esc(label)}</small><b>${esc(value)}</b>${note ? `<span>${esc(note)}</span>` : ''}</button>` : '';
  const stats = [
    stat(mod('livestock'), 'livestock', t('home.live_animals'), o.live_animals.toLocaleString('en-NG'), o.active_batches ? `${o.active_batches} batch${o.active_batches === 1 ? '' : 'es'}` : ''),
    stat(mod('livestock') && o.eggs_today > 0, 'livestock', t('home.eggs_today'), o.eggs_today.toLocaleString('en-NG')),
    stat(mod('livestock'), 'health', t('home.vacc_due'), vacc, o.vaccinations_overdue ? `${o.vaccinations_overdue} overdue` : o.vaccinations_need_review ? `${o.vaccinations_need_review} need vet review` : '', o.vaccinations_overdue ? 'is-bad' : vacc ? 'is-warn' : ''),
    stat(mod('crops'), 'crops', t('home.crops_growing'), o.growing_cycles, o.harvest_soon ? `${o.harvest_soon} harvest soon` : `${o.active_plots} plot${o.active_plots === 1 ? '' : 's'}`),
    stat(mod('processing') || mod('palm'), 'palm-oil', t('home.processing_open'), o.open_processing),
    stat(true, 'advisory', t('home.suggestions'), o.suggestions, '', o.suggestions ? 'is-warn' : '', 'suggestions'),
  ].join('');
  const reminders = o.reminders.slice(0, 4).map((r) => `<li class="${r.overdue ? 'is-overdue' : ''}">
      <button type="button" class="rem-check" data-rem-done="${r.id}" aria-label="Mark done">${icon('check', 14)}</button>
      <span>${esc(r.title)}${r.is_demo ? ` <em class="pill pill-mute">${esc(t('common.demo'))}</em>` : ''}</span><small>${esc(dateLabel(r.due_date))}</small></li>`).join('');
  return `${setup}<section class="farm-card" aria-label="${esc(t('home.farm_today'))}">
    <header><h2>${esc(t('home.farm_today'))}</h2>${o.demo_loaded ? `<span class="pill pill-warn">${esc(t('common.demo_data'))}</span>` : ''}</header>
    <div class="farm-stats">${stats}</div>
    ${reminders ? `<div class="farm-reminders"><h3>${esc(t('home.reminders'))}</h3><ul>${reminders}</ul><button type="button" class="link-btn" data-go="advisory" data-tab="reminders">${esc(t('common.open'))}</button></div>` : ''}
    ${o.low_inputs.length ? `<p class="farm-low">${icon('alert', 15)} ${o.low_inputs.map((x) => `${esc(x.name)} (${esc(x.stock_qty)} ${esc(x.unit)})`).join(', ')} running low</p>` : ''}
  </section>`;
}

/** Managers and owners: stock value, money owed both ways, this month's expenses and an estimated profit. */
function moneyCard(m: NonNullable<FarmOverview['money']>): string {
  const cell = (view: string, tab: string, label: string, value: number, note = '', tone = '') => `<button type="button" class="farm-stat ${tone}" data-go="${view}" ${tab ? `data-tab="${tab}"` : ''}><span class="stat-icon">${icon(STAT_ICON[view] ?? 'info', 17)}</span><small>${esc(label)}</small><b>${esc(money(value, { whole: true }))}</b>${note ? `<span>${esc(note)}</span>` : ''}</button>`;
  return `<section class="farm-card" aria-label="Money this month"><header><h2>Money this month</h2><small class="muted">from ${esc(dateLabel(m.month_from))}, demo excluded</small></header>
    <div class="farm-stats">
      ${cell('farm-reports', 'pl', 'Estimated profit', m.profit_estimate_kobo, `sales ${money(m.sales_this_month_kobo, { whole: true })}`, m.profit_estimate_kobo < 0 ? 'is-bad' : '')}
      ${cell('expenses', '', 'Expenses', m.expenses_this_month_kobo)}
      ${cell('storage', '', 'Inventory value', m.inventory_value_kobo, 'at cost')}
      ${cell('purchases', '', 'Owed to suppliers', m.owed_to_suppliers_kobo, '', m.owed_to_suppliers_kobo ? 'is-warn' : '')}
      ${cell('customers', '', 'Owed to you', m.owed_by_customers_kobo)}
    </div></section>`;
}
