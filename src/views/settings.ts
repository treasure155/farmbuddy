import { t } from '../lib/i18n';
import { auth, call } from '../api';
import { $, $$, confirmDialog, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { icon } from '../lib/icons';
import { printHtml, receiptMarkup } from '../lib/receipt';
import { applyTheme, contrast, PRESETS } from '../lib/theme';
import { store } from '../store';
import type { BackupStatus, PaymentMethod, Receipt } from '../types';
import { activationDialog, licence, vendorBlock, wireVendor } from '../lib/license-ui';
import { appVersion } from '../lib/updater';
import { checkNow } from '../lib/app-update';
import { isMobile, openExternal } from '../lib/platform';
import { VENDOR } from '../lib/vendor';
import { shareWhatsApp } from '../lib/whatsapp';
import { dateLabel, relativeDay } from '../lib/format';
import type { ViewCtx } from './shell';
import { questionFields, readQuestions, wireQuestionFields } from './recovery-ui';

type Section = 'business' | 'receipts' | 'selling' | 'appearance' | 'email' | 'payments' | 'security' | 'backup' | 'reset' | 'licence' | 'updates' | 'account' | 'about';
const SECTIONS: Array<{ key: Section; label: string; icon: string; owner: boolean }> = [
  { key: 'business', label: 'Farm profile', icon: 'home', owner: true },
  { key: 'receipts', label: 'Receipts & tax', icon: 'receipt', owner: true },
  { key: 'selling', label: 'Selling rules & wording', icon: 'tag', owner: true },
  { key: 'appearance', label: 'Colours & theme', icon: 'palette', owner: true },
  { key: 'email', label: 'Email', icon: 'mail', owner: true },
  { key: 'payments', label: 'Payment methods', icon: 'cash', owner: true },
  { key: 'security', label: 'Security & recovery', icon: 'shield', owner: true },
  { key: 'backup', label: 'Backup', icon: 'download', owner: true },
  { key: 'reset', label: 'Start afresh', icon: 'trash', owner: true },
  { key: 'licence', label: 'Licence', icon: 'key', owner: true },
  { key: 'updates', label: 'Updates', icon: 'refresh', owner: true },
  { key: 'account', label: 'My account', icon: 'user', owner: false },
  { key: 'about', label: 'About & contact', icon: 'info', owner: false },
];

const PROVIDERS: Record<string, { name: string; host: string; port: string; security: string; help: string }> = {
  gmail: { name: 'Gmail / Google Workspace', host: 'smtp.gmail.com', port: '587', security: 'starttls',
    help: `<ol><li>Turn on <b>2-Step Verification</b> for the Google account.</li><li>Open <b>myaccount.google.com/apppasswords</b>, create an app password named “FarmBuddy”, and copy the 16 letters.</li><li>Paste it below as the password. Your normal Gmail password will not work.</li></ol><p>Send from the same Gmail address, or an alias added under Gmail → Settings → Accounts → “Send mail as”. Gmail allows about 500 emails a day.</p>` },
  outlook: { name: 'Outlook / Microsoft 365', host: 'smtp.office365.com', port: '587', security: 'starttls',
    help: '<p>Use your full email address and password. For Microsoft 365 business mailboxes, your admin must allow <b>Authenticated SMTP</b> for this mailbox. If you use two-step sign-in, create an app password.</p><p>Outlook.com personal accounts use <b>smtp-mail.outlook.com</b>.</p>' },
  yahoo: { name: 'Yahoo Mail', host: 'smtp.mail.yahoo.com', port: '465', security: 'ssl',
    help: '<p>Go to Yahoo Account Security → <b>Generate app password</b>, and use that password here.</p>' },
  zoho: { name: 'Zoho Mail', host: 'smtp.zoho.com', port: '465', security: 'ssl',
    help: '<p>Use <b>smtp.zoho.eu</b> or <b>smtp.zoho.in</b> if your account is hosted there. With two-factor sign-in, create an application-specific password.</p>' },
  custom: { name: 'Other (your host or domain email)', host: '', port: '587', security: 'starttls',
    help: '<p>Get the SMTP server, port and security type from your web host (cPanel → Email Accounts → Connect Devices). Port 587 normally uses STARTTLS, and port 465 uses SSL/TLS.</p>' },
};

let section: Section = 'business';

export async function render(ctx: ViewCtx) {
  const owner = auth.can('owner');
  const visible = SECTIONS.filter((s) => owner || !s.owner);
  const asked = ctx.params.section as Section | undefined;
  if (asked && visible.some((v) => v.key === asked)) section = asked;
  else if (!visible.some((v) => v.key === section)) section = visible[0].key;
  await store.loadSettings();
  // Phones: a list of sections as tiles; each opens on its own page (Back returns to the list).
  if (window.matchMedia('(max-width: 760px)').matches) {
    if (!asked) {
      ctx.header(t('page.settings.title', {}, 'Settings'), owner ? 'Make the app fit your business.' : 'Your account.');
      ctx.main.innerHTML = `<div class="subtile-grid one">${visible.map((v) => `<button type="button" class="subtile" data-open-sec="${v.key}"><span class="subtile-icon">${icon(v.icon, 22)}</span><span class="subtile-text"><b>${esc(v.label)}</b></span>${icon('chevron', 16)}</button>`).join('')}</div>`;
      ctx.main.onclick = (e) => { const b = (e.target as HTMLElement).closest<HTMLElement>('[data-open-sec]'); if (b) ctx.go('settings', { section: b.dataset.openSec }); };
      return;
    }
    ctx.header(visible.find((v) => v.key === section)?.label ?? 'Settings');
    applyTheme(store.settings.theme_primary, store.settings.theme_secondary, store.settings.theme_mode);
    ctx.main.innerHTML = '<div class="settings-body solo" data-body></div>';
    await SECTION_FNS[section]($('[data-body]', ctx.main)!, ctx);
    return;
  }
  ctx.header('Settings', owner ? 'Make the app fit your business.' : 'Your account and the app.');
  ctx.main.innerHTML = `<div class="settings">
    <nav class="settings-nav" aria-label="Settings sections">${visible.map((s) => `<button data-sec="${s.key}" class="${s.key === section ? 'is-on' : ''}">${icon(s.icon, 17)}<span>${s.label}</span></button>`).join('')}</nav>
    <div class="settings-body" data-body></div></div>`;
  const body = $('[data-body]', ctx.main)!;
  const show = async () => {
    $$('[data-sec]', ctx.main).forEach((b) => b.classList.toggle('is-on', b.dataset.sec === section));
    // Leaving the colour section without saving restores the saved theme.
    applyTheme(store.settings.theme_primary, store.settings.theme_secondary, store.settings.theme_mode);
    body.innerHTML = '';
    await SECTION_FNS[section](body, ctx);
  };
  ctx.main.addEventListener('click', (e) => {
    const b = (e.target as HTMLElement).closest<HTMLElement>('[data-sec]');
    if (b) { section = b.dataset.sec as Section; show().catch((x) => toast(errText(x), 'error')); }
  });
  await show();
}

const SECTION_FNS = { business, receipts, selling, appearance, email, payments, security, backup, reset: startAfresh, licence: licenceSection, updates, account, about } as Record<Section, (b: HTMLElement, c: ViewCtx) => Promise<void> | void>;

async function save(values: Record<string, string>, btn: HTMLButtonElement | null, after?: () => void) {
  await withBusy(btn, async () => {
    await call('save_settings', { values });
    await store.loadSettings();
    toast('Settings saved');
    after?.();
  });
}

const head = (title: string, text: string) => `<header class="settings-head"><h2>${esc(title)}</h2><p class="muted">${text}</p></header>`;
const check = (name: string, title: string, text: string) => `<label class="check"><input type="checkbox" name="${name}" ${store.flag(name) ? 'checked' : ''}><span><b>${esc(title)}</b><br><small class="muted">${esc(text)}</small></span></label>`;
const val = (k: string) => esc(store.settings[k] ?? '');

// ---------------------------------------------------------------- business

function business(body: HTMLElement, ctx: ViewCtx) {
  let logo = store.settings.business_logo ?? '';
  body.innerHTML = `${head('Business profile', 'Shown on receipts, emails and the sign-in screen.')}
    <form class="stack form-narrow" novalidate>
      <label class="field"><span>Business name</span><input name="business_name" required maxlength="120" value="${val('business_name')}"></label>
      <label class="field"><span>Address</span><textarea name="business_address" rows="2" maxlength="300">${val('business_address')}</textarea></label>
      <div class="grid-2">
        <label class="field"><span>Phone</span><input name="business_phone" maxlength="60" value="${val('business_phone')}"></label>
        <label class="field"><span>Email</span><input name="business_email" type="email" maxlength="254" value="${val('business_email')}"></label>
      </div>
      <div class="grid-2">
        <label class="field"><span>Currency symbol</span><input name="currency_symbol" maxlength="4" value="${val('currency_symbol') || '₦'}"><small>Use ₦ for naira, or GH₵, KSh, $, £ and so on.</small></label>
      </div>
      <div class="field"><span>Logo</span>
        <div class="logo-row"><div class="logo-preview" data-logo>${logo ? `<img src="${esc(logo)}" alt="Current logo">` : '<span class="muted small">No logo</span>'}</div>
          <div class="stack-sm"><label class="btn btn-ghost btn-sm file-btn">${icon('download', 15)} Choose image<input type="file" accept="image/png,image/jpeg,image/webp" data-file hidden></label>
          <button type="button" class="link-btn" data-clear-logo ${logo ? '' : 'hidden'}>Remove logo</button>
          <small class="muted">PNG, JPG or WebP. Square images work best.</small></div></div></div>
      <div class="form-actions start"><button class="btn btn-primary">Save business profile</button></div>
    </form>`;
  const form = $<HTMLFormElement>('form', body)!;
  const preview = $('[data-logo]', body)!;
  $<HTMLInputElement>('[data-file]', body)!.addEventListener('change', async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    try {
      logo = await shrinkImage(file, 320);
      preview.innerHTML = `<img src="${esc(logo)}" alt="New logo">`;
      $('[data-clear-logo]', body)!.hidden = false;
    } catch (x) { toast(errText(x), 'error'); }
  });
  $('[data-clear-logo]', body)!.addEventListener('click', () => { logo = ''; preview.innerHTML = '<span class="muted small">No logo</span>'; $('[data-clear-logo]', body)!.hidden = true; });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    save({ business_name: v.business_name, business_address: v.business_address, business_phone: v.business_phone, business_email: v.business_email, currency_symbol: v.currency_symbol, business_logo: logo }, $('.btn-primary', form) as HTMLButtonElement, ctx.refreshChrome);
  });
}

