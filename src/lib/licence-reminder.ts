/**
 * Once a day, people still on the free trial (or whose licence ends within a
 * week) see how many days are left and how to activate. "Remind me tomorrow"
 * closes it until the next day.
 */
import { auth } from '../api';
import type { LicenseStatus } from '../types';
import { $, esc, modal } from './dom';
import { dateLabel } from './format';
import { icon } from './icons';
import { activationDialog, licence, vendorBlock, wireVendor } from './license-ui';

const KEY = 'farmbuddy.licenceReminder';
const today = () => new Date().toISOString().slice(0, 10);

function lastShown(): string { try { return localStorage.getItem(KEY) ?? ''; } catch { return ''; } }
function markShown() { try { localStorage.setItem(KEY, today()); } catch { /* private mode: shown again next time */ } }

function daysUntil(isoDate: string): number {
  const end = new Date(`${isoDate}T23:59:59`);
  return Math.ceil((end.getTime() - Date.now()) / 86_400_000);
}

/** Why a reminder is due today, or null. */
export function reminderReason(s: LicenseStatus | null): { title: string; days: number; text: string } | null {
  if (!s) return null;
  if (s.state === 'trial') {
    const d = Math.max(0, s.trial_days_left);
    return { title: 'You are using the free trial', days: d, text: `Activate the full version to keep selling, printing receipts and seeing reports after the trial ends.` };
  }
  if (s.state === 'licensed' && s.expires_on) {
    const d = daysUntil(s.expires_on);
    if (d <= 7) return { title: 'Your licence is ending soon', days: Math.max(0, d), text: `It ends on ${dateLabel(s.expires_on)}. Renew now so the app keeps working without a break.` };
  }
  return null;
}

/** Shows the reminder if one is due and it was not shown today. Resolves when closed. */
export function dailyLicenceReminder(): Promise<void> {
  const s = licence.status;
  const why = reminderReason(s);
  if (!s || !why || lastShown() === today()) return Promise.resolve();
  markShown();
  const owner = auth.can('owner');
  return new Promise((resolve) => {
    const m = modal({
      title: why.title,
      size: 'sm',
      onClose: () => resolve(),
      body: `<div class="stack licence-reminder">
        <div class="lr-days"><b>${why.days}</b><span>day${why.days === 1 ? '' : 's'} left</span></div>
        <p>${esc(why.text)}</p>
        ${owner ? vendorBlock() : '<p class="muted">Please ask the owner of this business to activate FarmBuddy.</p>'}
        <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>Remind me tomorrow</button>${owner ? `<button type="button" class="btn btn-primary" data-act>${icon('key', 16)} I have a key</button>` : ''}</div>
      </div>`,
    });
    if (owner) {
      wireVendor(m.el, s.install_id);
      $('[data-act]', m.el)?.addEventListener('click', () => { m.close(); activationDialog(s); });
    }
  });
}
