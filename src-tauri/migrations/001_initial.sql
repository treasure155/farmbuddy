PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

INSERT OR IGNORE INTO settings(key, value) VALUES
  ('eatery_name', 'My Eatery'),
  ('eatery_address', ''),
  ('eatery_phone', ''),
  ('receipt_width_mm', '80'),
  ('currency', 'NGN');

CREATE TABLE IF NOT EXISTS products (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  sku TEXT NOT NULL UNIQUE COLLATE NOCASE,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('finished','ingredient')),
  unit TEXT NOT NULL DEFAULT 'piece',
  selling_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(selling_price_kobo >= 0),
  cost_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(cost_price_kobo >= 0),
  stock_qty REAL NOT NULL DEFAULT 0 CHECK(stock_qty >= 0),
  reorder_level REAL NOT NULL DEFAULT 0 CHECK(reorder_level >= 0),
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS stock_movements (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product_id INTEGER NOT NULL REFERENCES products(id),
  movement_type TEXT NOT NULL CHECK(movement_type IN ('opening','stock_in','adjustment','sale','void_reversal')),
  quantity_delta REAL NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  receipt_id INTEGER,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS receipts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  receipt_no TEXT NOT NULL UNIQUE,
  customer_name TEXT NOT NULL DEFAULT '',
  customer_email TEXT NOT NULL DEFAULT '',
  subtotal_kobo INTEGER NOT NULL CHECK(subtotal_kobo >= 0),
  discount_kobo INTEGER NOT NULL DEFAULT 0 CHECK(discount_kobo >= 0),
  total_kobo INTEGER NOT NULL CHECK(total_kobo >= 0),
  payment_status TEXT NOT NULL DEFAULT 'paid' CHECK(payment_status IN ('paid','unpaid','refunded')),
  note TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS receipt_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  receipt_id INTEGER NOT NULL REFERENCES receipts(id) ON DELETE RESTRICT,
  product_id INTEGER REFERENCES products(id),
  product_name TEXT NOT NULL,
  product_kind TEXT NOT NULL CHECK(product_kind IN ('finished','ingredient')),
  unit TEXT NOT NULL,
  quantity REAL NOT NULL CHECK(quantity > 0),
  unit_price_kobo INTEGER NOT NULL CHECK(unit_price_kobo >= 0),
  line_total_kobo INTEGER NOT NULL CHECK(line_total_kobo >= 0)
);

CREATE TABLE IF NOT EXISTS receipt_payments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  receipt_id INTEGER NOT NULL REFERENCES receipts(id) ON DELETE RESTRICT,
  method TEXT NOT NULL CHECK(method IN ('cash','transfer','pos')),
  amount_kobo INTEGER NOT NULL CHECK(amount_kobo > 0),
  reference TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS email_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  receipt_id INTEGER REFERENCES receipts(id),
  recipient TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('queued','sent','failed')),
  message TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_products_name ON products(name);
CREATE INDEX IF NOT EXISTS idx_receipts_created_at ON receipts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_receipt_items_receipt_id ON receipt_items(receipt_id);
CREATE INDEX IF NOT EXISTS idx_stock_movements_product_id ON stock_movements(product_id);
