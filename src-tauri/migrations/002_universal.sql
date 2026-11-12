-- Migration 002: universal multi-business, multi-user schema.
-- Runs once (PRAGMA user_version < 2) inside a transaction with foreign keys
-- temporarily disabled, so tables with changed CHECK constraints can be rebuilt
-- while every existing row from 001 is preserved.

-- ---------------------------------------------------------------- users
CREATE TABLE users_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE COLLATE NOCASE,
  full_name TEXT NOT NULL DEFAULT '',
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'cashier' CHECK(role IN ('owner','manager','cashier')),
  active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)),
  last_login_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO users_new(id, username, full_name, password_hash, role, created_at)
  SELECT id, username, username, password_hash, 'owner', created_at FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

-- ---------------------------------------------------------------- categories
CREATE TABLE IF NOT EXISTS categories (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  sort_order INTEGER NOT NULL DEFAULT 0,
  active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------- products
-- kind: product  = sellable item with stock (goods, meals, birds, produce)
--       service  = sellable, no stock (dry-cleaning a shirt, delivery fee)
--       material = internal stock, not sold (ingredients, feed, detergent)
CREATE TABLE products_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  sku TEXT NOT NULL UNIQUE COLLATE NOCASE,
  barcode TEXT NOT NULL DEFAULT '',
  name TEXT NOT NULL,
  category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK(kind IN ('product','service','material')),
  unit TEXT NOT NULL DEFAULT 'piece',
  selling_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(selling_price_kobo >= 0),
  cost_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(cost_price_kobo >= 0),
  track_stock INTEGER NOT NULL DEFAULT 1 CHECK(track_stock IN (0,1)),
  stock_qty REAL NOT NULL DEFAULT 0,
  reorder_level REAL NOT NULL DEFAULT 0 CHECK(reorder_level >= 0),
  description TEXT NOT NULL DEFAULT '',
  active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO products_new(id, sku, name, kind, unit, selling_price_kobo, cost_price_kobo, stock_qty, reorder_level, active, created_at, updated_at)
  SELECT id, sku, name, CASE kind WHEN 'finished' THEN 'product' ELSE 'material' END,
         unit, selling_price_kobo, cost_price_kobo, stock_qty, reorder_level, active, created_at, updated_at
  FROM products;
DROP TABLE products;
ALTER TABLE products_new RENAME TO products;

-- ---------------------------------------------------------------- customers
CREATE TABLE IF NOT EXISTS customers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  phone TEXT NOT NULL DEFAULT '',
  email TEXT NOT NULL DEFAULT '',
  address TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------- payment methods
CREATE TABLE IF NOT EXISTS payment_methods (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)),
  sort_order INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO payment_methods(name, sort_order) VALUES
  ('Cash', 1), ('Bank transfer', 2), ('POS / card', 3);

-- ---------------------------------------------------------------- receipts
CREATE TABLE receipts_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  receipt_no TEXT NOT NULL UNIQUE,
  customer_id INTEGER REFERENCES customers(id) ON DELETE SET NULL,
  customer_name TEXT NOT NULL DEFAULT '',
  customer_phone TEXT NOT NULL DEFAULT '',
  customer_email TEXT NOT NULL DEFAULT '',
  cashier_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
  cashier_name TEXT NOT NULL DEFAULT '',
  subtotal_kobo INTEGER NOT NULL CHECK(subtotal_kobo >= 0),
  discount_kobo INTEGER NOT NULL DEFAULT 0 CHECK(discount_kobo >= 0),
  tax_kobo INTEGER NOT NULL DEFAULT 0 CHECK(tax_kobo >= 0),
  total_kobo INTEGER NOT NULL CHECK(total_kobo >= 0),
  amount_paid_kobo INTEGER NOT NULL DEFAULT 0 CHECK(amount_paid_kobo >= 0),
  change_kobo INTEGER NOT NULL DEFAULT 0 CHECK(change_kobo >= 0),
  payment_status TEXT NOT NULL DEFAULT 'paid' CHECK(payment_status IN ('paid','partial','unpaid','void')),
  fulfilment_status TEXT NOT NULL DEFAULT 'none' CHECK(fulfilment_status IN ('none','pending','ready','collected')),
  collection_date TEXT NOT NULL DEFAULT '',
  note TEXT NOT NULL DEFAULT '',
  void_reason TEXT NOT NULL DEFAULT '',
  voided_by TEXT NOT NULL DEFAULT '',
  voided_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO receipts_new(id, receipt_no, customer_name, customer_email, subtotal_kobo, discount_kobo, total_kobo,
                         amount_paid_kobo, payment_status, note, created_at)
  SELECT id, receipt_no, customer_name, customer_email, subtotal_kobo, discount_kobo, total_kobo,
         CASE payment_status WHEN 'paid' THEN total_kobo ELSE 0 END,
         CASE payment_status WHEN 'paid' THEN 'paid' WHEN 'unpaid' THEN 'unpaid' ELSE 'void' END,
         note, created_at
  FROM receipts;
