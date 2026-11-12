-- FarmBuddy 1.0.3: the oil palm business (buy fruit or harvest it, process,
-- sell oil and kernel), worked out run by run.
CREATE TABLE IF NOT EXISTS palm_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_no TEXT NOT NULL UNIQUE COLLATE NOCASE,
  title TEXT NOT NULL DEFAULT '',
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  start_date TEXT NOT NULL,
  end_date TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','closed')),
  -- The unit the oil and kernel are counted in (the farmer's own measure).
  oil_unit TEXT NOT NULL DEFAULT '25-litre keg',
  kernel_unit TEXT NOT NULL DEFAULT 'bag',
  -- The farmer's own expected prices, used only to value what is not sold yet.
  expected_oil_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(expected_oil_price_kobo >= 0),
  expected_kernel_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(expected_kernel_price_kobo >= 0),
  notes TEXT NOT NULL DEFAULT '',
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Every money or quantity line of a run.
--   fruit  : fruit bought (category 'bought') or harvested from your own palms ('own')
--   cost   : transport_fruit, processing_fee, labour, transport_market, fuel, water, containers, loading, levy, other
--   output : oil, kernel, byproduct (fibre, shells, sludge…)
--   sale   : oil, kernel, byproduct
CREATE TABLE IF NOT EXISTS palm_run_lines (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  run_id INTEGER NOT NULL REFERENCES palm_runs(id) ON DELETE CASCADE,
  line_type TEXT NOT NULL CHECK(line_type IN ('fruit','cost','output','sale')),
  category TEXT NOT NULL,
  entry_date TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  quantity REAL NOT NULL DEFAULT 0 CHECK(quantity >= 0),
  unit TEXT NOT NULL DEFAULT '',
  unit_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(unit_price_kobo >= 0),
  amount_kobo INTEGER NOT NULL DEFAULT 0 CHECK(amount_kobo >= 0),
  party TEXT NOT NULL DEFAULT '',
  workers INTEGER NOT NULL DEFAULT 0 CHECK(workers >= 0),
  days REAL NOT NULL DEFAULT 0 CHECK(days >= 0),
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_palm_lines ON palm_run_lines(run_id, line_type);

INSERT OR IGNORE INTO settings(key, value) VALUES ('module_palm', '1');
