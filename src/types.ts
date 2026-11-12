export type Role = 'owner' | 'manager' | 'cashier' | 'worker' | 'adviser';
export type Kind = 'product' | 'service' | 'material';

export interface SessionInfo { token: string; user_id: number; username: string; full_name: string; role: Role }

export interface Branding { business_name: string; business_logo: string; theme_primary: string; theme_secondary: string; theme_mode: string }

export interface TemplateInfo { key: string; name: string; summary: string; categories: string[]; units: string[]; sample_count: number }

export interface Product {
  id: number; sku: string; barcode: string; name: string; category_id: number | null; category_name: string; kind: Kind; unit: string;
  selling_price_kobo: number; cost_price_kobo: number; track_stock: boolean; stock_qty: number; reorder_level: number; description: string; active: boolean;
  pack_unit: string; pack_size: number; pack_price_kobo: number; wholesale_price_kobo: number; wholesale_min_qty: number; track_expiry: boolean;
  has_recipe: boolean; recipe_cost_kobo: number;
}

export interface Category { id: number; name: string; sort_order: number; item_count: number }
export interface PaymentMethod { id: number; name: string; active: boolean; sort_order: number }

export interface Movement {
  id: number; product_id: number; product_name: string; unit: string; movement_type: string; quantity_delta: number;
  unit_cost_kobo: number; note: string; receipt_no: string; user_name: string; created_at: string;
}

export interface ReceiptLine { id: number; returned_qty: number; sold_as: string; product_name: string; product_kind: Kind; unit: string; quantity: number; unit_price_kobo: number; line_total_kobo: number }
export interface ReceiptPayment { method: string; amount_kobo: number; reference: string; received_by: string; created_at: string }
export type PaymentStatus = 'paid' | 'partial' | 'unpaid' | 'void';
export type Fulfilment = 'none' | 'pending' | 'ready' | 'collected';

export interface Receipt {
  id: number; receipt_no: string; customer_id: number | null; customer_name: string; customer_phone: string; customer_email: string; cashier_name: string;
  subtotal_kobo: number; discount_kobo: number; tax_kobo: number; total_kobo: number; amount_paid_kobo: number; change_kobo: number; balance_kobo: number;
  payment_status: PaymentStatus; fulfilment_status: Fulfilment; collection_date: string; note: string; void_reason: string; voided_by: string; created_at: string;
  returned_kobo: number; refunded_kobo: number;
  items: ReceiptLine[]; payments: ReceiptPayment[]; returns: ReturnInfo[];
}
export interface ReturnInfo { return_no: string; value_kobo: number; refund_paid_kobo: number; refund_method: string; reason: string; user_name: string; created_at: string }

export interface ReceiptSummary {
  id: number; receipt_no: string; customer_name: string; customer_email: string; cashier_name: string; total_kobo: number; amount_paid_kobo: number;
  returned_kobo: number; refunded_kobo: number; payment_status: PaymentStatus; fulfilment_status: Fulfilment; collection_date: string; created_at: string; item_count: number;
}

export interface Customer { id: number; name: string; phone: string; email: string; address: string; notes: string; receipt_count: number; total_spent_kobo: number; balance_kobo: number }

export interface UserRow { id: number; username: string; full_name: string; role: Role; active: boolean; last_login_at: string | null; created_at: string }

export interface DayPoint { date: string; total_kobo: number; receipts: number }
export interface Dashboard {
  today_sales_kobo: number; today_receipts: number; today_collected_kobo: number; yesterday_sales_kobo: number; outstanding_kobo: number; owing_receipts: number;
  pending_collections: number; ready_collections: number; product_count: number; low_stock_count: number;
  expiring_count: number; expired_count: number; today_expenses_kobo: number; today_returns_kobo: number;
  low_stock: Array<{ id: number; name: string; unit: string; stock_qty: number; reorder_level: number }>; last_7_days: DayPoint[]; scope: 'all' | 'mine';
}

