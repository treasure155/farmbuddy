import { auth, call } from '../api';
import { $, confirmDialog, errText, esc, modal, toast } from '../lib/dom';
import { dateLabel, isoDate, money, qty, toKobo } from '../lib/format';
import { icon } from '../lib/icons';
import { tl } from '../lib/i18n';
import { fmtNum, formModal, num, pill, tableHtml, type Field } from '../lib/ui';
import type { Farm, Flock } from '../types';

interface Animal { id: number; flock_id: number; flock_code: string; tag: string; name: string; sex: string; breed: string; birth_date: string; acquired_date: string; dam_id: number | null; dam_tag: string; sire_id: number | null; sire_tag: string; status: string; status_date: string; notes: string; latest_weight_kg: number; latest_weight_date: string; weights: Array<[string, number]> }
interface Breeding { id: number; flock_id: number; flock_code: string; dam_id: number | null; dam_tag: string; sire_id: number | null; sire_tag: string; sire_text: string; mating_date: string; method: string; expected_due: string; status: string; notes: string; births: number }
interface Pond { id: number; farm_id: number | null; code: string; pond_type: string; size: number; size_unit: string; notes: string; status: string; is_demo: boolean; stocked: number; batches: string[]; last_water_date: string }
interface Water { id: number; pond_id: number; record_date: string; temp_c: number | null; ph: number | null; dissolved_oxygen: number | null; ammonia: number | null; notes: string; user_name: string }

const today = () => isoDate(new Date());
const SEX: Array<[string, string]> = [['female', 'Female'], ['male', 'Male'], ['unknown', 'Not known']];
const age = (birth: string) => {
  if (!birth) return '';
  const days = Math.floor((Date.now() - new Date(birth).getTime()) / 86400000);
  return days < 60 ? `${days} days` : days < 730 ? `${Math.floor(days / 30.4)} months` : `${(days / 365).toFixed(1)} years`;
};

/** More animals arriving into an existing batch (bought, gifted, restocked). */
export function receiveForm(f: Flock, done: () => void) {
  formModal({ title: `Add animals to ${f.code}`, intro: 'For animals bought or brought in after the batch started. Births have their own form.', fields: ([
    { name: 'date', label: 'Date', type: 'date', value: today(), required: true, pair: true },
    { name: 'qty', label: 'Number received', type: 'number', required: true },
    { name: 'cost', label: 'Cost (total)', type: 'money', pair: auth.can('manager') },
    { name: 'source', label: 'From (seller, hatchery)' },
    { name: 'notes', label: 'Notes', type: 'textarea' },
  ] as Field[]).filter((x) => x.name !== 'cost' || auth.can('manager')), done: 'Animals added', submit: async (v) => {
    await call('receive_animals', { input: { flock_id: f.id, date: v.date, quantity: Math.round(num(v.qty) ?? 0), cost_kobo: v.cost ? toKobo(v.cost) : 0, source: v.source ?? '', notes: v.notes } });
    done();
  } });
}

/** Moves animals to another batch of the same type (e.g. bucks to their own pen, fish graded into another pond). */
export async function transferForm(f: Flock, done: () => void) {
  const all = await call<Flock[]>('list_flocks', { includeClosed: false, group: null });
  const targets = all.filter((x) => x.id !== f.id && x.species === f.species);
  if (!targets.length) { toast(`Start another ${tl('species', f.species)} batch first, then move animals into it.`, 'info'); return; }
  formModal({ title: `Move animals out of ${f.code}`, intro: `${qty(f.alive)} alive now. The total on the farm does not change.`, fields: [
    { name: 'to', label: 'To batch', type: 'select', options: targets.map((x): [string, string] => [String(x.id), `${x.code} (${qty(x.alive)} alive)`]), required: true },
    { name: 'date', label: 'Date', type: 'date', value: today(), required: true, pair: true },
    { name: 'qty', label: 'Number moved', type: 'number', required: true },
    { name: 'notes', label: 'Reason', type: 'textarea' },
  ], done: 'Animals moved', submit: async (v) => {
    await call('transfer_animals', { input: { from_flock_id: f.id, to_flock_id: Number(v.to), date: v.date, quantity: Math.round(num(v.qty) ?? 0), notes: v.notes } });
    done();
  } });
}