DROP TABLE receipts;
ALTER TABLE receipts_new RENAME TO receipts;

CREATE TABLE receipt_items_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  receipt_id INTEGER NOT NULL REFERENCES receipts(id) ON DELETE RESTRICT,
  product_id INTEGER REFERENCES products(id) ON DELETE SET NULL,
  product_name TEXT NOT NULL,
  product_kind TEXT NOT NULL CHECK(product_kind IN ('product','service','material')),
  unit TEXT NOT NULL,
  quantity REAL NOT NULL CHECK(quantity > 0),
  unit_price_kobo INTEGER NOT NULL CHECK(unit_price_kobo >= 0),
  unit_cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(unit_cost_kobo >= 0),
  line_total_kobo INTEGER NOT NULL CHECK(line_total_kobo >= 0),
  stock_deducted INTEGER NOT NULL DEFAULT 1 CHECK(stock_deducted IN (0,1))
);
INSERT INTO receipt_items_new(id, receipt_id, product_id, product_name, product_kind, unit, quantity, unit_price_kobo, line_total_kobo)
  SELECT id, receipt_id, product_id, product_name,
         CASE product_kind WHEN 'finished' THEN 'product' ELSE 'material' END,
         unit, quantity, unit_price_kobo, line_total_kobo
  FROM receipt_items;
DROP TABLE receipt_items;
ALTER TABLE receipt_items_new RENAME TO receipt_items;

CREATE TABLE receipt_payments_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  receipt_id INTEGER NOT NULL REFERENCES receipts(id) ON DELETE RESTRICT,
  method TEXT NOT NULL,
  amount_kobo INTEGER NOT NULL CHECK(amount_kobo > 0),
  reference TEXT NOT NULL DEFAULT '',
  received_by TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO receipt_payments_new(id, receipt_id, method, amount_kobo, reference)
  SELECT id, receipt_id,
         CASE method WHEN 'cash' THEN 'Cash' WHEN 'transfer' THEN 'Bank transfer' WHEN 'pos' THEN 'POS / card' ELSE method END,
         amount_kobo, reference
  FROM receipt_payments;
DROP TABLE receipt_payments;
ALTER TABLE receipt_payments_new RENAME TO receipt_payments;

-- ---------------------------------------------------------------- stock movements
CREATE TABLE stock_movements_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  movement_type TEXT NOT NULL CHECK(movement_type IN ('opening','stock_in','stock_out','adjustment','sale','void_reversal')),
  quantity_delta REAL NOT NULL,
  unit_cost_kobo INTEGER NOT NULL DEFAULT 0,
  note TEXT NOT NULL DEFAULT '',
  receipt_id INTEGER REFERENCES receipts(id) ON DELETE SET NULL,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO stock_movements_new(id, product_id, movement_type, quantity_delta, note, receipt_id, created_at)
  SELECT id, product_id, movement_type, quantity_delta, note, receipt_id, created_at FROM stock_movements;
