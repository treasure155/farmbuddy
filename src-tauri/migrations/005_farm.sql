-- Migration 005 (FarmBuddy 0.1): farms, livestock beyond broilers, health and
-- vaccination records, crop plots and cycles, harvests, processing batches,
-- the offline knowledge library, farm reminders and demo-data flags.
-- Runs once with foreign keys off so that stock_movements can be rebuilt with
-- the new farm movement types. Money is always integer kobo.

-- ---------------------------------------------------------------- stock movements: farm types
CREATE TABLE stock_movements_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  movement_type TEXT NOT NULL CHECK(movement_type IN ('opening','stock_in','stock_out','adjustment','sale','void_reversal','return','purchase','recipe','flock_issue','crop_issue','harvest','processing_issue','processing_output')),
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

-- ---------------------------------------------------------------- farms
CREATE TABLE IF NOT EXISTS farms (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT '',
  lga TEXT NOT NULL DEFAULT '',
  community TEXT NOT NULL DEFAULT '',
  address TEXT NOT NULL DEFAULT '',
  phone TEXT NOT NULL DEFAULT '',
  activities TEXT NOT NULL DEFAULT '',          -- comma list: poultry,goats,fish,crops,oil_palm,...
  notes TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived')),
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------- livestock: flocks become general batches
ALTER TABLE flocks ADD COLUMN species TEXT NOT NULL DEFAULT 'broiler';
ALTER TABLE flocks ADD COLUMN purpose TEXT NOT NULL DEFAULT '';
ALTER TABLE flocks ADD COLUMN source TEXT NOT NULL DEFAULT '';
ALTER TABLE flocks ADD COLUMN farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL;
ALTER TABLE flocks ADD COLUMN is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1));
ALTER TABLE flock_logs ADD COLUMN eggs_collected INTEGER NOT NULL DEFAULT 0 CHECK(eggs_collected >= 0);
ALTER TABLE flock_logs ADD COLUMN eggs_damaged INTEGER NOT NULL DEFAULT 0 CHECK(eggs_damaged >= 0);
ALTER TABLE flock_logs ADD COLUMN water_l REAL NOT NULL DEFAULT 0 CHECK(water_l >= 0);
CREATE INDEX IF NOT EXISTS idx_flocks_farm ON flocks(farm_id, status);

CREATE TABLE IF NOT EXISTS health_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  observed_date TEXT NOT NULL,
  issue TEXT NOT NULL,
  symptoms TEXT NOT NULL DEFAULT '',
  affected INTEGER NOT NULL DEFAULT 0 CHECK(affected >= 0),
  action_taken TEXT NOT NULL DEFAULT '',
  vet_consulted TEXT NOT NULL DEFAULT '',
  treatment TEXT NOT NULL DEFAULT '',            -- product name, as prescribed
  dosage_as_prescribed TEXT NOT NULL DEFAULT '',
  route TEXT NOT NULL DEFAULT '',
  start_date TEXT NOT NULL DEFAULT '',
  end_date TEXT NOT NULL DEFAULT '',
  withdrawal_days INTEGER NOT NULL DEFAULT 0 CHECK(withdrawal_days >= 0),
  follow_up_date TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_health_flock ON health_records(flock_id, observed_date);

CREATE TABLE IF NOT EXISTS vaccination_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  due_date TEXT NOT NULL,
  vaccine_name TEXT NOT NULL,
  disease TEXT NOT NULL DEFAULT '',
  manufacturer TEXT NOT NULL DEFAULT '',
  lot_no TEXT NOT NULL DEFAULT '',
  vaccine_expiry TEXT NOT NULL DEFAULT '',
  dose TEXT NOT NULL DEFAULT '',
  route TEXT NOT NULL DEFAULT '',
  -- Where the schedule came from. Only 'vet' and 'label' count as confirmed;
  -- anything taken from a public reference stays 'pending' until a vet confirms.
  source TEXT NOT NULL DEFAULT 'pending' CHECK(source IN ('vet','label','hatchery','pending')),
  reviewed_by TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'planned' CHECK(status IN ('planned','completed','skipped')),
  administered_date TEXT NOT NULL DEFAULT '',
  administered_by TEXT NOT NULL DEFAULT '',
  skip_reason TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_vacc_due ON vaccination_events(status, due_date);

