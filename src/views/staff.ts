import { t } from '../lib/i18n';
import { auth, call } from '../api';
import { $, errText, esc, formValues, modal, toast, withBusy } from '../lib/dom';
import { relativeDay } from '../lib/format';
import { icon } from '../lib/icons';
import type { Farm, Role, UserRow } from '../types';
import type { ViewCtx } from './shell';

const ROLES: Array<{ key: Role; name: string; can: string }> = [
  { key: 'worker', name: 'Farm worker', can: 'Enters daily records, feed, eggs, field work, weather and water readings on the farms they are assigned to. No sales, prices, costs or profit.' },
  { key: 'adviser', name: 'Adviser', can: 'Vet, extension officer or agronomist. Reads farm records, plans vaccinations, approves schedules, guides and articles, confirms crop problems. No sales or money.' },
  { key: 'cashier', name: 'Cashier', can: 'Makes sales, prints and emails receipts, records payments and adds customers. Sees only the receipts they issued.' },
  { key: 'manager', name: 'Manager', can: 'Everything a cashier does, plus inventory, stock, all receipts, voids and reports.' },
  { key: 'owner', name: 'Owner', can: 'Full access, including staff accounts, settings, email setup and backups.' },
];

export async function render(ctx: ViewCtx) {
  ctx.header(t('page.staff.title', {}, 'Staff'), t('page.staff.subtitle', {}, 'Give each person their own sign-in so every sale carries a name.'), `<button class="btn btn-primary" data-new>${icon('plus', 16)} Add staff</button>`);
  const draw = async () => {
    const [users, farms] = await Promise.all([call<UserRow[]>('list_users'), call<Farm[]>('list_farms').catch(() => [] as Farm[])]);
    ctx.main.innerHTML = `<div class="staff-layout">
      <section class="panel flush"><div class="table-scroll"><table class="table">
        <thead><tr><th>Name</th><th>Username</th><th>Role</th><th>Last signed in</th><th>Status</th><th class="actions-col"><span class="sr-only">Actions</span></th></tr></thead>
        <tbody>${users.map((u) => `<tr class="${u.active ? '' : 'is-archived'}">
          <td><b>${esc(u.full_name || u.username)}</b>${u.id === auth.session!.user_id ? ' <span class="pill pill-info">You</span>' : ''}</td>
          <td class="mono-ish">${esc(u.username)}</td>
          <td><span class="role role-${u.role}">${esc(ROLES.find((r) => r.key === u.role)!.name)}</span></td>
          <td>${u.last_login_at ? esc(relativeDay(u.last_login_at)) : '<span class="muted">Never</span>'}</td>
          <td>${u.active ? '<span class="pill pill-ok">Active</span>' : '<span class="pill pill-mute">Deactivated</span>'}</td>
          <td class="actions-col"><button class="btn btn-ghost btn-sm" data-edit="${u.id}">${icon('edit', 15)} Edit</button>${['worker', 'adviser', 'cashier'].includes(u.role) && farms.length > 1 ? `<button class="btn btn-ghost btn-sm" data-farms="${u.id}">${icon('home', 15)} Farms</button>` : ''}</td>
        </tr>`).join('')}</tbody></table></div></section>
      <aside class="panel role-guide"><h2>What each role can do</h2>${ROLES.slice().reverse().map((r) => `<div><span class="role role-${r.key}">${r.name}</span><p>${esc(r.can)}</p></div>`).join('')}</aside>
    </div>`;
    ctx.main.onclick = (e) => {
      const b = (e.target as HTMLElement).closest<HTMLElement>('[data-edit]');
      if (b) userForm(users.find((u) => u.id === Number(b.dataset.edit))!, draw);
      const fb = (e.target as HTMLElement).closest<HTMLElement>('[data-farms]');
      if (fb) void farmAccess(users.find((u) => u.id === Number(fb.dataset.farms))!, farms);
    };
  };
  await draw();
  $('[data-new]')!.addEventListener('click', () => userForm(null, draw));
}