/** Eggs sold, used on the farm or lost, out of the egg stock collected. */
export function eggsForm(f: Flock, done: () => void) {
  const seller = auth.canSell();
  formModal({ title: `Eggs from ${f.code}`, intro: `${qty(f.egg_stock)} eggs in stock (collected ${qty(f.eggs)}, damaged ${qty(f.eggs_damaged)}, sold ${qty(f.eggs_sold)}, used ${qty(f.eggs_used)}, lost ${qty(f.eggs_lost)}).`, fields: [
    { name: 'kind', label: 'What happened', type: 'select', options: [...(seller ? [['sold', 'Sold'] as [string, string]] : []), ['used', 'Used on the farm'], ['lost', 'Lost or spoiled']], required: true, pair: true },
    { name: 'eggs', label: 'Number of eggs', type: 'number', required: true, help: '1 crate = 30 eggs' },
    { name: 'date', label: 'Date', type: 'date', value: today(), required: true, pair: seller },
    ...(seller ? [{ name: 'amount', label: 'Amount received', type: 'money' as const }, { name: 'buyer', label: 'Buyer' }] : []),
    { name: 'notes', label: 'Notes', type: 'textarea' },
  ], done: 'Eggs recorded', submit: async (v) => {
    await call('record_eggs', { input: { flock_id: f.id, date: v.date, kind: v.kind, eggs: Math.round(num(v.eggs) ?? 0), amount_kobo: v.amount ? toKobo(v.amount) : 0, buyer: v.buyer ?? '', notes: v.notes } });
    done();
  } });
}