/** Downscales an uploaded logo to a small PNG data URL so it stays light in the database and emails. */
function shrinkImage(file: File, max: number): Promise<string> {
  return new Promise((resolve, reject) => {
    if (!/^image\/(png|jpeg|webp)$/.test(file.type)) return reject(new Error('Choose a PNG, JPG or WebP image'));
    if (file.size > 8 * 1024 * 1024) return reject(new Error('That image is larger than 8 MB'));
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      const scale = Math.min(1, max / Math.max(img.width, img.height));
      const c = document.createElement('canvas');
      c.width = Math.round(img.width * scale); c.height = Math.round(img.height * scale);
      c.getContext('2d')!.drawImage(img, 0, 0, c.width, c.height);
      URL.revokeObjectURL(url);
      const out = c.toDataURL('image/png');
      out.length > 680_000 ? reject(new Error('Logo is still too large after resizing')) : resolve(out);
    };
    img.onerror = () => { URL.revokeObjectURL(url); reject(new Error('That file could not be read as an image')); };
    img.src = url;
  });
}

// ---------------------------------------------------------------- receipts

function sampleReceipt(): Receipt {
  const items = (store.products.filter((p) => p.kind !== 'material').slice(0, 3));
  const lines = items.length ? items.map((p, i) => ({ id: i + 1, returned_qty: 0, sold_as: '', product_name: p.name, product_kind: p.kind, unit: p.unit, quantity: i + 1, unit_price_kobo: p.selling_price_kobo, line_total_kobo: p.selling_price_kobo * (i + 1) }))
    : [{ id: 1, returned_qty: 0, sold_as: '', product_name: 'Sample item', product_kind: 'product' as const, unit: 'piece', quantity: 2, unit_price_kobo: 250000, line_total_kobo: 500000 }];
  const subtotal = lines.reduce((a, l) => a + l.line_total_kobo, 0);
  const rate = store.flag('tax_enabled') ? Number(store.settings.tax_rate || 0) : 0;
  const tax = Math.round((subtotal * rate) / 100);
  return { id: 0, receipt_no: `${store.settings.receipt_prefix || 'RCP'}-2610-00042`, customer_id: null, customer_name: 'Mrs Ekaette Bassey', customer_phone: '', customer_email: '', cashier_name: auth.session!.full_name,
    subtotal_kobo: subtotal, discount_kobo: 0, tax_kobo: tax, total_kobo: subtotal + tax, amount_paid_kobo: subtotal + tax, change_kobo: 0, balance_kobo: 0, payment_status: 'paid', fulfilment_status: 'none',
    collection_date: '', note: '', void_reason: '', voided_by: '', created_at: new Date().toISOString().slice(0, 19).replace('T', ' '), items: lines, returned_kobo: 0, refunded_kobo: 0, returns: [],
    payments: [{ method: 'Bank transfer', amount_kobo: subtotal + tax, reference: '', received_by: '', created_at: '' }] };
}