export interface NamedTotal { name: string; count: number; total_kobo: number }
export interface ItemPerformance { name: string; unit: string; quantity: number; revenue_kobo: number; cost_kobo: number; profit_kobo: number }
export interface SalesReport {
  from: string; to: string; receipts: number; voided: number; gross_kobo: number; discount_kobo: number; tax_kobo: number; net_kobo: number;
  collected_kobo: number; outstanding_kobo: number; cost_kobo: number; profit_kobo: number;
  returns_kobo: number; refunds_kobo: number; expenses_kobo: number; net_profit_kobo: number; by_expense: NamedTotal[];
  by_day: DayPoint[]; by_method: NamedTotal[]; by_cashier: NamedTotal[]; by_category: NamedTotal[]; items: ItemPerformance[];
}

export type Settings = Record<string, string>;

export interface LicenseStatus {
  state: 'licensed' | 'trial' | 'expired'; install_id: string; trial_days_left: number; trial_days: number;
  expires_on: string; licensed_since: string; can_activate: boolean; message: string;
}

export interface MethodTotal { method: string; amount_kobo: number }
export interface ShiftReport {
  id: number; user_name: string; status: 'open' | 'closed'; opened_at: string; closed_at: string | null;
  opening_float_kobo: number; cash_sales_kobo: number; cash_refunds_kobo: number; cash_expenses_kobo: number;
  expected_cash_kobo: number; counted_cash_kobo: number; difference_kobo: number; receipts: number; sales_kobo: number; by_method: MethodTotal[]; note: string;
}
export interface ShiftRow { id: number; user_name: string; status: 'open' | 'closed'; opened_at: string; closed_at: string | null; opening_float_kobo: number; expected_cash_kobo: number; counted_cash_kobo: number; difference_kobo: number }

export interface Expense { id: number; expense_date: string; category: string; description: string; amount_kobo: number; method: string; paid_to: string; user_name: string; created_at: string }
export interface ExpenseList { items: Expense[]; total_kobo: number; by_category: Array<[string, number]>; categories: string[] }

export interface Supplier { id: number; name: string; phone: string; email: string; address: string; notes: string; purchase_count: number; total_bought_kobo: number; owed_kobo: number }
export interface PurchaseLine { product_name: string; unit: string; quantity: number; base_quantity: number; unit_cost_kobo: number; line_total_kobo: number; batch_no: string; expiry_date: string }
export interface Purchase {
  id: number; purchase_no: string; supplier_id: number | null; supplier_name: string; invoice_ref: string; total_kobo: number; amount_paid_kobo: number; balance_kobo: number;
  note: string; user_name: string; created_at: string; lines: PurchaseLine[]; payments: Array<{ amount_kobo: number; method: string; reference: string; user_name: string; created_at: string }>;
}
export interface PurchaseSummary { id: number; purchase_no: string; supplier_name: string; invoice_ref: string; total_kobo: number; amount_paid_kobo: number; balance_kobo: number; item_count: number; user_name: string; created_at: string }

export interface Batch { id: number; product_id: number; product_name: string; unit: string; batch_no: string; expiry_date: string; quantity_left: number; days_left: number }
export interface RecipeLine { material_id: number; material_name: string; unit: string; quantity: number; cost_kobo: number }
export interface HeldSale { id: number; label: string; item_count: number; total_kobo: number; cart_json: string; user_name: string; created_at: string }

export interface StatementLine { date: string; kind: string; reference: string; description: string; debit_kobo: number; credit_kobo: number; balance_kobo: number }
export interface Statement { customer_name: string; customer_phone: string; customer_email: string; customer_address: string; from: string; to: string; opening_kobo: number; closing_kobo: number; lines: StatementLine[] }