/** The animal register of one batch: tag, sex, date of birth, parents, weights and status. */
export async function animalsModal(f: Flock, done: () => void) {
  const m = modal({ title: `Animals in ${f.code}`, subtitle: `${tl('species', f.species)} · ${qty(f.alive)} alive · ${qty(f.animals_registered)} with a tag`, size: 'lg', onClose: done, body: '<div data-animals class="stack"></div>' });
  const box = $('[data-animals]', m.el)!;
  const draw = async () => {
    const list = await call<Animal[]>('list_animals', { flockId: f.id, includeGone: true });
    const active = list.filter((a) => a.status === 'active');
    box.innerHTML = `<p class="muted small">Tag the animals you want to follow one by one (breeding stock, cattle, pigs). The batch count stays the total; the register can never hold more than are alive.</p>
      ${f.status === 'active' ? `<div class="row-actions"><button class="btn btn-primary btn-sm" data-add>${icon('plus', 14)} Register an animal</button></div>` : ''}
      ${tableHtml(['Tag', 'Sex', 'Born (age)', 'Parents', 'Weight', 'Status', ''], list.map((a) => [`<b>${esc(a.tag)}</b>${a.name ? ` <small class="muted">${esc(a.name)}</small>` : ''}`, esc(a.sex), a.birth_date ? `${esc(dateLabel(a.birth_date))} <small class="muted">${esc(age(a.birth_date))}</small>` : '—',
        esc([a.dam_tag && `dam ${a.dam_tag}`, a.sire_tag && `sire ${a.sire_tag}`].filter(Boolean).join(', ') || '—'), a.latest_weight_kg ? `${fmtNum(a.latest_weight_kg)} kg <small class="muted">${esc(dateLabel(a.latest_weight_date))}</small>` : '—',
        a.status === 'active' ? pill('Active', 'ok') : pill(`${a.status} ${dateLabel(a.status_date)}`),
        a.status === 'active' ? `<button class="btn btn-ghost btn-sm" data-weigh="${a.id}">Weigh</button><button class="btn btn-ghost btn-sm" data-edit="${a.id}">Edit</button><button class="btn btn-ghost btn-sm" data-out="${a.id}">Sold / died</button>` : '']), 'No animals registered yet.')}`;
    const form = (a: Animal | null) => formModal({ title: a ? `Edit ${a.tag}` : 'Register an animal', fields: [
      { name: 'tag', label: 'Tag or ear number', value: a?.tag ?? '', required: true, pair: true },
      { name: 'name', label: 'Name (optional)', value: a?.name ?? '' },
      { name: 'sex', label: 'Sex', type: 'select', options: SEX, value: a?.sex ?? 'female', pair: true },
      { name: 'breed', label: 'Breed', value: a?.breed ?? f.breed },
      { name: 'birth', label: 'Date of birth', type: 'date', value: a?.birth_date ?? '', pair: true },
      { name: 'acquired', label: 'Date bought or brought in', type: 'date', value: a?.acquired_date ?? '' },
      { name: 'dam', label: 'Mother (dam)', type: 'select', options: [['', '—'], ...active.filter((x) => x.sex === 'female' && x.id !== a?.id).map((x): [string, string] => [String(x.id), x.tag])], value: a?.dam_id ? String(a.dam_id) : '', pair: true },
      { name: 'sire', label: 'Father (sire)', type: 'select', options: [['', '—'], ...active.filter((x) => x.sex === 'male' && x.id !== a?.id).map((x): [string, string] => [String(x.id), x.tag])], value: a?.sire_id ? String(a.sire_id) : '' },
      { name: 'notes', label: 'Notes', type: 'textarea', value: a?.notes ?? '' },
    ], done: 'Animal saved', submit: async (v) => {
      await call('save_animal', { input: { id: a?.id ?? null, flock_id: f.id, tag: v.tag, name: v.name, sex: v.sex, breed: v.breed, birth_date: v.birth, acquired_date: v.acquired, dam_id: v.dam ? Number(v.dam) : null, sire_id: v.sire ? Number(v.sire) : null, notes: v.notes } });
      await draw();
    } });
    box.onclick = (e) => {
      const el = e.target as HTMLElement;
      if (el.closest('[data-add]')) form(null);
      const ed = el.closest<HTMLElement>('[data-edit]');
      if (ed) form(list.find((x) => x.id === Number(ed.dataset.edit)) ?? null);
      const w = el.closest<HTMLElement>('[data-weigh]');
      if (w) formModal({ title: 'Record a weight', fields: [{ name: 'date', label: 'Date', type: 'date', value: today(), required: true, pair: true }, { name: 'kg', label: 'Weight (kg)', type: 'number', required: true }], done: 'Weight saved', submit: async (v) => {
        await call('add_animal_weight', { animalId: Number(w.dataset.weigh), date: v.date, weightKg: num(v.kg) ?? 0 });
        await draw();
      } });
      const o = el.closest<HTMLElement>('[data-out]');
      if (o) {
        const seller = auth.canSell();
        formModal({ title: 'Animal leaves the batch', intro: 'The batch count goes down by one and the daily record or sale is written for you.', fields: [
          { name: 'status', label: 'What happened', type: 'select', options: [...(seller ? [['sold', 'Sold'] as [string, string]] : []), ['dead', 'Died'], ['culled', 'Culled']], pair: true },
          { name: 'date', label: 'Date', type: 'date', value: today(), required: true },
          ...(seller ? [{ name: 'amount', label: 'Amount received (if sold)', type: 'money' as const, pair: true }, { name: 'buyer', label: 'Buyer' }] : []),
        ], done: 'Saved', submit: async (v) => {
          await call('set_animal_status', { id: Number(o.dataset.out), status: v.status, date: v.date, amountKobo: v.amount ? toKobo(v.amount) : null, buyer: v.buyer ?? null });
          await draw();
        } });
      }
    };
  };
  await draw().catch((x) => { box.innerHTML = `<p class="form-error">${esc(errText(x))}</p>`; });
}