async function receipts(body: HTMLElement) {
  if (!store.products.length) await store.loadProducts().catch(() => undefined);
  body.innerHTML = `${head('Receipts & tax', 'What prints at the top and bottom of every receipt.')}
    <div class="split-preview"><form class="stack" novalidate>
      <div class="grid-2">
        <label class="field"><span>Receipt title</span><input name="receipt_title" maxlength="60" value="${val('receipt_title')}"><small>e.g. Sales receipt, Laundry ticket, Invoice.</small></label>
        <label class="field"><span>Number prefix</span><input name="receipt_prefix" maxlength="8" value="${val('receipt_prefix')}" spellcheck="false"><small>Letters or numbers, up to 8.</small></label>
      </div>
      <label class="field"><span>Message at the bottom</span><textarea name="receipt_footer" rows="3" maxlength="300">${val('receipt_footer')}</textarea></label>
      <div class="field"><span>Paper size</span><div class="seg" role="radiogroup">
        ${[['58', '58 mm roll'], ['80', '80 mm roll'], ['a4', 'A4 sheet']].map(([v, l]) => `<label><input type="radio" name="receipt_width_mm" value="${v}" ${store.settings.receipt_width_mm === v ? 'checked' : ''}><span>${l}</span></label>`).join('')}
      </div><small class="muted">Pick your thermal printer and the same paper size in the print dialog.</small></div>
      <fieldset class="subtle stack">
        ${check('tax_enabled', 'Charge tax on sales', 'Added after any discount and shown as its own line on the receipt.')}
        <div class="grid-2"><label class="field"><span>Tax name</span><input name="tax_label" maxlength="20" value="${val('tax_label')}"></label>
        <label class="field"><span>Rate (%)</span><input name="tax_rate" inputmode="decimal" value="${val('tax_rate')}"><small>Nigerian VAT is 7.5%.</small></label></div>
      </fieldset>
      <div class="form-actions start"><button class="btn btn-primary">Save receipt settings</button></div>
    </form><aside class="preview-pane"><span class="field-label">Preview</span><div class="paper-wrap" data-preview></div></aside></div>`;
  const form = $<HTMLFormElement>('form', body)!;
  const preview = $('[data-preview]', body)!;
  const draw = () => {
    const v = formValues(form);
    const saved = { ...store.settings };
    Object.assign(store.settings, { receipt_title: v.receipt_title, receipt_prefix: v.receipt_prefix, receipt_footer: v.receipt_footer, tax_enabled: v.tax_enabled, tax_label: v.tax_label, tax_rate: v.tax_rate });
    preview.innerHTML = receiptMarkup(sampleReceipt(), { width: v.receipt_width_mm === 'a4' ? '80' : v.receipt_width_mm });
    store.settings = saved;
  };
  draw();
  form.addEventListener('input', draw);
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    save({ receipt_title: v.receipt_title, receipt_prefix: v.receipt_prefix, receipt_footer: v.receipt_footer, receipt_width_mm: v.receipt_width_mm, tax_enabled: v.tax_enabled, tax_label: v.tax_label, tax_rate: v.tax_rate }, $('.btn-primary', form) as HTMLButtonElement);
  });
}

// ---------------------------------------------------------------- selling rules & wording

function selling(body: HTMLElement, ctx: ViewCtx) {
  body.innerHTML = `${head('Selling rules & wording', 'Adjust how selling works and what things are called, so the app speaks your farm’s language.')}
    <form class="stack form-narrow">
      <fieldset class="subtle stack"><legend>When selling</legend>
        ${check('allow_price_override', 'Let cashiers change prices on a sale', 'Useful for farm produce, bulk buyers and negotiated prices. Managers can always change prices.')}
        ${check('allow_negative_stock', 'Allow selling when stock shows zero', 'Keep this off unless you sell before recording deliveries. Counts can go below zero.')}
        ${check('track_collection', 'Track orders for pickup', 'Each receipt gets In progress, Ready and Collected stages with a collection date. Ideal for dry cleaning, tailoring and repairs.')}
        ${check('require_shift', 'Cashiers must open a shift before selling', 'They count the float at the start and the cash at the end. Any shortage shows under Shifts & cash-up.')}
      </fieldset>
      <fieldset class="subtle stack"><legend>Extra features</legend>
        ${check('feature_recipes', 'Recipes for meals and services', 'Selling a meal takes its ingredients (rice, oil, chicken…) out of kitchen stock and works out its true cost.')}
        ${check('feature_flocks', 'Livestock batch records', 'Keeps livestock batch tracking available. Switch whole pages on or off in Farm → My Farm.')}
        <div class="grid-2">
          <label class="field"><span>Warn about expiry this many days ahead</span><input name="expiry_warn_days" type="number" min="1" max="365" value="${val('expiry_warn_days') || '30'}"></label>
          <label class="field"><span>WhatsApp country code</span><span class="input-affix"><span class="affix">+</span><input name="whatsapp_country_code" inputmode="numeric" maxlength="4" value="${val('whatsapp_country_code') || '234'}"></span><small>Turns 0803… into +234 803… when sending receipts.</small></label>
        </div>
      </fieldset>
      <fieldset class="subtle stack"><legend>Wording</legend>
        <div class="grid-3">
          <label class="field"><span>One item is called</span><input name="label_item" maxlength="30" value="${val('label_item')}" placeholder="Item"></label>
          <label class="field"><span>Many items are called</span><input name="label_items" maxlength="30" value="${val('label_items')}" placeholder="Items"></label>
          <label class="field"><span>A buyer is called</span><input name="label_customer" maxlength="30" value="${val('label_customer')}" placeholder="Customer"></label>
        </div>
        <small class="muted">For example: Item / Items / Buyer, or Produce / Produce / Customer.</small>
      </fieldset>
      <div class="form-actions start"><button class="btn btn-primary">${esc(t('common.save'))}</button></div>
    </form>`;
  const form = $<HTMLFormElement>('form', body)!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    save({ allow_price_override: v.allow_price_override, allow_negative_stock: v.allow_negative_stock, track_collection: v.track_collection, require_shift: v.require_shift,
      feature_recipes: v.feature_recipes, feature_flocks: v.feature_flocks, expiry_warn_days: v.expiry_warn_days || '30', whatsapp_country_code: v.whatsapp_country_code || '234', label_item: v.label_item || 'Item', label_items: v.label_items || 'Items', label_customer: v.label_customer || 'Customer' }, $('.btn-primary', form) as HTMLButtonElement, ctx.refreshChrome);
  });
}

// ---------------------------------------------------------------- appearance