export interface Flock {
  id: number; code: string; breed: string; house: string; placed_date: string; chicks_placed: number; chick_cost_kobo: number; status: 'active' | 'closed'; closed_date: string; notes: string;
  species: string; group: 'poultry' | 'livestock' | 'fish'; purpose: string; source: string; farm_id: number | null; farm_name: string; is_demo: boolean;
  age_days: number; dead: number; culled: number; sold: number; alive: number; mortality_pct: number; survival_pct: number; feed_kg: number; water_l: number; eggs: number; eggs_damaged: number;
  latest_weight_kg: number; sold_weight_kg: number; fcr: number; last_log_date: string;
  received: number; transferred_in: number; transferred_out: number; born: number; total_in: number; eggs_sold: number; eggs_used: number; eggs_lost: number; egg_stock: number; egg_revenue_kobo: number; laying_pct: number;
  feed_wasted_kg: number; pond_id: number | null; pond_code: string; animals_registered: number;
  costs_kobo: number; revenue_kobo: number; profit_kobo: number; cost_per_bird_kobo: number; cost_per_kg_kobo: number;
}
export type FlockEntryKind = 'log' | 'cost' | 'sale' | 'receive' | 'transfer_out' | 'transfer_in' | 'eggs' | 'birth';
export interface FlockEntry { id: number; kind: FlockEntryKind; date: string; description: string; quantity: number; amount_kobo: number; mortality: number; culls: number; feed_kg: number; avg_weight_kg: number; eggs: number; eggs_damaged: number; water_l: number; user_name: string }

// ---------------------------------------------------------------- FarmBuddy

export interface Farm { id: number; name: string; state: string; lga: string; community: string; address: string; phone: string; activities: string; notes: string; status: 'active' | 'archived'; is_demo: boolean; created_at: string }

export interface Reminder { id: number; title: string; due_date: string; related_kind: string; related_id: number | null; source: string; status: 'open' | 'done' | 'dismissed'; notes: string; is_demo: boolean; overdue: boolean; user_name: string }

export interface FarmOverview {
  farms: number; active_batches: number; live_animals: number; by_species: Array<{ species: string; batches: number; alive: number }>;
  active_plots: number; growing_cycles: number; harvest_soon: number; open_processing: number; eggs_today: number;
  vaccinations_due: number; vaccinations_overdue: number; vaccinations_need_review: number;
  reminders: Reminder[]; suggestions: number; low_inputs: Array<{ id: number; name: string; unit: string; stock_qty: number; reorder_level: number }>;
  demo_loaded: boolean; onboarding_done: boolean;
  money: { inventory_value_kobo: number; owed_to_suppliers_kobo: number; owed_by_customers_kobo: number; expenses_this_month_kobo: number; sales_this_month_kobo: number; profit_estimate_kobo: number; month_from: string } | null;
}

export interface OnboardingQuestion { id: string; step: number; kind: 'single' | 'multi' | 'text' | 'number' | 'date' | 'state' | 'lga'; prompt: string; help: string; options: Array<[string, string]>; required: boolean }
export interface OnboardingStep { step: number; title: string; questions: OnboardingQuestion[] }
export interface OnboardingResult { farm_id: number; created: string[]; modules: string[] }

export interface HealthRecord {
  id: number; flock_id: number; flock_code: string; observed_date: string; issue: string; symptoms: string; affected: number; action_taken: string; vet_consulted: string;
  treatment: string; dosage_as_prescribed: string; route: string; start_date: string; end_date: string; withdrawal_days: number; withdrawal_until: string; follow_up_date: string; notes: string; user_name: string;
}
export type VaccStatus = 'upcoming' | 'due' | 'overdue' | 'completed' | 'skipped' | 'needs_review';
export interface Vaccination {
  id: number; flock_id: number; flock_code: string; species: string; due_date: string; vaccine_name: string; disease: string; manufacturer: string; lot_no: string; vaccine_expiry: string;
  dose: string; route: string; source: 'vet' | 'label' | 'hatchery' | 'pending'; reviewed_by: string; status: 'planned' | 'completed' | 'skipped'; display_status: VaccStatus;
  administered_date: string; administered_by: string; skip_reason: string; notes: string; user_name: string;
}

