import { auth, call } from '../api';
import { $, esc, modal } from '../lib/dom';
import { icon } from '../lib/icons';
import { t } from '../lib/i18n';
import { store } from '../store';
import type { Role } from '../types';

export interface ViewCtx {
  main: HTMLElement;
  params: Record<string, unknown>;
  header: (title: string, subtitle?: string, actions?: string) => void;
  go: (view: string, params?: Record<string, unknown>) => void;
  refreshChrome: () => void;
}

type Loader = () => Promise<{ render: (ctx: ViewCtx) => Promise<void> | void }>;
export interface ViewDef { id: string; label: string; icon: string; min: Role; when?: () => boolean; load: Loader }

const mod = (k: string) => () => store.settings[`module_${k}`] !== '0';
const sells = () => auth.canSell();
const sellMod = () => sells() && mod('sales')();

export const VIEWS: ViewDef[] = [
  { id: 'home', label: 'nav.home', icon: 'home', min: 'cashier', load: () => import('./home') },
  { id: 'onboarding', label: 'nav.onboarding', icon: 'sliders', min: 'owner', load: () => import('./onboarding') },
  { id: 'farm', label: 'nav.farm', icon: 'home', min: 'cashier', load: () => import('./farm') },
  { id: 'batches', label: 'nav.livestock', icon: 'bird', min: 'worker', when: mod('livestock'), load: () => import('./batches') },
  { id: 'livestock', label: 'nav.livestock', icon: 'bird', min: 'cashier', when: mod('livestock'), load: () => import('./livestock') },
  { id: 'health', label: 'nav.health', icon: 'shield', min: 'cashier', when: mod('livestock'), load: () => import('./health') },
  { id: 'crops', label: 'nav.crops', icon: 'sun', min: 'cashier', when: mod('crops'), load: () => import('./crops') },
  { id: 'processing', label: 'nav.processing', icon: 'layers', min: 'cashier', when: mod('processing'), load: () => import('./processing') },
  { id: 'palm-oil', label: 'nav.palm', icon: 'mill', min: 'cashier', when: mod('processing') || mod('palm'), load: () => import('./palm-oil') },
  { id: 'feed', label: 'nav.feed', icon: 'chart', min: 'cashier', when: mod('livestock'), load: () => import('./feed') },
  { id: 'storage', label: 'nav.storage', icon: 'store', min: 'cashier', load: () => import('./storage') },
  { id: 'weather', label: 'nav.weather', icon: 'sun', min: 'cashier', load: () => import('./weather') },
  { id: 'farm-reports', label: 'nav.farm_reports', icon: 'chart', min: 'cashier', load: () => import('./farm-reports') },
  { id: 'audit', label: 'nav.audit', icon: 'shield', min: 'owner', load: () => import('./audit') },
  { id: 'advisory', label: 'nav.advisory', icon: 'help', min: 'cashier', load: () => import('./advisory') },
  { id: 'dashboard', label: 'nav.dashboard', icon: 'chart', min: 'cashier', when: sells, load: () => import('./dashboard') },
  { id: 'pos', label: 'nav.pos', icon: 'till', min: 'cashier', when: sellMod, load: () => import('./pos') },
  { id: 'receipts', label: 'nav.receipts', icon: 'receipt', min: 'cashier', when: sells, load: () => import('./receipts') },
  { id: 'customers', label: 'nav.customers', icon: 'people', min: 'cashier', when: sells, load: () => import('./customers') },
  { id: 'shifts', label: 'nav.shifts', icon: 'cash', min: 'cashier', when: sellMod, load: () => import('./shifts') },
  { id: 'inventory', label: 'nav.inventory', icon: 'box', min: 'manager', load: () => import('./inventory') },
  { id: 'purchases', label: 'nav.purchases', icon: 'truck', min: 'manager', load: () => import('./purchases') },
  { id: 'expenses', label: 'nav.expenses', icon: 'wallet', min: 'manager', load: () => import('./expenses') },
  { id: 'reports', label: 'nav.reports', icon: 'chart', min: 'manager', load: () => import('./reports') },
  { id: 'staff', label: 'nav.staff', icon: 'badge', min: 'owner', load: () => import('./staff') },
  { id: 'settings', label: 'nav.settings', icon: 'sliders', min: 'cashier', load: () => import('./settings') },
  { id: 'guide', label: 'nav.guide', icon: 'help', min: 'cashier', load: () => import('./guide') },
];