function appearance(body: HTMLElement) {
  let primary = store.settings.theme_primary || PRESETS[0].primary;
  let secondary = store.settings.theme_secondary || PRESETS[0].secondary;
  let mode = store.settings.theme_mode || 'light';
  body.innerHTML = `${head('Colours & theme', 'Pick two colours: the action colour for buttons and highlights, and the frame colour for the side menu and receipt emails.')}
    <div class="stack form-narrow">
      <div class="field"><span>Ready-made pairs</span><div class="swatches">${PRESETS.map((p) => `<button type="button" class="swatch" data-preset="${p.primary}|${p.secondary}" aria-label="${esc(p.name)}"><span class="swatch-colors"><i style="background:${p.secondary}"></i><i style="background:${p.primary}"></i></span><span>${esc(p.name)}</span></button>`).join('')}</div></div>
      <div class="grid-2">
        <label class="field"><span>Action colour</span><span class="color-field"><input type="color" data-c="primary" value="${primary}"><input data-hex="primary" value="${primary}" maxlength="7" spellcheck="false" aria-label="Action colour hex"></span></label>
        <label class="field"><span>Frame colour</span><span class="color-field"><input type="color" data-c="secondary" value="${secondary}"><input data-hex="secondary" value="${secondary}" maxlength="7" spellcheck="false" aria-label="Frame colour hex"></span></label>
      </div>
      <p class="small" data-contrast></p>
      <div class="field"><span>Brightness</span><div class="seg" role="radiogroup">
        <label><input type="radio" name="mode" value="light" ${mode === 'light' ? 'checked' : ''}><span>${icon('sun', 15)} Light</span></label>
        <label><input type="radio" name="mode" value="dark" ${mode === 'dark' ? 'checked' : ''}><span>${icon('moon', 15)} Dark</span></label>
      </div></div>
      <div class="theme-sample"><div class="ts-nav"><span></span><span class="on"></span><span></span></div><div class="ts-main"><b>Sample screen</b><p class="muted small">Buttons and highlights use the action colour.</p><div class="ts-row"><button type="button" class="btn btn-primary btn-sm">Take payment</button><span class="pill pill-ok">Paid</span><a class="link-btn">A link</a></div></div></div>
      <div class="form-actions start"><button type="button" class="btn btn-ghost" data-reset>Undo changes</button><button type="button" class="btn btn-primary" data-save>Save colours</button></div>
    </div>`;
  const sync = () => {
    applyTheme(primary, secondary, mode);
    $$<HTMLElement>('[data-preset]', body).forEach((b) => b.classList.toggle('is-on', b.dataset.preset!.toUpperCase() === `${primary}|${secondary}`.toUpperCase()));
    (['primary', 'secondary'] as const).forEach((k) => {
      const v = k === 'primary' ? primary : secondary;
      ($(`[data-c="${k}"]`, body) as HTMLInputElement).value = v;
      ($(`[data-hex="${k}"]`, body) as HTMLInputElement).value = v.toUpperCase();
    });
    const surface = mode === 'dark' ? '#171C24' : '#FFFFFF';
    const c = contrast(primary, surface);
    $('[data-contrast]', body)!.innerHTML = c < 3 ? `${icon('alert', 15)} This action colour is faint against the page. Buttons stay readable, but a deeper shade will be clearer.` : '';
  };
  sync();
  body.addEventListener('click', (e) => {
    const p = (e.target as HTMLElement).closest<HTMLElement>('[data-preset]');
    if (p) { [primary, secondary] = p.dataset.preset!.split('|'); sync(); }
  });
  body.addEventListener('input', (e) => {
    const el = e.target as HTMLInputElement;
    const key = el.dataset.c ?? el.dataset.hex;
    if (key && /^#[0-9a-f]{6}$/i.test(el.value)) { if (key === 'primary') primary = el.value; else secondary = el.value; sync(); }
    if (el.name === 'mode') { mode = el.value; sync(); }
  });
  $('[data-reset]', body)!.addEventListener('click', () => {
    primary = store.settings.theme_primary; secondary = store.settings.theme_secondary; mode = store.settings.theme_mode || 'light';
    ($(`input[name=mode][value=${mode}]`, body) as HTMLInputElement).checked = true;
    sync();
  });
  $('[data-save]', body)!.addEventListener('click', (e) => save({ theme_primary: primary, theme_secondary: secondary, theme_mode: mode }, e.currentTarget as HTMLButtonElement));
}

// ---------------------------------------------------------------- email

function email(body: HTMLElement) {
  const s = store.settings;
  let provider = s.smtp_provider || 'gmail';
  const pwSet = s.smtp_password_set === '1';
  body.innerHTML = `${head('Email', 'Send receipts straight to customers’ inboxes from your own email account.')}
    <form class="stack form-narrow" novalidate>
      <div class="field"><span>Email provider</span><div class="provider-grid" role="radiogroup">
        ${Object.entries(PROVIDERS).map(([k, p]) => `<label><input type="radio" name="smtp_provider" value="${k}" ${provider === k ? 'checked' : ''}><span>${esc(p.name)}</span></label>`).join('')}
      </div></div>
      <div class="callout callout-info" data-help></div>
      <div class="grid-3" data-server>
        <label class="field span-2"><span>SMTP server</span><input name="smtp_host" maxlength="253" value="${esc(s.smtp_host || '')}" spellcheck="false"></label>
        <label class="field"><span>Port</span><input name="smtp_port" inputmode="numeric" maxlength="5" value="${esc(s.smtp_port || '587')}"></label>
      </div>
      <div class="field" data-security><span>Connection security</span><div class="seg" role="radiogroup">
        ${[['starttls', 'STARTTLS (587)'], ['ssl', 'SSL/TLS (465)'], ['none', 'None (local only)']].map(([v, l]) => `<label><input type="radio" name="smtp_security" value="${v}" ${(s.smtp_security || 'starttls') === v ? 'checked' : ''}><span>${l}</span></label>`).join('')}
      </div></div>
      <div class="grid-2">
        <label class="field"><span>Email address / username</span><input name="smtp_username" maxlength="254" value="${esc(s.smtp_username || '')}" autocomplete="off" spellcheck="false" placeholder="yourshop@gmail.com"></label>
        <label class="field"><span data-pwlabel>App password</span><input name="smtp_password" type="password" maxlength="256" autocomplete="new-password" placeholder="${pwSet ? 'Saved. Leave blank to keep it.' : 'Paste the password here'}"></label>
      </div>
      <div class="grid-2">
        <label class="field"><span>Sender name</span><input name="smtp_from_name" maxlength="80" value="${esc(s.smtp_from_name || '')}" placeholder="${esc(s.business_name || '')}"></label>
        <label class="field"><span>Send from address</span><input name="smtp_from_email" type="email" maxlength="254" value="${esc(s.smtp_from_email || '')}" placeholder="Same as the username"></label>
      </div>
      <label class="field"><span>Replies go to <em class="muted">(optional)</em></span><input name="smtp_reply_to" type="email" maxlength="254" value="${esc(s.smtp_reply_to || '')}" placeholder="${esc(s.business_email || 'e.g. manager@yourshop.com')}"></label>
      ${check('email_bcc_self', 'Send me a copy of every receipt', 'A blind copy goes to the sending address, so you have a record in your mailbox.')}
      <div class="form-actions start"><button class="btn btn-primary">Save email settings</button></div>
    </form>
    <section class="test-mail">
      <h3>Send a test email</h3><p class="muted small">Save first, then send a test to check everything works.</p>
      <form class="inline-form" data-test><input type="email" name="to" required placeholder="you@example.com" value="${esc(s.business_email || s.smtp_username || '')}" aria-label="Test recipient"><button class="btn btn-ghost">${icon('mail', 16)} Send test</button></form>
    </section>`;
  const form = $<HTMLFormElement>('form', body)!;
  const apply = (fromUser: boolean) => {
    const p = PROVIDERS[provider];
    $('[data-help]', body)!.innerHTML = `${icon('mail', 18)}<div><b>${esc(p.name)}</b>${p.help}</div>`;
    const host = form.elements.namedItem('smtp_host') as HTMLInputElement;
    const port = form.elements.namedItem('smtp_port') as HTMLInputElement;
    if (fromUser && provider !== 'custom') {
      host.value = p.host; port.value = p.port;
      ($(`input[name=smtp_security][value=${p.security}]`, form) as HTMLInputElement).checked = true;
    }
    if (fromUser && provider === 'custom') { host.value = ''; }
    $('[data-pwlabel]', form)!.textContent = provider === 'gmail' || provider === 'yahoo' ? 'App password' : 'Password';
  };
  apply(false);
  form.addEventListener('change', (e) => {
    const el = e.target as HTMLInputElement;
    if (el.name === 'smtp_provider') { provider = el.value; apply(true); }
    if (el.name === 'smtp_security') { const port = form.elements.namedItem('smtp_port') as HTMLInputElement; if (el.value === 'ssl' && port.value === '587') port.value = '465'; if (el.value === 'starttls' && port.value === '465') port.value = '587'; }
  });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const values: Record<string, string> = {};
    ['smtp_provider', 'smtp_host', 'smtp_port', 'smtp_security', 'smtp_username', 'smtp_password', 'smtp_from_name', 'smtp_from_email', 'smtp_reply_to', 'email_bcc_self'].forEach((k) => (values[k] = v[k] ?? ''));
    save(values, $('.btn-primary', form) as HTMLButtonElement, () => { (form.elements.namedItem('smtp_password') as HTMLInputElement).value = ''; if (values.smtp_password) (form.elements.namedItem('smtp_password') as HTMLInputElement).placeholder = 'Saved. Leave blank to keep it.'; });
  });
  const test = $<HTMLFormElement>('[data-test]', body)!;
  test.addEventListener('submit', (e) => {
    e.preventDefault();
    withBusy(test.querySelector('button'), async () => toast(await call<string>('send_test_email', { recipient: (test.elements.namedItem('to') as HTMLInputElement).value })));
  });
}