/** Matings, pregnancies and births with the offspring registered. */
export async function breedingModal(f: Flock, done: () => void) {
  const m = modal({ title: `Breeding · ${f.code}`, subtitle: tl('species', f.species), size: 'lg', onClose: done, body: '<div data-breed class="stack"></div>' });
  const box = $('[data-breed]', m.el)!;
  const draw = async () => {
    const [list, animals] = await Promise.all([call<Breeding[]>('list_breeding', { flockId: f.id }), call<Animal[]>('list_animals', { flockId: f.id, includeGone: false })]);
    const females = animals.filter((a) => a.sex === 'female');
    const males = animals.filter((a) => a.sex === 'male');
    const statusTone: Record<string, 'ok' | 'warn' | 'mute' | 'bad' | 'info'> = { mated: 'info', pregnant: 'warn', delivered: 'ok', not_pregnant: 'mute', lost: 'bad' };
    box.innerHTML = `<div class="row-actions">${f.status === 'active' ? `<button class="btn btn-primary btn-sm" data-mate>${icon('plus', 14)} Record a mating</button><button class="btn btn-ghost btn-sm" data-birth>${icon('plus', 14)} Record a birth</button>` : ''}</div>
      ${females.length ? '' : '<p class="muted small">Register the breeding females (and males) under Animals to link matings and births to them.</p>'}
      ${tableHtml(['Mated', 'Dam', 'Sire', 'Method', 'Due', 'Status', 'Births#', ''], list.map((b) => [esc(dateLabel(b.mating_date)), esc(b.dam_tag || '—'), esc(b.sire_tag || b.sire_text || '—'), esc(b.method), b.expected_due ? esc(dateLabel(b.expected_due)) : '—', pill(b.status.replace('_', ' '), statusTone[b.status] ?? 'mute'), String(b.births),
        !['delivered', 'lost'].includes(b.status) ? `<button class="btn btn-ghost btn-sm" data-edit="${b.id}">Update</button><button class="btn btn-ghost btn-sm" data-birth-for="${b.id}">Birth</button>` : '']), 'No matings recorded yet.')}`;
    const mate = (b: Breeding | null) => formModal({ title: b ? 'Update mating' : 'Record a mating', fields: [
      { name: 'dam', label: 'Female (dam)', type: 'select', options: [['', '—'], ...females.map((a): [string, string] => [String(a.id), a.tag])], value: b?.dam_id ? String(b.dam_id) : '', pair: true },
      { name: 'sire', label: 'Male (sire)', type: 'select', options: [['', 'Not in the register'], ...males.map((a): [string, string] => [String(a.id), a.tag])], value: b?.sire_id ? String(b.sire_id) : '' },
      { name: 'sire_text', label: 'Sire details if not registered', value: b?.sire_text ?? '', placeholder: 'e.g. neighbour\'s buck, AI straw number' },
      { name: 'date', label: 'Mating date', type: 'date', value: b?.mating_date ?? today(), required: true, pair: true },
      { name: 'method', label: 'Method', type: 'select', options: [['natural', 'Natural'], ['artificial', 'Artificial insemination']], value: b?.method ?? 'natural' },
      { name: 'due', label: 'Expected due date', type: 'date', value: b?.expected_due ?? '', help: 'Ask your vet or extension officer for the gestation length of your breed.', pair: true },
      { name: 'status', label: 'Status', type: 'select', options: [['mated', 'Mated'], ['pregnant', 'Confirmed pregnant'], ['not_pregnant', 'Not pregnant'], ['lost', 'Pregnancy lost']], value: b?.status ?? 'mated' },
      { name: 'notes', label: 'Notes', type: 'textarea', value: b?.notes ?? '' },
    ], done: 'Saved', submit: async (v) => {
      await call('save_breeding', { input: { id: b?.id ?? null, flock_id: f.id, dam_id: v.dam ? Number(v.dam) : null, sire_id: v.sire ? Number(v.sire) : null, sire_text: v.sire_text, mating_date: v.date, method: v.method, expected_due: v.due, status: v.status, notes: v.notes } });
      await draw();
    } });
    const birth = (b: Breeding | null) => formModal({ title: 'Record a birth', intro: 'Live births are added to the batch count. Give a tag prefix to register each newborn in the animal register.', fields: [
      { name: 'date', label: 'Date of birth', type: 'date', value: today(), required: true, pair: true },
      { name: 'dam', label: 'Mother', type: 'select', options: [['', '—'], ...females.map((a): [string, string] => [String(a.id), a.tag])], value: b?.dam_id ? String(b.dam_id) : '' },
      { name: 'alive', label: 'Born alive', type: 'number', required: true, pair: true },
      { name: 'still', label: 'Stillborn', type: 'number', value: 0 },
      { name: 'prefix', label: 'Tag prefix for newborns (optional)', placeholder: 'e.g. K26' },
      { name: 'notes', label: 'Notes', type: 'textarea' },
    ], done: 'Birth recorded', submit: async (v) => {
      await call('record_birth', { input: { flock_id: f.id, breeding_id: b?.id ?? null, dam_id: v.dam ? Number(v.dam) : null, birth_date: v.date, born_alive: Math.round(num(v.alive) ?? 0), stillborn: Math.round(num(v.still) ?? 0), notes: v.notes, tag_prefix: v.prefix } });
      await draw();
    } });
    box.onclick = (e) => {
      const el = e.target as HTMLElement;
      if (el.closest('[data-mate]')) mate(null);
      if (el.closest('[data-birth]')) birth(null);
      const ed = el.closest<HTMLElement>('[data-edit]');
      if (ed) mate(list.find((x) => x.id === Number(ed.dataset.edit)) ?? null);
      const bf = el.closest<HTMLElement>('[data-birth-for]');
      if (bf) birth(list.find((x) => x.id === Number(bf.dataset.birthFor)) ?? null);
    };
  };
  await draw().catch((x) => { box.innerHTML = `<p class="form-error">${esc(errText(x))}</p>`; });
}

