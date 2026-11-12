import { copyText } from './clipboard';
import { receivePayment } from './customer-pay';
import { toast } from './dom';
import { icon } from './icons';
import { shareWhatsApp } from './whatsapp';

/**
 * Small additions that apply to every page without touching each page's code.
 * Runs once at start-up and again whenever a page redraws itself.
 */

/** Install ID (the code you send to get a licence key): a Copy button and a WhatsApp button next to it. */
function enhanceInstallIds() {
  document.querySelectorAll<HTMLElement>('.install-id-row:not([data-enhanced])').forEach((row) => {
    const id = row.querySelector('code')?.textContent?.trim();
    if (!id) return;
    row.dataset.enhanced = '1';
    row.querySelectorAll('[data-copy]').forEach((old) => old.remove());
    const actions = document.createElement('div');
    actions.className = 'install-id-actions';
    actions.innerHTML = `<button type="button" class="btn btn-primary btn-sm" data-copy-id>Copy</button>
      <button type="button" class="btn btn-ghost btn-sm" data-send-id>${icon('chat', 15)} WhatsApp</button>`;
    row.append(actions);
    actions.querySelector('[data-copy-id]')!.addEventListener('click', async () => {
      toast((await copyText(id)) ? 'Install ID copied' : 'Could not copy. Press and hold the code to select it.', 'info');
    });
    actions.querySelector('[data-send-id]')!.addEventListener('click', () => { void shareWhatsApp('', `FarmBuddy Install ID: ${id}`); });
  });
}

/**
 * On a phone, tables turn into stacked cards (see mobile-fixes.css) so no column or button sits
 * off-screen. The cards need each cell to know its column heading, which is set here.
 */
function labelTables() {
  document.querySelectorAll<HTMLTableElement>('table.table').forEach((table) => {
    const heads = Array.from(table.tHead?.rows[0]?.cells ?? []).map((c) => (c.textContent ?? '').trim());
    if (!heads.length) return;
    Array.from(table.tBodies).forEach((body) => Array.from(body.rows).forEach((row) => {
      let col = 0;
      Array.from(row.cells).forEach((cell) => {
        if (cell.tagName === 'TD' && !cell.hasAttribute('data-label')) cell.setAttribute('data-label', heads[col] ?? '');
        col += cell.colSpan || 1;
      });
    }));
  });
}

/** Customers who owe: a "Receive payment" button next to Receipts / Statement. */
function enhanceCustomerRows() {
  document.querySelectorAll<HTMLElement>('[data-receipts]:not([data-pay-added])').forEach((btn) => {
    btn.dataset.payAdded = '1';
    if (!btn.closest('tr')?.querySelector('b.warn-text')) return;
    const pay = document.createElement('button');
    pay.type = 'button';
    pay.className = 'btn btn-primary btn-sm';
    pay.textContent = 'Receive payment';
    pay.dataset.payCustomer = btn.dataset.receipts ?? '';
    btn.before(pay);
  });
}
document.addEventListener('click', (e) => {
  const b = (e.target as HTMLElement).closest<HTMLElement>('[data-pay-customer]');
  if (!b) return;
  void receivePayment(Number(b.dataset.payCustomer), () => {
    document.querySelector<HTMLInputElement>('[data-search]')?.dispatchEvent(new Event('input'));
  });
});

let queued = false;
function run() {
  queued = false;
  enhanceInstallIds();
  labelTables();
  enhanceCustomerRows();
}
new MutationObserver(() => {
  if (!queued) { queued = true; requestAnimationFrame(run); }
}).observe(document.body, { childList: true, subtree: true });
run();