DROP TABLE stock_movements;
ALTER TABLE stock_movements_new RENAME TO stock_movements;

-- ---------------------------------------------------------------- settings
-- Carry the eatery-era keys over to the universal names, then seed defaults.
INSERT OR IGNORE INTO settings(key, value) SELECT 'business_name', value FROM settings WHERE key = 'eatery_name';
INSERT OR IGNORE INTO settings(key, value) SELECT 'business_address', value FROM settings WHERE key = 'eatery_address';
INSERT OR IGNORE INTO settings(key, value) SELECT 'business_phone', value FROM settings WHERE key = 'eatery_phone';
INSERT OR IGNORE INTO settings(key, value) SELECT 'smtp_from_email', value FROM settings WHERE key = 'smtp_from';
INSERT OR IGNORE INTO settings(key, value) SELECT 'business_type', 'eatery' FROM settings WHERE key = 'eatery_name' AND (SELECT COUNT(*) FROM users) > 0;
DELETE FROM settings WHERE key IN ('eatery_name','eatery_address','eatery_phone','smtp_from');

INSERT OR IGNORE INTO settings(key, value) VALUES
  ('business_name', 'My Business'),
  ('business_address', ''),
  ('business_phone', ''),
  ('business_email', ''),
  ('business_type', 'general'),
  ('business_logo', ''),
  ('currency_symbol', '₦'),
  ('receipt_width_mm', '80'),
  ('receipt_title', 'Sales receipt'),
  ('receipt_footer', 'Thank you for your patronage.'),
  ('receipt_prefix', 'RCP'),
  ('tax_enabled', '0'),
  ('tax_label', 'VAT'),
  ('tax_rate', '7.5'),
  ('allow_negative_stock', '0'),
  ('allow_price_override', '0'),
  ('track_collection', '0'),
  ('label_item', 'Item'),
  ('label_items', 'Items'),
  ('label_customer', 'Customer'),
  ('theme_primary', '#E8731A'),
  ('theme_secondary', '#1D4E89'),
  ('theme_mode', 'light'),
  ('smtp_provider', 'gmail'),
  ('smtp_host', 'smtp.gmail.com'),
  ('smtp_port', '587'),
  ('smtp_security', 'starttls'),
  ('smtp_username', ''),
  ('smtp_password', ''),
  ('smtp_from_name', ''),
  ('smtp_from_email', ''),
  ('smtp_reply_to', ''),
  ('email_bcc_self', '0');

-- ---------------------------------------------------------------- indexes
CREATE INDEX IF NOT EXISTS idx_products_name ON products(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_products_category ON products(category_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_products_barcode ON products(barcode) WHERE barcode <> '';
CREATE INDEX IF NOT EXISTS idx_customers_name ON customers(name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_customers_phone ON customers(phone);
CREATE INDEX IF NOT EXISTS idx_receipts_created_at ON receipts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_receipts_status ON receipts(payment_status);
CREATE INDEX IF NOT EXISTS idx_receipts_fulfilment ON receipts(fulfilment_status);
CREATE INDEX IF NOT EXISTS idx_receipts_customer ON receipts(customer_id);
CREATE INDEX IF NOT EXISTS idx_receipts_cashier ON receipts(cashier_id);
CREATE INDEX IF NOT EXISTS idx_receipt_items_receipt_id ON receipt_items(receipt_id);
CREATE INDEX IF NOT EXISTS idx_receipt_items_product_id ON receipt_items(product_id);
CREATE INDEX IF NOT EXISTS idx_receipt_payments_receipt_id ON receipt_payments(receipt_id);
CREATE INDEX IF NOT EXISTS idx_stock_movements_product_id ON stock_movements(product_id);
CREATE INDEX IF NOT EXISTS idx_stock_movements_created_at ON stock_movements(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_email_logs_receipt_id ON email_logs(receipt_id);