function userForm(u: UserRow | null, done: () => void) {
  const self = u?.id === auth.session!.user_id;
  const m = modal({ title: u ? `Edit ${u.full_name || u.username}` : 'Add staff member', size: 'md', body: `<form class="stack" novalidate>
    <div class="grid-2">
      <label class="field"><span>Full name</span><input name="full_name" required maxlength="80" value="${esc(u?.full_name ?? '')}" autofocus></label>
      <label class="field"><span>Username</span><input name="username" required minlength="3" maxlength="64" spellcheck="false" autocapitalize="off" value="${esc(u?.username ?? '')}"><small>Used to sign in.</small></label>
    </div>
    <div class="field"><span>Role</span><div class="role-pick" role="radiogroup">
      ${ROLES.map((r) => `<label class="${self && u!.role !== r.key ? 'is-disabled' : ''}"><input type="radio" name="role" value="${r.key}" ${(u?.role ?? 'cashier') === r.key ? 'checked' : ''} ${self && u!.role !== r.key ? 'disabled' : ''}><span><b>${r.name}</b><small>${esc(r.can)}</small></span></label>`).join('')}
    </div>${self ? '<small class="muted">You cannot change your own role.</small>' : ''}</div>
    <label class="field"><span>${u ? 'New password' : 'Password'}</span><input name="password" type="password" minlength="8" autocomplete="new-password" ${u ? 'placeholder="Leave blank to keep the current password"' : 'required'}><small>At least 8 characters. Share it privately; they can change it under Settings → My account.</small></label>
    ${u && !self ? `<label class="check"><input type="checkbox" name="active" ${u.active ? 'checked' : ''}><span><b>Account active</b><br><small class="muted">Deactivated staff cannot sign in. Their past sales stay on record.</small></span></label>` : ''}
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${u ? 'Save changes' : 'Add staff member'}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const v = formValues(form);
    const err = $('[data-err]', form)!;
    if (!u && (v.password ?? '').length < 8) { err.textContent = 'Password must be at least 8 characters.'; return; }
    withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try {
        await call('save_user', { input: { id: u?.id ?? null, username: v.username, full_name: v.full_name, role: v.role ?? u?.role, active: u && !self ? v.active === '1' : true, password: v.password || null } });
      } catch (x) { err.textContent = errText(x); return; }
      toast(u ? 'Staff member updated' : `${v.full_name} can now sign in as ${v.username}`);
      m.close(); done();
    });
  });
}

/** Which farms a worker, adviser or cashier sees. None ticked = all farms. Owners and managers always see every farm. */
async function farmAccess(u: UserRow, farms: Farm[]) {
  const mine = await call<number[]>('get_user_farms', { userId: u.id });
  const m = modal({ title: `Farms for ${u.full_name || u.username}`, size: 'sm', body: `<form class="stack">
    <p class="muted small">Tick the farms this person works on. They will only see batches, plots and processing on those farms. Leave all unticked to allow every farm.</p>
    ${farms.map((f) => `<label class="check"><input type="checkbox" name="farm" value="${f.id}" ${mine.includes(f.id) ? 'checked' : ''}><span>${esc(f.name)}</span></label>`).join('')}
    <p class="form-error" role="alert" data-err></p>
    <div class="form-actions"><button type="button" class="btn btn-ghost" data-close>${esc(t('common.cancel'))}</button><button class="btn btn-primary">${esc(t('common.save'))}</button></div></form>` });
  const form = m.el.querySelector('form')!;
  form.addEventListener('submit', (e) => {
    e.preventDefault();
    const ids = [...form.querySelectorAll<HTMLInputElement>('input[name=farm]:checked')].map((i) => Number(i.value));
    void withBusy($<HTMLButtonElement>('.btn-primary', form), async () => {
      try { await call('set_user_farms', { userId: u.id, farmIds: ids }); } catch (x) { $('[data-err]', form)!.textContent = errText(x); return; }
      toast('Farm access saved. It applies the next time they open a page.'); m.close();
    });
  });
}
