import { call } from '../api';
import { exportCsv } from '../lib/csv';
import { $, debounce, esc } from '../lib/dom';
import { dateTimeLabel } from '../lib/format';
import { icon } from '../lib/icons';
import { t } from '../lib/i18n';
import { tableHtml } from '../lib/ui';
import type { ViewCtx } from './shell';

interface Entry { id: number; at: string; user_name: string; action: string; entity: string; entity_id: number | null; details: string }

/** Owner only: who changed what. Sensitive changes (deletions, staff, settings, restores, stock adjustments, voids) are written here and cannot be edited. */
export async function render(ctx: ViewCtx) {
  ctx.header(t('nav.audit', {}, 'Activity log'), 'Who changed what, and when. Entries cannot be edited or deleted.');
  ctx.main.innerHTML = `<div class="toolbar"><label class="search-field grow">${icon('search', 16)}<input type="search" data-q placeholder="Search by person, action or detail" aria-label="Search the activity log"></label>
    <button class="btn btn-ghost" data-csv>${icon('download', 16)} Export CSV</button></div><div data-list></div>`;
  let rows: Entry[] = [];
  const load = async () => {
    rows = await call<Entry[]>('list_audit', { query: ($('[data-q]', ctx.main) as HTMLInputElement).value });
    $('[data-list]', ctx.main)!.innerHTML = tableHtml(['When', 'Who', 'Action', 'Record', 'Details'], rows.map((r) => [esc(dateTimeLabel(r.at)), esc(r.user_name), esc(r.action.replace(/_/g, ' ')), esc(`${r.entity}${r.entity_id ? ` #${r.entity_id}` : ''}`), esc(r.details)]), 'No entries match.');
  };
  $('[data-q]', ctx.main)!.addEventListener('input', debounce(() => void load(), 250));
  $('[data-csv]', ctx.main)!.addEventListener('click', () => void exportCsv('activity-log.csv', ['When', 'Who', 'Action', 'Record', 'Record id', 'Details'], rows.map((r) => [r.at, r.user_name, r.action, r.entity, r.entity_id ?? '', r.details])));
  await load();
}