/** A tile on the home screen: a page, optionally opened on a given tab or section. */
export interface Tile { label: string; hint: string; icon: string; view: string; params?: Record<string, unknown>; min: Role; when?: () => boolean }
export interface Group { id: string; label: string; hint: string; icon: string; tone: string; tiles: Tile[] }

/** The old sidebar, regrouped. Tapping a group shows its tiles. */
export const GROUPS: Group[] = [
  { id: 'farm', label: 'group.farm', hint: 'group.farm.hint', icon: 'sun', tone: 'green', tiles: [
    { label: 'tile.my_farm', hint: 'tile.my_farm.hint', icon: 'home', view: 'farm', min: 'cashier' },
    { label: 'tile.livestock', hint: 'tile.livestock.hint', icon: 'bird', view: 'batches', min: 'worker', when: mod('livestock') },
    { label: 'tile.health_and_vaccination', hint: 'tile.health_and_vaccination.hint', icon: 'shield', view: 'health', min: 'cashier', when: mod('livestock') },
    { label: 'tile.crop_farming', hint: 'tile.crop_farming.hint', icon: 'sun', view: 'crops', min: 'cashier', when: mod('crops') },
    { label: 'tile.processing', hint: 'tile.processing.hint', icon: 'mill', view: 'palm-oil', min: 'cashier', when: mod('processing') || mod('palm') },
    { label: 'tile.feed', hint: 'tile.feed.hint', icon: 'chart', view: 'feed', min: 'cashier', when: mod('livestock') },
    { label: 'tile.storage', hint: 'tile.storage.hint', icon: 'store', view: 'storage', min: 'cashier' },
    { label: 'tile.weather_and_markets', hint: 'tile.weather_and_markets.hint', icon: 'sun', view: 'weather', min: 'cashier' },
  ] },
  { id: 'advice', label: 'group.advice', hint: 'group.advice.hint', icon: 'help', tone: 'teal', tiles: [
    { label: 'tile.suggestions', hint: 'tile.suggestions.hint', icon: 'info', view: 'advisory', params: { tab: 'suggestions' }, min: 'cashier' },
    { label: 'tile.ask_farmbuddy', hint: 'tile.ask_farmbuddy.hint', icon: 'chat', view: 'advisory', params: { tab: 'ask' }, min: 'cashier' },
    { label: 'tile.reminders', hint: 'tile.reminders.hint', icon: 'calendar', view: 'advisory', params: { tab: 'reminders' }, min: 'cashier' },
    { label: 'tile.knowledge_library', hint: 'tile.knowledge_library.hint', icon: 'help', view: 'advisory', params: { tab: 'library' }, min: 'cashier' },
  ] },
  { id: 'sell', label: 'group.sell', hint: 'group.sell.hint', icon: 'till', tone: 'orange', tiles: [
    { label: 'tile.new_sale', hint: 'tile.new_sale.hint', icon: 'till', view: 'pos', min: 'cashier', when: sellMod },
    { label: 'tile.receipts', hint: 'tile.receipts.hint', icon: 'receipt', view: 'receipts', min: 'cashier', when: sells },
    { label: 'tile.buyers_and_customers', hint: 'tile.buyers_and_customers.hint', icon: 'people', view: 'customers', min: 'cashier', when: sells },
    { label: 'tile.shifts_and_cash_up', hint: 'tile.shifts_and_cash_up.hint', icon: 'cash', view: 'shifts', min: 'cashier', when: sellMod },
  ] },
  { id: 'stock', label: 'group.stock', hint: 'group.stock.hint', icon: 'box', tone: 'blue', tiles: [
    { label: 'tile.inventory', hint: 'tile.inventory.hint', icon: 'box', view: 'inventory', params: { tab: 'items' }, min: 'manager' },
    { label: 'tile.receive_goods', hint: 'tile.receive_goods.hint', icon: 'truck', view: 'purchases', min: 'manager' },
    { label: 'tile.expiry_dates', hint: 'tile.expiry_dates.hint', icon: 'calendar', view: 'inventory', params: { tab: 'expiry' }, min: 'manager' },
    { label: 'tile.stock_history', hint: 'tile.stock_history.hint', icon: 'stock', view: 'inventory', params: { tab: 'history' }, min: 'manager' },
    { label: 'tile.categories', hint: 'tile.categories.hint', icon: 'tag', view: 'inventory', params: { tab: 'categories' }, min: 'manager' },
  ] },
  { id: 'money', label: 'group.money', hint: 'group.money.hint', icon: 'chart', tone: 'brown', tiles: [
    { label: 'tile.overview', hint: 'tile.overview.hint', icon: 'home', view: 'dashboard', min: 'cashier', when: sells },
    { label: 'tile.farm_reports', hint: 'tile.farm_reports.hint', icon: 'chart', view: 'farm-reports', min: 'cashier' },
    { label: 'tile.finance_summary', hint: 'tile.finance_summary.hint', icon: 'wallet', view: 'farm-reports', params: { tab: 'finance' }, min: 'owner' },
    { label: 'tile.sales_reports', hint: 'tile.sales_reports.hint', icon: 'chart', view: 'reports', min: 'manager' },
    { label: 'tile.expenses', hint: 'tile.expenses.hint', icon: 'wallet', view: 'expenses', min: 'manager' },
  ] },
  { id: 'business', label: 'group.business', hint: 'group.business.hint', icon: 'sliders', tone: 'purple', tiles: [
    { label: 'tile.settings', hint: 'tile.settings.hint', icon: 'sliders', view: 'settings', min: 'owner' },
    { label: 'tile.staff_and_users', hint: 'tile.staff_and_users.hint', icon: 'badge', view: 'staff', min: 'owner' },
    { label: 'tile.activity_log', hint: 'tile.activity_log.hint', icon: 'shield', view: 'audit', min: 'owner' },
    { label: 'tile.licence', hint: 'tile.licence.hint', icon: 'key', view: 'settings', params: { section: 'licence' }, min: 'owner' },
    { label: 'tile.backup', hint: 'tile.backup.hint', icon: 'download', view: 'settings', params: { section: 'backup' }, min: 'owner' },
    { label: 'tile.my_account', hint: 'tile.my_account.hint', icon: 'user', view: 'settings', params: { section: 'account' }, min: 'cashier' },
  ] },
  { id: 'help', label: 'group.help', hint: 'group.help.hint', icon: 'help', tone: 'teal', tiles: [
    { label: 'tile.how_to', hint: 'tile.how_to.hint', icon: 'help', view: 'guide', min: 'cashier' },
    { label: 'tile.about_and_contact', hint: 'tile.about_and_contact.hint', icon: 'info', view: 'settings', params: { section: 'about' }, min: 'cashier' },
    { label: 'tile.updates', hint: 'tile.updates.hint', icon: 'refresh', view: 'settings', params: { section: 'updates' }, min: 'owner' },
  ] },
];


