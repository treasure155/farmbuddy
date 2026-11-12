import { call } from './api';
import type { Category, PaymentMethod, Product, Settings, TemplateInfo } from './types';

/** Shared, cached reference data. Views call the loaders after any change. */
export const store = {
  settings: {} as Settings,
  products: [] as Product[],
  categories: [] as Category[],
  paymentMethods: [] as PaymentMethod[],
  templates: [] as TemplateInfo[],
  /** Farm chosen in the farm switcher; null shows every farm the user may see. */
  farmId: ((): number | null => { try { const v = Number(localStorage.getItem('fb.farm')); return v > 0 ? v : null; } catch { return null; } })(),
  setFarm(id: number | null) { this.farmId = id; try { if (id) localStorage.setItem('fb.farm', String(id)); else localStorage.removeItem('fb.farm'); } catch { /* storage unavailable */ } },
  /** True when a record belongs to the chosen farm (records without a farm always show). */
  inFarm(farmId: number | null | undefined): boolean { return !this.farmId || !farmId || farmId === this.farmId; },

  async loadSettings() { this.settings = await call<Settings>('get_settings'); },
  async loadProducts(includeInactive = false) { this.products = await call<Product[]>('list_products', { includeInactive }); },
  async loadCategories() { this.categories = await call<Category[]>('list_categories'); },
  async loadPaymentMethods() { this.paymentMethods = await call<PaymentMethod[]>('list_payment_methods'); },
  async loadTemplates() { if (!this.templates.length) this.templates = await call<TemplateInfo[]>('list_templates'); },

  label(key: 'item' | 'items' | 'customer'): string {
    const fallback = { item: 'Item', items: 'Items', customer: 'Customer' }[key];
    return this.settings[`label_${key}`] || fallback;
  },
  flag(key: string): boolean { return this.settings[key] === '1'; },
  units(): string[] {
    const t = this.templates.find((x) => x.key === this.settings.business_type);
    const base = t?.units ?? ['piece', 'pack', 'carton', 'kg', 'litre', 'bag', 'crate', 'set', 'service'];
    const used = this.products.map((p) => p.unit);
    return [...new Set([...base, ...used])].sort();
  },
};