// ---------------------------------------------------------------- payment methods

async function payments(body: HTMLElement) {
  const list = await call<PaymentMethod[]>('list_payment_methods');
  body.innerHTML = `${head('Payment methods', 'The buttons cashiers see when taking payment. Turn off any you do not accept.')}
    <section class="form-narrow stack">
      <ul class="cat-list">${list.map((p) => `<li data-id="${p.id}">
        <input value="${esc(p.name)}" maxlength="40" data-name aria-label="Payment method name">
        <input type="number" class="w-num" value="${p.sort_order}" data-sort aria-label="Order">
        <label class="check inline"><input type="checkbox" data-active ${p.active ? 'checked' : ''}><span>In use</span></label>
        <button class="btn btn-ghost btn-sm" data-save>${esc(t('common.save'))}</button></li>`).join('')}</ul>
      <form class="inline-form" data-add><input name="name" maxlength="40" required placeholder="e.g. OPay, Moniepoint, Cheque" aria-label="New payment method"><button class="btn btn-primary">${icon('plus', 16)} Add method</button></form>
      <p class="muted small">Selling on credit does not need a method: take a part payment or none, and the balance is tracked against the ${esc(store.label('customer').toLowerCase())}.</p>
    </section>`;
  body.onclick = (e) => {
    const t = e.target as HTMLElement;
    const li = t.closest<HTMLElement>('li[data-id]');
    if (li && t.closest('[data-save]')) withBusy(t.closest('button'), async () => {
      await call('save_payment_method', { id: Number(li.dataset.id), name: ($('[data-name]', li) as HTMLInputElement).value, active: ($('[data-active]', li) as HTMLInputElement).checked, sortOrder: Number(($('[data-sort]', li) as HTMLInputElement).value) || 0 });
      await store.loadPaymentMethods(); toast('Payment method saved');
    });
  };
  $<HTMLFormElement>('[data-add]', body)!.addEventListener('submit', (e) => {
    e.preventDefault();
    const f = e.currentTarget as HTMLFormElement;
    withBusy(f.querySelector('button'), async () => {
      await call('save_payment_method', { id: null, name: (f.elements.namedItem('name') as HTMLInputElement).value, active: true, sortOrder: list.length + 1 });
      await store.loadPaymentMethods(); toast('Payment method added'); await payments(body);
    });
  });
}

// ---------------------------------------------------------------- backup

