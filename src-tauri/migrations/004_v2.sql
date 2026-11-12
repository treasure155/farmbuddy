-- Migration 004 (v2.0): expenses, cash shifts, suppliers and purchases,
-- pack units and wholesale prices, returns, expiry batches, recipes, held
-- sales and broiler flock tracking. Runs once with foreign keys off so that
-- stock_movements can be rebuilt with the new movement types.

-- ---------------------------------------------------------------- stock movements: new types
CREATE TABLE stock_movements_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  movement_type TEXT NOT NULL CHECK(movement_type IN ('opening','stock_in','stock_out','adjustment','sale','void_reversal','return','purchase','recipe','flock_issue')),
  quantity_delta REAL NOT NULL,
  unit_cost_kobo INTEGER NOT NULL DEFAULT 0,
  note TEXT NOT NULL DEFAULT '',
  receipt_id INTEGER REFERENCES receipts(id) ON DELETE SET NULL,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO stock_movements_new SELECT id, product_id, movement_type, quantity_delta, unit_cost_kobo, note, receipt_id, user_name, created_at FROM stock_movements;
DROP TABLE stock_movements;
ALTER TABLE stock_movements_new RENAME TO stock_movements;
CREATE INDEX IF NOT EXISTS idx_stock_movements_product_id ON stock_movements(product_id);
CREATE INDEX IF NOT EXISTS idx_stock_movements_created_at ON stock_movements(created_at DESC);

-- ---------------------------------------------------------------- products: packs, wholesale, expiry
ALTER TABLE products ADD COLUMN pack_unit TEXT NOT NULL DEFAULT '';
ALTER TABLE products ADD COLUMN pack_size REAL NOT NULL DEFAULT 0 CHECK(pack_size >= 0);
ALTER TABLE products ADD COLUMN pack_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(pack_price_kobo >= 0);
ALTER TABLE products ADD COLUMN wholesale_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(wholesale_price_kobo >= 0);
ALTER TABLE products ADD COLUMN wholesale_min_qty REAL NOT NULL DEFAULT 0 CHECK(wholesale_min_qty >= 0);
ALTER TABLE products ADD COLUMN track_expiry INTEGER NOT NULL DEFAULT 0 CHECK(track_expiry IN (0,1));

-- ---------------------------------------------------------------- receipts: returns and shifts
ALTER TABLE receipts ADD COLUMN returned_kobo INTEGER NOT NULL DEFAULT 0 CHECK(returned_kobo >= 0);
ALTER TABLE receipts ADD COLUMN refunded_kobo INTEGER NOT NULL DEFAULT 0 CHECK(refunded_kobo >= 0);
ALTER TABLE receipt_items ADD COLUMN base_quantity REAL NOT NULL DEFAULT 0;
ALTER TABLE receipt_items ADD COLUMN sold_as TEXT NOT NULL DEFAULT '';
ALTER TABLE receipt_items ADD COLUMN returned_qty REAL NOT NULL DEFAULT 0 CHECK(returned_qty >= 0);
UPDATE receipt_items SET base_quantity = quantity WHERE base_quantity = 0;
ALTER TABLE receipt_payments ADD COLUMN shift_id INTEGER REFERENCES shifts(id) ON DELETE SET NULL;

-- ---------------------------------------------------------------- returns
CREATE TABLE IF NOT EXISTS returns (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  return_no TEXT NOT NULL UNIQUE,
  receipt_id INTEGER NOT NULL REFERENCES receipts(id) ON DELETE RESTRICT,
  value_kobo INTEGER NOT NULL CHECK(value_kobo >= 0),
  refund_paid_kobo INTEGER NOT NULL DEFAULT 0 CHECK(refund_paid_kobo >= 0),
  refund_method TEXT NOT NULL DEFAULT '',
  reason TEXT NOT NULL DEFAULT '',
  shift_id INTEGER REFERENCES shifts(id) ON DELETE SET NULL,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS return_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  return_id INTEGER NOT NULL REFERENCES returns(id) ON DELETE CASCADE,
  receipt_item_id INTEGER NOT NULL REFERENCES receipt_items(id) ON DELETE RESTRICT,
  quantity REAL NOT NULL CHECK(quantity > 0),
  value_kobo INTEGER NOT NULL CHECK(value_kobo >= 0),
  cost_kobo INTEGER NOT NULL DEFAULT 0,
  restocked INTEGER NOT NULL DEFAULT 1 CHECK(restocked IN (0,1))
);
CREATE INDEX IF NOT EXISTS idx_returns_receipt ON returns(receipt_id);
CREATE INDEX IF NOT EXISTS idx_returns_created_at ON returns(created_at);

-- ---------------------------------------------------------------- cash shifts
CREATE TABLE IF NOT EXISTS shifts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL REFERENCES users(id),
  user_name TEXT NOT NULL,
  opening_float_kobo INTEGER NOT NULL DEFAULT 0 CHECK(opening_float_kobo >= 0),
  opened_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  closed_at TEXT,
  expected_cash_kobo INTEGER NOT NULL DEFAULT 0,
  counted_cash_kobo INTEGER NOT NULL DEFAULT 0,
  note TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','closed'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_shifts_one_open ON shifts(user_id) WHERE status = 'open';