-- ---------------------------------------------------------------- crops
CREATE TABLE IF NOT EXISTS farm_plots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  code TEXT NOT NULL UNIQUE COLLATE NOCASE,
  name TEXT NOT NULL DEFAULT '',
  area REAL NOT NULL DEFAULT 0 CHECK(area >= 0),
  area_unit TEXT NOT NULL DEFAULT 'hectare',
  location TEXT NOT NULL DEFAULT '',
  soil_notes TEXT NOT NULL DEFAULT '',
  irrigation TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived')),
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS crop_cycles (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  plot_id INTEGER NOT NULL REFERENCES farm_plots(id) ON DELETE RESTRICT,
  crop TEXT NOT NULL,
  variety TEXT NOT NULL DEFAULT '',
  planting_date TEXT NOT NULL,
  area_planted REAL NOT NULL DEFAULT 0 CHECK(area_planted >= 0),
  quantity_planted REAL NOT NULL DEFAULT 0 CHECK(quantity_planted >= 0),
  planting_unit TEXT NOT NULL DEFAULT '',
  material_source TEXT NOT NULL DEFAULT '',
  planting_cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(planting_cost_kobo >= 0),
  method TEXT NOT NULL DEFAULT '',
  expected_harvest_from TEXT NOT NULL DEFAULT '',
  expected_harvest_to TEXT NOT NULL DEFAULT '',
  purpose TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'growing' CHECK(status IN ('growing','closed')),
  closed_date TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_crop_cycles_plot ON crop_cycles(plot_id, status);

CREATE TABLE IF NOT EXISTS crop_activities (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  cycle_id INTEGER NOT NULL REFERENCES crop_cycles(id) ON DELETE CASCADE,
  activity_date TEXT NOT NULL,
  activity_type TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  area_done REAL NOT NULL DEFAULT 0 CHECK(area_done >= 0),
  labour_kobo INTEGER NOT NULL DEFAULT 0 CHECK(labour_kobo >= 0),
  other_cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(other_cost_kobo >= 0),
  product_id INTEGER REFERENCES products(id) ON DELETE SET NULL,
  input_qty REAL NOT NULL DEFAULT 0 CHECK(input_qty >= 0),
  input_cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(input_cost_kobo >= 0),
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_crop_activities ON crop_activities(cycle_id, activity_date);

CREATE TABLE IF NOT EXISTS harvest_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  cycle_id INTEGER NOT NULL REFERENCES crop_cycles(id) ON DELETE CASCADE,
  harvest_date TEXT NOT NULL,
  quantity REAL NOT NULL CHECK(quantity > 0),
  unit TEXT NOT NULL,
  grade TEXT NOT NULL DEFAULT '',
  damaged_qty REAL NOT NULL DEFAULT 0 CHECK(damaged_qty >= 0),
  product_id INTEGER REFERENCES products(id) ON DELETE SET NULL,
  stored_qty REAL NOT NULL DEFAULT 0 CHECK(stored_qty >= 0),
  sold_qty REAL NOT NULL DEFAULT 0 CHECK(sold_qty >= 0),
  sale_amount_kobo INTEGER NOT NULL DEFAULT 0 CHECK(sale_amount_kobo >= 0),
  buyer TEXT NOT NULL DEFAULT '',
  labour_kobo INTEGER NOT NULL DEFAULT 0 CHECK(labour_kobo >= 0),
  transport_kobo INTEGER NOT NULL DEFAULT 0 CHECK(transport_kobo >= 0),
  other_cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(other_cost_kobo >= 0),
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_harvest_cycle ON harvest_records(cycle_id, harvest_date);

-- ---------------------------------------------------------------- processing
CREATE TABLE IF NOT EXISTS processing_batches (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_no TEXT NOT NULL UNIQUE COLLATE NOCASE,
  process_type TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  start_date TEXT NOT NULL,
  end_date TEXT NOT NULL DEFAULT '',
  stage TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','closed')),
  notes TEXT NOT NULL DEFAULT '',
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS processing_inputs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id INTEGER NOT NULL REFERENCES processing_batches(id) ON DELETE CASCADE,
  entry_date TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('raw','cost')),
  name TEXT NOT NULL,                             -- raw material, or cost category (Labour, Transport, Milling fee…)
  quantity REAL NOT NULL DEFAULT 0 CHECK(quantity >= 0),
  unit TEXT NOT NULL DEFAULT '',
  unit_cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(unit_cost_kobo >= 0),
  amount_kobo INTEGER NOT NULL CHECK(amount_kobo >= 0),
  product_id INTEGER REFERENCES products(id) ON DELETE SET NULL,
  supplier TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_proc_inputs ON processing_inputs(batch_id);

CREATE TABLE IF NOT EXISTS processing_outputs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id INTEGER NOT NULL REFERENCES processing_batches(id) ON DELETE CASCADE,
  entry_date TEXT NOT NULL,
  output_type TEXT NOT NULL CHECK(output_type IN ('main','byproduct','waste')),
  name TEXT NOT NULL,
  quantity REAL NOT NULL CHECK(quantity >= 0),
  unit TEXT NOT NULL,
  -- Share of the batch cost carried by this main output (percent). Main
  -- outputs must add up to 100 before the batch can close.
  cost_share_pct REAL NOT NULL DEFAULT 0 CHECK(cost_share_pct >= 0 AND cost_share_pct <= 100),
  product_id INTEGER REFERENCES products(id) ON DELETE SET NULL,
  added_to_stock INTEGER NOT NULL DEFAULT 0 CHECK(added_to_stock IN (0,1)),
  sold_qty REAL NOT NULL DEFAULT 0 CHECK(sold_qty >= 0),                -- sold straight from the batch
  sale_amount_kobo INTEGER NOT NULL DEFAULT 0 CHECK(sale_amount_kobo >= 0),
  buyer TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_proc_outputs ON processing_outputs(batch_id);

-- ---------------------------------------------------------------- knowledge library and reminders
CREATE TABLE IF NOT EXISTS knowledge_sources (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  publisher TEXT NOT NULL DEFAULT '',
  url TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS knowledge_articles (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  slug TEXT NOT NULL UNIQUE,
  language TEXT NOT NULL DEFAULT 'en' CHECK(language IN ('en','ib')),
  title TEXT NOT NULL,
  category TEXT NOT NULL,
  applies_to TEXT NOT NULL DEFAULT '',            -- comma list of species / crops / processes
  summary TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL,
  source_id INTEGER REFERENCES knowledge_sources(id) ON DELETE SET NULL,
  source_ref TEXT NOT NULL DEFAULT '',
  author TEXT NOT NULL DEFAULT '',
  review_status TEXT NOT NULL DEFAULT 'pending_review' CHECK(review_status IN ('draft','pending_review','approved')),
  reviewer TEXT NOT NULL DEFAULT '',
  review_date TEXT NOT NULL DEFAULT '',
  version INTEGER NOT NULL DEFAULT 1,
  safety_level TEXT NOT NULL DEFAULT 'general' CHECK(safety_level IN ('general','record_keeping','planning','professional_confirmation','vet_only','chemical_label')),
  is_builtin INTEGER NOT NULL DEFAULT 0 CHECK(is_builtin IN (0,1)),
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS farm_reminders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  title TEXT NOT NULL,
  due_date TEXT NOT NULL,
  related_kind TEXT NOT NULL DEFAULT '',          -- livestock | crop | processing | ''
  related_id INTEGER,
  source TEXT NOT NULL DEFAULT 'manual' CHECK(source IN ('manual','advisory','vaccination')),
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','done','dismissed')),
  notes TEXT NOT NULL DEFAULT '',
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  done_at TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_reminders_due ON farm_reminders(status, due_date);

-- ---------------------------------------------------------------- demo flags on shared records
ALTER TABLE suppliers ADD COLUMN is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1));
ALTER TABLE customers ADD COLUMN is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1));

-- ---------------------------------------------------------------- settings
INSERT OR IGNORE INTO settings(key, value) VALUES
  ('language', 'en'),
  ('onboarding_done', '0'),
  ('onboarding_answers', '{}'),
  ('module_livestock', '1'),
  ('module_crops', '1'),
  ('module_processing', '1'),
  ('module_sales', '1'),
  ('demo_loaded', '0');
-- Every FarmBuddy install tracks livestock batches.
UPDATE settings SET value = '1' WHERE key = 'feature_flocks';
-- FarmBuddy colours (harvest orange actions, forest green navigation) replace
-- the eStock defaults, but never a colour the owner chose.
UPDATE settings SET value = '#E07A1F' WHERE key = 'theme_primary' AND value = '#E8731A';
UPDATE settings SET value = '#1E4D2B' WHERE key = 'theme_secondary' AND value = '#1D4E89';
-- A fresh install inherits the placeholder name from eStock's first migration.
UPDATE settings SET value = 'My Farm' WHERE key = 'business_name' AND value IN ('My Eatery', 'My Business', '');