async function backup(body: HTMLElement) {
  const [st, restorable] = await Promise.all([call<BackupStatus>('backup_status'), call<Array<{ name: string; path: string; size_bytes: number; modified: string }>>('list_backup_files').catch(() => [])]);
  body.innerHTML = `${head('Backup', 'Everything is stored on this computer only. FarmBuddy copies it automatically once a day, and you can make a copy any time.')}
    <div class="form-narrow stack">
      ${st.last_error ? `<div class="callout callout-warn"><div><b>The last automatic backup failed</b><span>${esc(st.last_error)}</span></div></div>` : ''}
      <div class="figures figures-2"><div><span>Last backup</span><b>${st.last_auto ? esc(relativeDay(st.last_auto)) : 'Not yet'}</b></div><div><span>Backups kept</span><b>${st.files.length} of ${st.keep}</b></div></div>
      <form class="stack" data-bform>
        ${`<label class="check"><input type="checkbox" name="auto" ${st.auto ? 'checked' : ''}><span><b>Back up automatically every day</b><br><small class="muted">Runs in the background while the app is open. Older copies are deleted after the number you keep.</small></span></label>`}
        <label class="field"><span>Backup folder</span><span class="input-affix"><input name="dir" value="${esc(st.dir === st.default_dir ? '' : st.dir)}" placeholder="${esc(st.default_dir)}" spellcheck="false">${isMobile ? '' : '<button type="button" class="affix-btn" data-pick>Choose…</button>'}</span>
          <small>Pick a flash drive or a Google Drive / OneDrive folder on this computer to keep copies off the machine. Empty uses ${esc(st.default_dir)}.</small></label>
        <label class="field"><span>Number of backups to keep</span><input name="keep" type="number" min="1" max="365" value="${st.keep}" class="w-num"></label>
        <p class="form-error" role="alert" data-err></p>
        <div class="form-actions start"><button class="btn btn-primary">Save backup settings</button><button type="button" class="btn btn-ghost" data-now>${icon('download', 16)} Back up now</button></div>
      </form>
      ${st.files.length ? `<details><summary>Backup files (${st.files.length})</summary><ul class="history">${st.files.map((f) => `<li><span class="mono-ish">${esc(f.name)}<small class="muted block">${esc(f.modified)}</small></span><b>${(f.size / 1048576).toFixed(1)} MB</b></li>`).join('')}</ul></details>` : ''}
      <section class="panel"><header class="panel-head"><h2>Restore a backup</h2><p class="muted small">Replaces all data on this device with the backup. FarmBuddy checks the file first, makes a safety copy of what is here now, upgrades older backups, then signs everyone out.</p></header>
        <div class="stack"><label class="field"><span>Backup file</span><select data-restore-pick><option value="">Choose a backup…</option>${restorable.map((f) => `<option value="${esc(f.path)}">${esc(f.name)} · ${esc(f.modified)}</option>`).join('')}</select></label>
          ${isMobile ? '' : '<div><button type="button" class="btn btn-ghost btn-sm" data-restore-browse>Choose another file…</button></div>'}
          <div data-restore-check></div></div></section>
    </div>`;
  const form = $<HTMLFormElement>('[data-bform]', body)!;
  $('[data-pick]', body)?.addEventListener('click', async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const dir = await open({ directory: true, multiple: false, title: 'Choose a backup folder' });
      if (typeof dir === 'string') (form.elements.namedItem('dir') as HTMLInputElement).value = dir;
    } catch (x) { toast(`Folder picker unavailable: ${errText(x)}. Type the folder path instead.`, 'error'); }
  });
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('save_backup_settings', { input: { auto: v.auto === '1', dir: v.dir.trim(), keep: Number(v.keep) || 14 } }); }
      catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Backup settings saved'); await backup(body);
    });
  });
  $<HTMLButtonElement>('[data-now]', body)!.addEventListener('click', (e) => withBusy(e.currentTarget as HTMLButtonElement, async () => {
    toast(`Backup saved: ${await call<string>('backup_now')}`); await backup(body);
  }));
  const checkBox = $('[data-restore-check]', body)!;
  const check = async (path: string) => {
    if (!path) { checkBox.innerHTML = ''; return; }
    checkBox.innerHTML = '<div class="loading"><span></span><span></span><span></span></div>';
    try {
      const c = await call<{ path: string; schema_version: number; needs_upgrade: boolean; business_name: string; users: number; products: number; receipts: number; batches: number; plots: number; size_bytes: number }>('check_backup_file', { path });
      checkBox.innerHTML = `<div class="callout callout-info"><div><b>This backup looks healthy</b><span>${esc(c.business_name || 'Unnamed business')}: ${c.users} staff accounts, ${c.products} items, ${c.receipts} receipts, ${c.batches} livestock batches, ${c.plots} plots. ${(c.size_bytes / 1048576).toFixed(1)} MB.${c.needs_upgrade ? ' It is from an older version and will be upgraded.' : ''}</span></div></div>
        <form class="stack" data-restore-form><label class="field"><span>Your password</span><input type="password" name="password" required autocomplete="current-password"></label>
          <label class="field"><span>Type RESTORE to confirm</span><input name="confirm" required autocapitalize="characters"></label>
          <p class="form-error" role="alert" data-err></p><div class="form-actions start"><button class="btn btn-danger">Restore this backup</button></div></form>`;
      const f = $<HTMLFormElement>('[data-restore-form]', checkBox)!;
      f.addEventListener('submit', (e) => {
        e.preventDefault();
        const v = formValues(f);
        void withBusy($<HTMLButtonElement>('.btn-danger', f), async () => {
          try {
            const r = await call<{ safety_backup: string }>('restore_backup', { input: { path, password: v.password, confirm: v.confirm } });
            alertRestart(r.safety_backup);
          } catch (x) { $('[data-err]', f)!.textContent = errText(x); }
        });
      });
    } catch (x) { checkBox.innerHTML = `<div class="callout callout-danger"><div><b>This file cannot be restored</b><span>${esc(errText(x))}</span></div></div>`; }
  };
  $<HTMLSelectElement>('[data-restore-pick]', body)!.addEventListener('change', (e) => void check((e.target as HTMLSelectElement).value));
  $('[data-restore-browse]', body)?.addEventListener('click', async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const file = await open({ multiple: false, title: 'Choose a FarmBuddy backup', filters: [{ name: 'FarmBuddy backup', extensions: ['sqlite3', 'db', 'sqlite'] }] });
      if (typeof file === 'string') void check(file);
    } catch (x) { toast(errText(x), 'error'); }
  });
}

/** After a restore every session is gone: show where the safety copy is and reload to the sign-in screen. */
function alertRestart(safety: string) {
  const m = modal({ title: 'Backup restored', size: 'sm', body: `<div class="stack"><p>Your data was replaced with the backup. A safety copy of the data from before the restore is here:</p><p class="mono-ish small">${esc(safety)}</p><p>Sign in again with an account from the backup.</p>
    <div class="form-actions"><button class="btn btn-primary" data-reload>Sign in again</button></div></div>` });
  m.el.querySelector('[data-reload]')!.addEventListener('click', () => location.reload());
}

// ---------------------------------------------------------------- start afresh

type ResetResult = { removed_items: number; removed_receipts: number; loaded_items: number; backup_path: string };

async function startAfresh(body: HTMLElement, ctx: ViewCtx) {
  if (!auth.can('owner')) { body.innerHTML = '<p class="muted">Only the Owner can do this.</p>'; return; }
  body.innerHTML = `${head('Start afresh', 'Clear the items and stock you used while trying the app, then set up your real stock.')}
    <div class="form-narrow stack">
      <div class="callout callout-danger"><div><b>This deletes data and cannot be undone from this screen</b><span>A safety backup is made first. Only the Owner can do this.</span></div></div>
      <div class="reset-grid">
        <div class="reset-box"><b>Will be deleted</b><ul>
          <li>All items and services, with their prices and barcodes</li>
          <li>Categories, stock counts, stock history and expiry batches</li>
          <li>Deliveries (receive goods), suppliers and held sales</li>
          <li data-sales-li hidden>Receipts, customers and what they owe, expenses and shifts</li></ul></div>
        <div class="reset-box keep"><b>Stays the same</b><ul>
          <li>Your licence and this device</li>
          <li>Business name, logo, receipt and tax settings</li>
          <li>Payment methods, colours and email settings</li>
          <li>Staff accounts and passwords</li>
          <li data-sales-keep>Receipts, customers, expenses and reports</li></ul></div>
      </div>
      <form class="stack" data-rform autocomplete="off">
        <label class="check"><input type="checkbox" name="include_sales"><span><b>Also delete sales and test records</b><br><small class="muted">Receipts, customers and their balances, expenses and shifts. Choose this if everything so far was only a test.</small></span></label>
        <div class="stack-sm"><span class="field-label">After clearing</span>
          <label class="check"><input type="radio" name="after" value="empty" checked><span><b>Start with an empty item list</b><br><small class="muted">You add your own items in Farm stock.</small></span></label>
          <label class="check"><input type="radio" name="after" value="reload"><span><b>Load the farm’s starter items again</b><br><small class="muted">The starter items for your farm type (names only), then you add prices.</small></span></label>
        </div>
        <label class="field"><span>Your password</span><input name="password" type="password" required autocomplete="current-password"></label>
        <label class="field"><span>Type <b>RESET</b> to confirm</span><input name="confirm" required autocapitalize="characters" spellcheck="false" placeholder="RESET"></label>
        <p class="form-error" role="alert" data-err></p>
        <div class="form-actions start"><button class="btn btn-danger" data-go disabled>${icon('trash', 16)} Start afresh</button></div>
      </form>
    </div>`;
  const form = $<HTMLFormElement>('[data-rform]', body)!;
  const btn = $<HTMLButtonElement>('[data-go]', form)!;
  const sync = () => {
    const v = formValues(form);
    const all = v.include_sales === '1';
    $('[data-sales-li]', body)!.hidden = !all;
    $('[data-sales-keep]', body)!.hidden = all;
    btn.disabled = !(v.password && v.confirm.trim().toUpperCase() === 'RESET');
  };
  form.addEventListener('input', sync);
  form.addEventListener('change', sync);
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const v = formValues(form);
    const all = v.include_sales === '1';
    const ok = await confirmDialog({
      title: 'Delete and start afresh?',
      message: all
        ? 'All items, stock, receipts, customers, expenses and shifts will be deleted. Your licence, settings and staff stay. Are you sure?'
        : 'All items, categories and stock will be deleted. Receipts and reports stay. Are you sure?',
      confirm: 'Yes, delete', danger: true,
    });
    if (ok === null) return;
    const err = $('[data-err]', form)!;
    err.textContent = '';
    await withBusy(btn, async () => {
      try {
        const r = await call<ResetResult>('reset_business_data', { input: { password: v.password, confirm: v.confirm, include_sales: all, reload_items: v.after === 'reload' } });
        store.products = []; store.categories = [];
        await store.loadSettings();
        toast(r.loaded_items ? `Done. ${r.loaded_items} ready-made items loaded. Edit them in Farm stock.` : 'Done. Add your items in Inventory.');
        ctx.go('inventory');
      } catch (x) { err.textContent = errText(x); }
    });
  });
}