const allowed = (t: { min: Role; when?: () => boolean }) => auth.can(t.min) && (!t.when || t.when());
export const visibleGroups = () => GROUPS.map((g) => ({ ...g, tiles: g.tiles.filter(allowed) })).filter((g) => g.tiles.length);

const tt = (k: string) => t(k);
const tileHtml = (t: Tile) => `<button type="button" class="subtile" data-tile-view="${t.view}" data-tile-params="${esc(JSON.stringify(t.params ?? {}))}">
  <span class="subtile-icon">${icon(t.icon, 22)}</span><span class="subtile-text"><b>${esc(tt(t.label))}</b><small>${esc(tt(t.hint))}</small></span>${icon('chevron', 16)}</button>`;

/** The tiles of one group (or of every group) in a sheet that slides up. */
export function openGroupSheet(go: ViewCtx['go'], groupId?: string) {
  // Phones: the full menu is the group tiles; each opens its own sheet, so nothing scrolls.
  if (!groupId && matchMedia('(max-width: 760px)').matches) {
    const all = visibleGroups();
    const gm = modal({ title: 'Menu', size: 'md', body: `<div class="group-grid sheet-grid">${all.map((g) => `
      <button type="button" class="group-tile tone-${g.tone}" data-sheet-group="${g.id}">
        <span class="group-icon">${icon(g.icon, 24)}</span>
        <span class="group-text"><b>${esc(t(g.label))}</b><small>${esc(t(g.hint))}</small></span>
        <span class="group-count">${g.tiles.length}</span>
      </button>`).join('')}</div>` });
    gm.el.closest('.modal-wrap')?.classList.add('is-sheet');
    gm.el.addEventListener('click', (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>('[data-sheet-group]');
      if (!b) return;
      gm.close();
      openGroupSheet(go, b.dataset.sheetGroup);
    });
    return;
  }
  const groups = visibleGroups().filter((g) => !groupId || g.id === groupId);
  const m = modal({ title: groupId ? t(groups[0]?.label ?? 'Menu') : 'Everything', size: 'md', body: `<div class="sheet-groups">${groups.map((g) => `
    ${groupId ? '' : `<h3 class="sheet-group-title">${esc(t(g.label))}</h3>`}
    <div class="subtile-grid">${g.tiles.map(tileHtml).join('')}</div>`).join('')}</div>` });
  m.el.closest('.modal-wrap')?.classList.add('is-sheet');
  m.el.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-tile-view]');
    if (!b) return;
    m.close();
    go(b.dataset.tileView!, JSON.parse(b.dataset.tileParams || '{}'));
  });
}