export interface Plot { id: number; farm_id: number | null; farm_name: string; code: string; name: string; area: number; area_unit: string; location: string; soil_notes: string; irrigation: string; status: 'active' | 'archived'; is_demo: boolean; current_crop: string; cycles: number; field_id: number | null; field_name: string }
export interface HarvestTotal { unit: string; quantity: number; damaged: number; stored: number; sold: number }
export interface CropCycle {
  id: number; plot_id: number; plot_code: string; plot_name: string; area: number; area_unit: string; crop: string; variety: string; planting_date: string; quantity_planted: number; planting_unit: string;
  material_source: string; planting_cost_kobo: number; method: string; expected_harvest_from: string; expected_harvest_to: string; purpose: string; status: 'growing' | 'closed'; closed_date: string; notes: string; is_demo: boolean;
  days_since_planting: number; activity_count: number; last_activity_date: string; harvest_count: number; activity_cost_kobo: number; harvest_cost_kobo: number; total_cost_kobo: number;
  revenue_kobo: number; margin_kobo: number; harvests: HarvestTotal[]; yield_per_area: number; loss_pct: number;
}
export interface CropActivity { id: number; activity_date: string; activity_type: string; description: string; area_done: number; labour_kobo: number; other_cost_kobo: number; product_id: number | null; product_name: string; input_qty: number; input_unit: string; input_cost_kobo: number; user_name: string }
export interface Harvest { id: number; harvest_date: string; quantity: number; unit: string; grade: string; damaged_qty: number; product_id: number | null; product_name: string; stored_qty: number; sold_qty: number; sale_amount_kobo: number; buyer: string; labour_kobo: number; transport_kobo: number; other_cost_kobo: number; notes: string; user_name: string }

export interface ProcInput { id: number; entry_date: string; kind: 'raw' | 'cost'; name: string; quantity: number; unit: string; unit_cost_kobo: number; amount_kobo: number; product_id: number | null; supplier: string; notes: string; user_name: string }
export interface ProcOutput { id: number; entry_date: string; output_type: 'main' | 'byproduct' | 'waste'; name: string; quantity: number; unit: string; cost_share_pct: number; product_id: number | null; product_name: string; added_to_stock: boolean; sold_qty: number; sale_amount_kobo: number; buyer: string; notes: string; user_name: string; unit_cost_kobo: number; profit_on_sold_kobo: number }
export interface ProcBatch {
  id: number; batch_no: string; process_type: string; title: string; farm_id: number | null; farm_name: string; start_date: string; end_date: string; stage: string; status: 'open' | 'closed'; notes: string; is_demo: boolean;
  raw_cost_kobo: number; processing_cost_kobo: number; total_cost_kobo: number; raw_qty: number; raw_unit: string; main_share_pct: number; output_count: number;
  revenue_kobo: number; profit_on_sold_kobo: number; yield_pct: number | null; waste_pct: number | null; cost_breakdown: Array<[string, number]>;
}

export interface Recommendation { key: string; category: string; title: string; text: string; why: string; safety_level: string; source: string; article: string; priority: number; related_view: string; related_id: number | null }
export interface ArticleSummary {
  id: number; slug: string; language: 'en' | 'ib'; title: string; category: string; applies_to: string; summary: string; source_name: string; source_url: string; source_ref: string; author: string;
  review_status: 'draft' | 'pending_review' | 'approved'; reviewer: string; review_date: string; version: number; safety_level: string; is_builtin: boolean; updated_at: string;
}
export interface Article extends ArticleSummary { body: string; source_id: number | null }
export interface KnowledgeSource { id: number; name: string; publisher: string; url: string }

export interface BackupStatus { auto: boolean; dir: string; default_dir: string; keep: number; last_auto: string; last_error: string; files: Array<{ name: string; size: number; modified: string }> }