-- ---------------------------------------------------------------- expenses
CREATE TABLE IF NOT EXISTS expenses (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  expense_date TEXT NOT NULL,
  category TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  amount_kobo INTEGER NOT NULL CHECK(amount_kobo > 0),
  method TEXT NOT NULL DEFAULT '',
  paid_to TEXT NOT NULL DEFAULT '',
  shift_id INTEGER REFERENCES shifts(id) ON DELETE SET NULL,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_expenses_date ON expenses(expense_date);

-- ---------------------------------------------------------------- suppliers and purchases
CREATE TABLE IF NOT EXISTS suppliers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  phone TEXT NOT NULL DEFAULT '',
  email TEXT NOT NULL DEFAULT '',
  address TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS purchases (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  purchase_no TEXT NOT NULL UNIQUE,
  supplier_id INTEGER REFERENCES suppliers(id) ON DELETE SET NULL,
  supplier_name TEXT NOT NULL DEFAULT '',
  invoice_ref TEXT NOT NULL DEFAULT '',
  total_kobo INTEGER NOT NULL CHECK(total_kobo >= 0),
  amount_paid_kobo INTEGER NOT NULL DEFAULT 0 CHECK(amount_paid_kobo >= 0),
  note TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS purchase_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  purchase_id INTEGER NOT NULL REFERENCES purchases(id) ON DELETE CASCADE,
  product_id INTEGER REFERENCES products(id) ON DELETE SET NULL,
  product_name TEXT NOT NULL,
  unit TEXT NOT NULL,
  quantity REAL NOT NULL CHECK(quantity > 0),
  base_quantity REAL NOT NULL,
  unit_cost_kobo INTEGER NOT NULL CHECK(unit_cost_kobo >= 0),
  line_total_kobo INTEGER NOT NULL CHECK(line_total_kobo >= 0),
  batch_no TEXT NOT NULL DEFAULT '',
  expiry_date TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS supplier_payments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  purchase_id INTEGER NOT NULL REFERENCES purchases(id) ON DELETE CASCADE,
  amount_kobo INTEGER NOT NULL CHECK(amount_kobo > 0),
  method TEXT NOT NULL DEFAULT '',
  reference TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_purchases_supplier ON purchases(supplier_id);
CREATE INDEX IF NOT EXISTS idx_purchases_created_at ON purchases(created_at);

-- ---------------------------------------------------------------- expiry batches
CREATE TABLE IF NOT EXISTS stock_batches (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  batch_no TEXT NOT NULL DEFAULT '',
  expiry_date TEXT NOT NULL,
  quantity_received REAL NOT NULL CHECK(quantity_received > 0),
  quantity_left REAL NOT NULL CHECK(quantity_left >= 0),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_batches_product ON stock_batches(product_id, expiry_date);

-- ---------------------------------------------------------------- recipes
CREATE TABLE IF NOT EXISTS recipe_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  material_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  quantity REAL NOT NULL CHECK(quantity > 0),
  UNIQUE(product_id, material_id)
);

-- ---------------------------------------------------------------- held (parked) sales
CREATE TABLE IF NOT EXISTS held_sales (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  label TEXT NOT NULL,
  item_count REAL NOT NULL DEFAULT 0,
  total_kobo INTEGER NOT NULL DEFAULT 0,
  cart_json TEXT NOT NULL,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------- broiler flocks
CREATE TABLE IF NOT EXISTS flocks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  code TEXT NOT NULL UNIQUE COLLATE NOCASE,
  breed TEXT NOT NULL DEFAULT '',
  house TEXT NOT NULL DEFAULT '',
  placed_date TEXT NOT NULL,
  chicks_placed INTEGER NOT NULL CHECK(chicks_placed > 0),
  chick_cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(chick_cost_kobo >= 0),
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','closed')),
  closed_date TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS flock_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  log_date TEXT NOT NULL,
  mortality INTEGER NOT NULL DEFAULT 0 CHECK(mortality >= 0),
  culls INTEGER NOT NULL DEFAULT 0 CHECK(culls >= 0),
  feed_kg REAL NOT NULL DEFAULT 0 CHECK(feed_kg >= 0),
  avg_weight_kg REAL NOT NULL DEFAULT 0 CHECK(avg_weight_kg >= 0),
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS flock_costs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  cost_date TEXT NOT NULL,
  category TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  product_id INTEGER REFERENCES products(id) ON DELETE SET NULL,
  quantity REAL NOT NULL DEFAULT 0,
  amount_kobo INTEGER NOT NULL CHECK(amount_kobo >= 0),
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS flock_sales (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  sale_date TEXT NOT NULL,
  birds INTEGER NOT NULL CHECK(birds > 0),
  live_weight_kg REAL NOT NULL DEFAULT 0 CHECK(live_weight_kg >= 0),
  amount_kobo INTEGER NOT NULL DEFAULT 0 CHECK(amount_kobo >= 0),
  buyer TEXT NOT NULL DEFAULT '',
  receipt_no TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_flock_logs ON flock_logs(flock_id, log_date);

-- ---------------------------------------------------------------- settings
INSERT OR IGNORE INTO settings(key, value) VALUES
  ('auto_backup', '1'),
  ('backup_dir', ''),
  ('backup_keep', '14'),
  ('last_auto_backup', ''),
  ('require_shift', '0'),
  ('feature_flocks', CASE WHEN (SELECT value FROM settings WHERE key='business_type') = 'broiler_farm' THEN '1' ELSE '0' END),
  ('feature_recipes', CASE WHEN (SELECT value FROM settings WHERE key='business_type') = 'eatery' THEN '1' ELSE '0' END),
  ('expiry_warn_days', '30'),
  ('whatsapp_country_code', '234');
