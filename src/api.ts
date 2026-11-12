import { invoke } from '@tauri-apps/api/core';
import type { Role, SessionInfo } from './types';

const PUBLIC = new Set(['setup_status', 'setup_owner', 'setup_farm', 'setup_flow', 'setup_check_step', 'login', 'branding', 'list_templates', 'list_farm_templates', 'list_advisory_flows', 'recovery_options', 'recover_account', 'print_window', 'license_status', 'activate_license']);

let session: SessionInfo | null = null;
let onExpired: (() => void) | null = null;
let onLicence: (() => void) | null = null;

export const auth = {
  get session() { return session; },
  set(s: SessionInfo | null) { session = s; },
  onExpired(fn: () => void) { onExpired = fn; },
  onLicenceRequired(fn: () => void) { onLicence = fn; },
  can(min: Role): boolean {
    const rank: Record<Role, number> = { worker: 1, adviser: 1, cashier: 1, manager: 2, owner: 3 };
    return !!session && rank[session.role] >= rank[min];
  },
  /** Selling and money screens: cashiers and above, never farm workers or advisers. */
  canSell(): boolean { return !!session && session.role !== 'worker' && session.role !== 'adviser'; },
  /** Owner or adviser: may approve vaccination schedules, guides and advice. */
  canReview(): boolean { return !!session && (session.role === 'owner' || session.role === 'adviser'); },
  /** Manager, owner or adviser: may enter vaccination plans and crop health advice. */
  canAdvise(): boolean { return !!session && (session.role === 'adviser' || this.can('manager')); },
};

/** Calls a backend command, attaching the session token for protected commands. */
export async function call<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  const payload = PUBLIC.has(name) ? args : { ...args, sessionToken: session?.token ?? '' };
  try {
    return await invoke<T>(name, payload);
  } catch (err) {
    const message = typeof err === 'string' ? err : err instanceof Error ? err.message : String(err);
    if (/session has expired/i.test(message) && onExpired) onExpired();
    if (message.startsWith('LICENCE_REQUIRED') && onLicence) { onLicence(); throw new Error('This copy of FarmBuddy needs to be activated.'); }
    throw new Error(message);
  }
}