// ---------------------------------------------------------------- licence

async function licenceSection(body: HTMLElement) {
  const s = await licence.refresh();
  if (!s) { body.innerHTML = `${head('Licence', '')}<p class="form-error">Licence details could not be read.</p>`; return; }
  const state = s.state === 'licensed' ? `<span class="pill pill-ok">Activated</span>` : s.state === 'trial' ? `<span class="pill pill-info">Free trial</span>` : `<span class="pill pill-bad">Not active</span>`;
  body.innerHTML = `${head('Licence', 'Every computer needs its own licence key. Keys are made by TechAlpha Hub for one Install ID and do not work on any other computer.')}
    <div class="form-narrow stack">
      <div class="figures figures-3"><div><span>Status</span><b>${state}</b></div>
        <div><span>${s.state === 'licensed' ? 'Activated on' : 'Trial'}</span><b>${s.state === 'licensed' ? esc(dateLabel(s.licensed_since)) : `${s.trial_days_left} of ${s.trial_days} days left`}</b></div>
        <div><span>Valid until</span><b>${s.state === 'licensed' ? (s.expires_on ? esc(dateLabel(s.expires_on)) : 'No end date') : '—'}</b></div></div>
      <p class="muted">${esc(s.message)}</p>
      <div class="install-id"><span class="field-label">This computer's Install ID</span><div class="install-id-row"><code>${esc(s.install_id)}</code></div></div>
      ${vendorBlock()}
      <div><button class="btn ${s.state === 'licensed' ? 'btn-ghost' : 'btn-primary'}" data-activate>${icon('key', 16)} ${s.state === 'licensed' ? 'Enter a new key' : 'Activate'}</button></div>
    </div>`;
  $('[data-activate]', body)!.addEventListener('click', () => activationDialog(s, () => licenceSection(body)));
  wireVendor(body, s.install_id);
}

// ---------------------------------------------------------------- updates

async function updates(body: HTMLElement) {
  const version = await appVersion();
  body.innerHTML = `${head('Updates', isMobile
    ? 'New versions download inside the app and install over this one. You are told on screen when one is ready; there is nothing to look for.'
    : 'New versions are published by your provider. The app checks when you sign in, downloads the update in the background and asks you to restart. Only updates signed by the provider are installed.')}
    <div class="form-narrow stack">
      <div class="figures figures-2"><div><span>This version</span><b>${esc(version || 'Unknown')}</b></div><div><span>Updates</span><b>Automatic</b></div></div>
      <div data-uout></div>
      <div class="form-actions start"><button class="btn btn-ghost" data-check>${icon('refresh', 16)} Check now</button></div>
      <p class="muted small">Your data stays in place during an update.${isMobile ? ' The first time, Android may ask you to allow FarmBuddy to install updates. Turn it on and come back to the app.' : ' A backup is still a good idea first (Settings → Backup).'}</p>
    </div>`;
  const out = $('[data-uout]', body)!;
  $('[data-check]', body)!.addEventListener('click', () => checkNow(out));
  checkNow(out);
}

// ---------------------------------------------------------------- account

// ---------------------------------------------------------------- about

async function about(body: HTMLElement) {
  const version = await appVersion();
  const lic = await licence.refresh();
  const platformName = isMobile ? 'Android' : /Mac/i.test(navigator.userAgent) ? 'macOS' : /Win/i.test(navigator.userAgent) ? 'Windows' : 'Desktop';
  body.classList.add('about-sec');
  body.innerHTML = `${head('About FarmBuddy', 'Inventory, sales and receipts for any business. Works offline on Windows, Mac and Android.')}
    <div class="form-narrow stack">
      <div class="about-card">
        <img src="/logo-mark.svg" alt="" class="about-logo">
        <div><b>FarmBuddy</b><span>Version ${esc(version || '—')} · ${esc(platformName)}</span>
        <span>${lic ? (lic.state === 'licensed' ? `Licensed${lic.expires_on ? ` until ${esc(dateLabel(lic.expires_on))}` : ''}` : lic.state === 'trial' ? `Free trial, ${lic.trial_days_left} day${lic.trial_days_left === 1 ? '' : 's'} left` : 'Not activated') : ''}</span></div>
      </div>
      <h3>Contact us</h3>
      <div class="contact-list">
        <div><span>Company</span><b>${esc(VENDOR.name)}</b></div>
        <div><span>WhatsApp / phone</span><b>${esc(VENDOR.phoneDisplay)}</b></div>
        <div><span>Email</span><b>${esc(VENDOR.email)}</b></div>
        <div><span>Licence</span><b>${VENDOR.prices.map(esc).join(' · ')}</b></div>
      </div>
      <div class="row-actions about-actions">
        <button type="button" class="btn btn-primary" data-about-wa>${icon('chat', 16)} Chat on WhatsApp</button>
        <button type="button" class="btn btn-ghost" data-about-mail>${icon('mail', 16)} Send an email</button>
      </div>
      <p class="muted small">Built by TechAlpha Global Concept. Your records stay on this ${isMobile ? 'phone' : 'computer'}; sales, prices and customer details are never sent anywhere unless you email or WhatsApp them.</p>
    </div>`;
  const hello = `Hello ${VENDOR.name}, I use FarmBuddy ${version} (${platformName}).${lic ? ` Install ID: ${lic.install_id}.` : ''}\n`;
  $('[data-about-wa]', body)!.addEventListener('click', () => shareWhatsApp(VENDOR.whatsapp, hello));
  $('[data-about-mail]', body)!.addEventListener('click', () => openExternal(`mailto:${VENDOR.email}?subject=${encodeURIComponent('FarmBuddy')}&body=${encodeURIComponent(hello)}`));
}