const roleName: Record<Role, string> = { owner: 'Owner', manager: 'Manager', cashier: 'Cashier', worker: 'Farm worker', adviser: 'Adviser' };

export function mountShell(root: HTMLElement, onSignOut: () => void, startView = 'home') {
  const s = auth.session!;
  const initials = esc((s.full_name || s.username).trim().split(/\s+/).map((w) => w[0]).slice(0, 2).join('').toUpperCase());
  const groups = visibleGroups();
  const navItem = (tile: Tile) => `<button type="button" class="side-link" data-nav-view="${tile.view}" data-nav-params="${esc(JSON.stringify(tile.params ?? {}))}">${icon(tile.icon, 18)}<span>${esc(t(tile.label))}</span></button>`;
  root.innerHTML = `<div class="app2 fb-shell">
    <aside class="side" aria-label="Main menu">
      <button type="button" class="side-brand" data-go-home aria-label="Today"></button>
      <button type="button" class="side-farm farm-switch" data-farm-switch hidden aria-label="Choose farm"></button>
      <nav class="side-nav">
        <button type="button" class="side-link" data-nav-view="home" data-nav-params="{}">${icon('home', 18)}<span>${esc(t('nav.home'))}</span></button>
        ${groups.map((g) => `<div class="side-group"><p class="side-label">${esc(t(g.label))}</p>${g.tiles.filter((x) => !(x.view === 'settings' && x.params?.section === 'account')).map(navItem).join('')}</div>`).join('')}
      </nav>
      <div class="side-foot"><span class="net-pill" data-net role="status" aria-live="polite"></span><span class="side-version">FarmBuddy</span></div>
    </aside>
    <div class="main-col">
      <header class="appbar topbar">
        <button type="button" class="appbar-back icon-btn" data-back aria-label="Back">${icon('back', 22)}</button>
        <button type="button" class="appbar-brand topbar-brand" data-go-home aria-label="Today"></button>
        <div class="appbar-title"><h1 data-title></h1><p data-subtitle></p></div>
        <div class="page-actions" data-actions></div>
        ${auth.canSell() ? `<button type="button" class="topbar-sell btn btn-primary" data-quick="pos">${icon('till', 17)}<span>${esc(t('nav.pos'))}</span><kbd>F2</kbd></button>` : ''}
        <button type="button" class="appbar-avatar" data-account aria-label="Account">${initials}</button>
      </header>
      <div class="app-banner" data-banner hidden></div>
      <main class="view" id="view" tabindex="-1"></main>
    </div>
    <nav class="tabbar" aria-label="Main">
      <button type="button" data-quick="home">${icon('home', 22)}<span>${esc(t('nav.home'))}</span></button>
      ${auth.canSell() ? `<button type="button" data-quick="pos">${icon('till', 22)}<span>${esc(t('group.sell'))}</span></button>` : `<button type="button" data-quick="batches">${icon('bird', 22)}<span>Records</span></button>`}
      <button type="button" data-quick="farm">${icon('sun', 22)}<span>${esc(t('group.farm'))}</span></button>
      <button type="button" data-quick="advisory">${icon('help', 22)}<span>${esc(t('group.advice'))}</span></button>
      <button type="button" data-quick="menu">${icon('grid', 22)}<span>Menu</span></button>
    </nav>
  </div>`;
  const app = $('.app2', root)!;

  let current = '';
  let main = $<HTMLElement>('#view', root)!;
  // Pages visited, so Back (and the phone's back button) return to the previous page.
  const trail: Array<{ id: string; params: Record<string, unknown> }> = [];

  const refreshChrome = () => {
    const name = store.settings.business_name || 'My Farm';
    const logo = store.settings.business_logo;
    root.querySelectorAll<HTMLElement>('[data-go-home]').forEach((el) => {
      el.innerHTML = `<img src="${esc(logo || '/logo-mark.svg')}" alt=""><span><b>${esc(el.classList.contains('side-brand') ? 'FarmBuddy' : name)}</b><small>${esc(el.classList.contains('side-brand') ? name : 'FarmBuddy')}</small></span>`;
    });
    app.classList.toggle('is-home', current === 'home');
    app.dataset.view = current;
    root.querySelectorAll<HTMLElement>('[data-quick]').forEach((b) => b.classList.toggle('is-on', b.dataset.quick === current));
    const last = trail[trail.length - 1];
    const cur = JSON.stringify(last?.params ?? {});
    const links = [...root.querySelectorAll<HTMLElement>('[data-nav-view]')];
    const sameView = links.filter((b) => b.dataset.navView === current);
    const exact = sameView.find((b) => b.dataset.navParams === cur) ?? sameView.find((b) => b.dataset.navParams === '{}') ?? sameView[0];
    links.forEach((b) => b.classList.toggle('is-on', b === exact));
  };

  const header = (title: string, subtitle = '', actions = '') => {
    $('[data-title]', root)!.textContent = title;
    $('[data-subtitle]', root)!.textContent = subtitle;
    const a = $('[data-actions]', root)!;
    a.innerHTML = actions;
    a.hidden = !actions.trim();
  };

  const render = async (id: string, params: Record<string, unknown>) => {
    const def = VIEWS.find((v) => v.id === id && auth.can(v.min) && (!v.when || v.when())) ?? VIEWS[0];
    current = def.id;
    $<HTMLElement>('[data-actions]', root)!.onclick = null;
    header(t(def.label));
    refreshChrome();
    // A fresh element per page, so listeners from the previous page cannot leak.
    const fresh = document.createElement('main');
    fresh.id = 'view';
    fresh.tabIndex = -1;
    main.replaceWith(fresh);
    main = fresh;
    const page = fresh;
    page.className = `view view-${def.id}`;
    page.innerHTML = '<div class="loading" aria-label="Loading"><span></span><span></span><span></span></div>';
    try {
      const mod = await def.load();
      if (main !== page) return;
      await mod.render({ main: page, params, header, go, refreshChrome });
      if (!document.body.classList.contains('has-modal') && !page.contains(document.activeElement)) page.focus({ preventScroll: true });
    } catch (e) {
      if (main !== page) return;
      const msg = e instanceof Error ? `${e.message}` : String(e);
      page.innerHTML = `<div class="empty"><h3>This page could not open</h3><p>${esc(msg)}</p>
        <div class="row-actions"><button class="btn btn-primary" data-retry>Try again</button><button class="btn btn-ghost" data-home2>Go home</button></div></div>`;
      $('[data-retry]', page)?.addEventListener('click', () => render(def.id, params));
      $('[data-home2]', page)?.addEventListener('click', () => go('home'));
    }
  };

  function go(id: string, params: Record<string, unknown> = {}) {
    if (id === 'home') trail.length = 0;
    trail.push({ id, params });
    try { history.pushState({ farmbuddy: trail.length }, ''); } catch { /* not available */ }
    void render(id, params);
  }

  const back = () => {
    if (trail.length <= 1) { if (current !== 'home') go('home'); return; }
    trail.pop();
    const prev = trail[trail.length - 1];
    void render(prev.id, prev.params);
  };

  // Phone back button: close an open pop-up first, otherwise go to the previous page.
  const onPop = () => {
    const open = document.querySelectorAll('.modal-wrap');
    if (open.length) {
      document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      try { history.pushState({ farmbuddy: trail.length }, ''); } catch { /* ignore */ }
      return;
    }
    if (current === 'home' && trail.length <= 1) return; // the next Back leaves the app
    back();
    try { history.pushState({ farmbuddy: trail.length }, ''); } catch { /* ignore */ }
  };
  window.addEventListener('popstate', onPop);

  const accountSheet = () => {
    const m = modal({ title: s.full_name || s.username, subtitle: `${roleName[s.role]} · ${store.settings.business_name || ''}`, size: 'sm', body: `<div class="subtile-grid one">
      <button type="button" class="subtile" data-acc="account"><span class="subtile-icon">${icon('user', 22)}</span><span class="subtile-text"><b>My account</b><small>Change your password</small></span>${icon('chevron', 16)}</button>
      <button type="button" class="subtile" data-acc="about"><span class="subtile-icon">${icon('info', 22)}</span><span class="subtile-text"><b>About & contact</b><small>Version and TechAlpha Hub</small></span>${icon('chevron', 16)}</button>
      <button type="button" class="subtile danger" data-acc="signout"><span class="subtile-icon">${icon('logout', 22)}</span><span class="subtile-text"><b>Sign out</b><small>Let someone else sign in</small></span></button>
    </div>` });
    m.el.closest('.modal-wrap')?.classList.add('is-sheet');
    m.el.addEventListener('click', (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>('[data-acc]');
      if (!b) return;
      m.close();
      if (b.dataset.acc === 'signout') call('logout').catch(() => undefined).finally(onSignOut);
      else go('settings', { section: b.dataset.acc });
    });
  };

  root.addEventListener('click', (e) => {
    const t = e.target as HTMLElement;
    if (t.closest('[data-back]')) { back(); return; }
    if (t.closest('[data-go-home]')) { go('home'); return; }
    if (t.closest('[data-account]')) { accountSheet(); return; }
    const nl = t.closest<HTMLElement>('[data-nav-view]');
    if (nl) { go(nl.dataset.navView!, JSON.parse(nl.dataset.navParams || '{}')); return; }
    const q = t.closest<HTMLElement>('[data-quick]');
    if (q) { if (q.dataset.quick === 'menu') openGroupSheet(go); else go(q.dataset.quick!); return; }
    const b = t.closest<HTMLElement>('[data-banner-go]');
    if (b) go(b.dataset.bannerGo!, { section: b.dataset.section });
  });
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'F2' && auth.canSell() && !document.body.classList.contains('has-modal')) { e.preventDefault(); go('pos'); }
  };
  document.addEventListener('keydown', onKey);

  // Online / offline: FarmBuddy works fully offline; only email, updates and the usage notice need the internet.
  const net = $('[data-net]', root)!;
  const showNet = () => {
    const on = navigator.onLine;
    net.className = `net-pill ${on ? 'is-online' : 'is-offline'}`;
    net.innerHTML = `<span class="net-dot"></span><span class="net-text">${on ? 'Online' : 'Offline'}</span>`;
    net.title = on ? 'Connected. Email and updates can go through.' : 'No internet. Everything still works and is saved on this device; email and updates wait until you are back online.';
  };
  showNet();
  window.addEventListener('online', showNet);
  window.addEventListener('offline', showNet);

  // Farm switcher: shown when the user can see more than one farm. Lists then show that farm only.
  const farmBtn = $<HTMLButtonElement>('[data-farm-switch]', root)!;
  let myFarms: Array<{ id: number; name: string }> = [];
  const drawFarm = () => {
    const cur = myFarms.find((f) => f.id === store.farmId);
    if (store.farmId && !cur) store.setFarm(null);
    farmBtn.hidden = myFarms.length < 2;
    farmBtn.innerHTML = `${icon('home', 15)}<span><small>Farm</small><b>${esc(cur?.name ?? 'All farms')}</b></span>${icon('chevron', 14)}`;
  };
  void call<Array<{ id: number; name: string }>>('my_farms').then((f) => { myFarms = f; drawFarm(); }).catch(() => undefined);
  farmBtn.addEventListener('click', () => {
    const m = modal({ title: 'Choose farm', size: 'sm', body: `<div class="subtile-grid one">${[{ id: 0, name: 'All farms' }, ...myFarms].map((f) => `<button type="button" class="subtile ${(store.farmId ?? 0) === f.id ? 'is-on' : ''}" data-pick-farm="${f.id}"><span class="subtile-icon">${icon('home', 20)}</span><span class="subtile-text"><b>${esc(f.name)}</b></span></button>`).join('')}</div>` });
    m.el.addEventListener('click', (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>('[data-pick-farm]');
      if (!b) return;
      store.setFarm(Number(b.dataset.pickFarm) || null);
      m.close(); drawFarm();
      const last = trail[trail.length - 1];
      void render(last?.id ?? 'home', last?.params ?? {});
    });
  });

  // Thin strip above the page for the trial countdown and waiting updates.
  const banners = new Map<string, string>();
  const setBanner = (key: string, html: string | null) => {
    if (html) banners.set(key, html); else banners.delete(key);
    const el = $('[data-banner]', root)!;
    el.hidden = !banners.size;
    el.innerHTML = [...banners.values()].map((b) => `<div class="banner-row">${b}</div>`).join('');
  };

  go(startView);
  return { go, setBanner, dispose: () => { document.removeEventListener('keydown', onKey); window.removeEventListener('popstate', onPop); window.removeEventListener('online', showNet); window.removeEventListener('offline', showNet); } };
}
