-- FarmBuddy 1.0.5: runs fill the barn, sales empty it.
-- Runs are processing runs (palm oil, garri, fufu, flour) or harvest runs
-- (cucumber, pepper, vegetables, melon, water yam, yam). Each sale is recorded
-- with simple questions, makes a receipt and counts against the run it came
-- from. Selling measures and their sizes are the farmer's own.
ALTER TABLE palm_runs ADD COLUMN kind TEXT NOT NULL DEFAULT 'process';
ALTER TABLE palm_runs ADD COLUMN stock_unit TEXT NOT NULL DEFAULT '';
ALTER TABLE palm_runs ADD COLUMN second_stock_unit TEXT NOT NULL DEFAULT '';

-- How the farmer sells each product: "1 bag = 120 cups", price of one cup now.
CREATE TABLE IF NOT EXISTS sell_measures (
  product TEXT NOT NULL,
  unit TEXT NOT NULL COLLATE NOCASE,
  -- How many of this measure make one stock unit (the stock unit itself is 1).
  per_stock_unit REAL NOT NULL CHECK(per_stock_unit > 0),
  price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(price_kobo >= 0),
  is_stock_unit INTEGER NOT NULL DEFAULT 0 CHECK(is_stock_unit IN (0,1)),
  sort_order INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (product, unit)
);

CREATE TABLE IF NOT EXISTS run_sales (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL REFERENCES palm_runs(id) ON DELETE RESTRICT,
  part TEXT NOT NULL DEFAULT 'main' CHECK(part IN ('main','second')),
  sale_date TEXT NOT NULL,
  unit TEXT NOT NULL,
  quantity REAL NOT NULL CHECK(quantity > 0),
  -- The same amount in the run's stock unit (bags, kegs…).
  stock_qty REAL NOT NULL CHECK(stock_qty > 0),
  price_kobo INTEGER NOT NULL CHECK(price_kobo >= 0),
  amount_kobo INTEGER NOT NULL CHECK(amount_kobo >= 0),
  transport_kobo INTEGER NOT NULL DEFAULT 0 CHECK(transport_kobo >= 0),
  other_kobo INTEGER NOT NULL DEFAULT 0 CHECK(other_kobo >= 0),
  buyer TEXT NOT NULL DEFAULT '',
  receipt_id INTEGER REFERENCES receipts(id) ON DELETE SET NULL,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_run_sales_run ON run_sales(run_id);

-- 1.0.4 recorded sales as lines inside the run; carry them over as sales.
INSERT INTO run_sales(run_id, part, sale_date, unit, quantity, stock_qty, price_kobo, amount_kobo, user_name)
  SELECT run_id, CASE category WHEN 'oil' THEN 'main' ELSE 'second' END, entry_date, unit, quantity, quantity, unit_price_kobo, amount_kobo, user_name
  FROM palm_run_lines WHERE line_type = 'sale' AND quantity > 0;
DELETE FROM palm_run_lines WHERE line_type = 'sale';
UPDATE palm_runs SET stock_unit = oil_unit, second_stock_unit = kernel_unit WHERE stock_unit = '';