function account(body: HTMLElement) {
  const s = auth.session!;
  body.innerHTML = `${head('My account', `Signed in as <b>${esc(s.username)}</b>.`)}
    <form class="stack form-narrow" novalidate>
      <label class="field"><span>Current password</span><input name="current" type="password" required autocomplete="current-password"></label>
      <div class="grid-2"><label class="field"><span>New password</span><input name="next" type="password" minlength="8" required autocomplete="new-password"><small>At least 8 characters.</small></label>
      <label class="field"><span>Repeat new password</span><input name="again" type="password" minlength="8" required autocomplete="new-password"></label></div>
      <p class="form-error" role="alert" data-err></p>
      <div class="form-actions start"><button class="btn btn-primary">Change password</button></div>
    </form>`;
  const form = $<HTMLFormElement>('form', body)!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const err = $('[data-err]', form)!;
    err.textContent = '';
    if (v.next.length < 8) { err.textContent = 'New password must be at least 8 characters.'; return; }
    if (v.next !== v.again) { err.textContent = 'The new passwords do not match.'; return; }
    withBusy($('.btn-primary', form) as HTMLButtonElement, async () => {
      try { await call('change_own_password', { currentPassword: v.current, newPassword: v.next }); } catch (x) { err.textContent = errText(x); return; }
      form.reset(); toast('Password changed');
    });
  });
}

// ---------------------------------------------------------------- security & recovery

interface SecurityStatus { key_set: boolean; questions: string[]; updated_at: string; events: Array<{ event: string; username: string; detail: string; created_at: string }> }

const EVENT_LABEL: Record<string, string> = {
  recovery_failed: 'Failed recovery attempt',
  password_reset_by_recovery: 'Password reset with recovery',
  owner_created_by_recovery: 'Owner account created with recovery',
  recovery_key_changed: 'New recovery key created',
  security_questions_changed: 'Security questions changed',
};

async function security(body: HTMLElement) {
  const st = await call<SecurityStatus>('security_status');
  const ready = st.key_set && st.questions.length === 3;
  body.innerHTML = `${head('Security & recovery', 'If an owner password is lost, the recovery key or the three security answers let you reset any password, or create a new owner account, from the sign-in screen. It all works offline.')}
    <div class="form-narrow stack">
      ${ready ? '' : `<div class="callout callout-warn"><div><b>Recovery is not fully set up.</b><span>Set it up now so a forgotten owner password never locks you out.</span></div></div>`}
      <table class="table status-table"><tbody>
        <tr><th>Recovery key</th><td>${st.key_set ? '<span class="pill pill-ok">Set</span>' : '<span class="pill pill-bad">Not set</span>'}</td><td class="muted small">The key itself is never shown again after it is created.</td></tr>
        <tr><th>Security questions</th><td>${st.questions.length === 3 ? '<span class="pill pill-ok">Set</span>' : '<span class="pill pill-bad">Not set</span>'}</td><td class="muted small">${st.questions.map(esc).join('<br>') || 'No questions yet'}</td></tr>
      </tbody></table>

      <section class="sub-panel stack">
        <div><h3>${st.key_set ? 'Create a new recovery key' : 'Create a recovery key'}</h3><p class="muted small">Do this if the printed key is lost or someone else may have seen it. The old key stops working at once.</p></div>
        <form class="inline-form" data-newkey><input type="password" name="current" placeholder="Your current password" autocomplete="current-password" required aria-label="Your current password"><button class="btn btn-primary">${icon('key', 16)} Create key</button></form>
      </section>

      <section class="sub-panel stack">
        <div><h3>${st.questions.length === 3 ? 'Change security questions' : 'Set security questions'}</h3><p class="muted small">All three answers are needed to recover. Answers are not case-sensitive.</p></div>
        <form class="stack" data-questions novalidate>
          ${questionFields(st.questions)}
          <label class="field"><span>Your current password</span><input type="password" name="current" autocomplete="current-password" required></label>
          <p class="form-error" role="alert" data-err></p>
          <div class="form-actions start"><button class="btn btn-primary">Save security questions</button></div>
        </form>
      </section>

      <section class="stack">
        <h3>Recent security activity</h3>
        ${st.events.length ? `<table class="table"><thead><tr><th>When</th><th>What happened</th><th>Account</th></tr></thead><tbody>
          ${st.events.map((ev) => `<tr><td>${esc(ev.created_at)}</td><td>${esc(EVENT_LABEL[ev.event] ?? ev.event)}${ev.detail ? ` <small class="muted">(${esc(ev.detail)})</small>` : ''}</td><td>${esc(ev.username || '—')}</td></tr>`).join('')}
        </tbody></table>` : '<p class="muted small">Nothing yet. Recovery use and failed attempts appear here.</p>'}
      </section>
    </div>`;

  const keyForm = $<HTMLFormElement>('[data-newkey]', body)!;
  keyForm.addEventListener('submit', (e) => {
    e.preventDefault();
    const pw = (keyForm.elements.namedItem('current') as HTMLInputElement).value;
    withBusy(keyForm.querySelector('button'), async () => {
      const key = await call<string>('regenerate_recovery_key', { currentPassword: pw });
      keyForm.reset();
      showKeyModal(key);
      await security(body);
    });
  });
  const qForm = $<HTMLFormElement>('[data-questions]', body)!;
  wireQuestionFields(qForm);
  qForm.addEventListener('submit', (e) => {
    e.preventDefault();
    const err = $('[data-err]', qForm)!;
    err.textContent = '';
    const qa = readQuestions(qForm);
    if (typeof qa === 'string') { err.textContent = qa; return; }
    const pw = (qForm.elements.namedItem('current') as HTMLInputElement).value;
    if (!pw) { err.textContent = 'Enter your current password.'; return; }
    withBusy($<HTMLButtonElement>('.btn-primary', qForm), async () => {
      try { await call('save_security_questions', { currentPassword: pw, questions: qa.questions, answers: qa.answers }); }
      catch (x) { err.textContent = errText(x); return; }
      toast('Security questions saved');
      await security(body);
    });
  });
}

function showKeyModal(key: string) {
  const m = modal({ title: 'Your new recovery key', size: 'sm', body: `<div class="stack">
    <p class="lead-text">Write this down or print it now. It is not shown again, and the old key no longer works.</p>
    <div class="key-value">${esc(key)}</div>
    <div class="form-actions"><button class="btn btn-ghost" data-print>${icon('print', 16)} Print</button><button class="btn btn-primary" data-close>I have saved it</button></div>
  </div>` });
  m.el.querySelector('[data-print]')!.addEventListener('click', () => printHtml(`<article class="key-sheet"><h1>FarmBuddy recovery key</h1><p><b>${esc(store.settings.business_name || '')}</b> · ${esc(new Date().toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric' }))}</p><div class="key-sheet-key">${esc(key)}</div><p>Use with Forgot password or Create a new owner account on the sign-in screen. Keep it private and away from the shop counter.</p></article>`));
}