/** Fish ponds: register, which batches are stocked, water-quality readings. */
export async function pondsModal(done: () => void) {
  const m = modal({ title: 'Fish ponds', subtitle: 'Ponds, tanks and cages, with water records', size: 'lg', onClose: done, body: '<div data-ponds class="stack"></div>' });
  const box = $('[data-ponds]', m.el)!;
  const manager = auth.can('manager');
  const draw = async () => {
    const ponds = await call<Pond[]>('list_ponds', { includeArchived: true });
    box.innerHTML = `${manager ? `<div class="row-actions"><button class="btn btn-primary btn-sm" data-add>${icon('plus', 14)} Add pond</button></div>` : ''}
      ${tableHtml(['Pond', 'Type', 'Size', 'Fish stocked#', 'Batches', 'Last water check', ''], ponds.map((p) => [`<b>${esc(p.code)}</b>${p.is_demo ? ` ${pill('Demo', 'warn')}` : ''}${p.status === 'archived' ? ` ${pill('Archived')}` : ''}`, esc(p.pond_type), p.size ? `${fmtNum(p.size)} ${esc(p.size_unit.replace('_', ' '))}` : '—', qty(p.stocked), esc(p.batches.join(', ') || '—'),
        p.last_water_date ? esc(dateLabel(p.last_water_date)) : pill('Never', 'warn'), `<button class="btn btn-ghost btn-sm" data-water="${p.id}">Water</button>${manager ? `<button class="btn btn-ghost btn-sm" data-edit="${p.id}">Edit</button><button class="btn btn-ghost btn-sm" data-arch="${p.id}">${p.status === 'active' ? 'Archive' : 'Restore'}</button>` : ''}`]), 'No ponds yet. Add one, then choose it when you start a fish batch.')}
      <div data-water-box></div>`;
    const form = async (p: Pond | null) => {
      const farms = await call<Farm[]>('list_farms').catch(() => [] as Farm[]);
      formModal({ title: p ? `Edit ${p.code}` : 'Add pond', fields: [
        { name: 'code', label: 'Pond name or number', value: p?.code ?? '', required: true, pair: true },
        { name: 'type', label: 'Type', value: p?.pond_type ?? '', list: ['Earthen', 'Concrete', 'Tarpaulin', 'Plastic tank', 'Cage', 'Recirculating'] },
        { name: 'size', label: 'Size', type: 'number', value: p?.size ?? '', pair: true },
        { name: 'unit', label: 'Unit', type: 'select', options: [['square_metre', 'Square metres'], ['cubic_metre', 'Cubic metres'], ['litre', 'Litres']], value: p?.size_unit ?? 'square_metre' },
        { name: 'farm', label: 'Farm', type: 'select', options: [['', '—'], ...farms.map((x): [string, string] => [String(x.id), x.name])], value: p?.farm_id ? String(p.farm_id) : '' },
        { name: 'notes', label: 'Notes', type: 'textarea', value: p?.notes ?? '' },
      ], done: 'Pond saved', submit: async (v) => {
        await call('save_pond', { input: { id: p?.id ?? null, farm_id: v.farm ? Number(v.farm) : null, code: v.code, pond_type: v.type, size: num(v.size) ?? 0, size_unit: v.unit, notes: v.notes } });
        await draw();
      } });
    };
    const water = async (p: Pond) => {
      const list = await call<Water[]>('list_water_records', { pondId: p.id });
      const wb = $('[data-water-box]', box)!;
      const v = (x: number | null, unit = '') => (x === null ? '<span class="muted">not measured</span>' : `${fmtNum(x, 2)}${unit}`);
      wb.innerHTML = `<section class="panel"><header class="panel-head"><h2>Water: ${esc(p.code)}</h2><button class="btn btn-primary btn-sm" data-add-water="${p.id}">${icon('plus', 14)} Add reading</button></header>
        <p class="muted small">Only what you measured is saved. An empty reading is shown as "not measured", never as safe. Ask a fisheries officer what ranges suit your fish.</p>
        ${tableHtml(['Date', 'Temp#', 'pH#', 'Oxygen (mg/L)#', 'Ammonia (mg/L)#', 'Notes', 'By'], list.map((w) => [esc(dateLabel(w.record_date)), v(w.temp_c, ' °C'), v(w.ph), v(w.dissolved_oxygen), v(w.ammonia), esc(w.notes), esc(w.user_name)]), 'No readings yet.')}</section>`;
    };
    box.onclick = async (e) => {
      const el = e.target as HTMLElement;
      if (el.closest('[data-add]')) void form(null);
      const ed = el.closest<HTMLElement>('[data-edit]');
      if (ed) void form(ponds.find((x) => x.id === Number(ed.dataset.edit)) ?? null);
      const w = el.closest<HTMLElement>('[data-water]');
      if (w) void water(ponds.find((x) => x.id === Number(w.dataset.water))!);
      const aw = el.closest<HTMLElement>('[data-add-water]');
      if (aw) {
        const p = ponds.find((x) => x.id === Number(aw.dataset.addWater))!;
        formModal({ title: `Water reading · ${p.code}`, intro: 'Leave a box empty if you did not measure it.', fields: [
          { name: 'date', label: 'Date', type: 'date', value: today(), required: true, pair: true },
          { name: 'temp', label: 'Temperature (°C)', type: 'number' },
          { name: 'ph', label: 'pH', type: 'number', pair: true },
          { name: 'do', label: 'Dissolved oxygen (mg/L)', type: 'number' },
          { name: 'nh3', label: 'Ammonia (mg/L)', type: 'number' },
          { name: 'notes', label: 'Colour, smell, fish behaviour', type: 'textarea' },
        ], done: 'Reading saved', submit: async (v) => {
          await call('add_water_record', { input: { pond_id: p.id, record_date: v.date, temp_c: num(v.temp), ph: num(v.ph), dissolved_oxygen: num(v.do), ammonia: num(v.nh3), notes: v.notes } });
          await draw(); await water(p);
        } });
      }
      const ar = el.closest<HTMLElement>('[data-arch]');
      if (ar) {
        const p = ponds.find((x) => x.id === Number(ar.dataset.arch))!;
        if (p.status === 'active' && await confirmDialog({ title: `Archive ${p.code}?`, message: 'Records are kept. You can restore it later.', confirm: 'Archive' }) === null) return;
        try { await call('archive_pond', { id: p.id, restore: p.status !== 'active' }); await draw(); } catch (x) { toast(errText(x), 'error'); }
      }
    };
  };
  await draw().catch((x) => { box.innerHTML = `<p class="form-error">${esc(errText(x))}</p>`; });
}

export const eggLine = (f: Flock) => `${qty(f.egg_stock)} eggs in stock · laying ${fmtNum(f.laying_pct)}% · sold ${qty(f.eggs_sold)}${f.egg_revenue_kobo && auth.can('manager') ? ` (${money(f.egg_revenue_kobo, { whole: true })})` : ''} · used ${qty(f.eggs_used)} · lost ${qty(f.eggs_lost)}`;
