-- Migration 006 (FarmBuddy 1.0): the rest of the specification.
-- Individual animals, breeding and births, batch transfers, egg stock, fish
-- ponds and water quality, feed guides and formulations, vaccination
-- schedules, crop health, varieties and fields, storage locations, weather,
-- climate outlooks and market prices, configurable advisory rules, advisory
-- history, content packages, audit log, farm assignments, worker and adviser
-- roles, and demo flags on commerce records. Runs with foreign keys off so the
-- users table can be rebuilt with the new roles.

-- ---------------------------------------------------------------- roles: farm worker and adviser
CREATE TABLE users_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE COLLATE NOCASE,
  full_name TEXT NOT NULL DEFAULT '',
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'cashier' CHECK(role IN ('owner','manager','cashier','worker','adviser')),
  active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)),
  last_login_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO users_new(id, username, full_name, password_hash, role, active, last_login_at, created_at, updated_at)
  SELECT id, username, full_name, password_hash, role, active, last_login_at, created_at, updated_at FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

CREATE TABLE IF NOT EXISTS user_farms (
  user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  farm_id INTEGER NOT NULL REFERENCES farms(id) ON DELETE CASCADE,
  PRIMARY KEY (user_id, farm_id)
);

CREATE TABLE IF NOT EXISTS audit_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  user_name TEXT NOT NULL DEFAULT '',
  action TEXT NOT NULL,
  entity TEXT NOT NULL DEFAULT '',
  entity_id INTEGER,
  details TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_audit_at ON audit_logs(at DESC);

-- ---------------------------------------------------------------- livestock
CREATE TABLE IF NOT EXISTS fish_ponds (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  code TEXT NOT NULL UNIQUE COLLATE NOCASE,
  pond_type TEXT NOT NULL DEFAULT '',
  size REAL NOT NULL DEFAULT 0 CHECK(size >= 0),
  size_unit TEXT NOT NULL DEFAULT 'square_metre',
  notes TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived')),
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
ALTER TABLE flocks ADD COLUMN pond_id INTEGER REFERENCES fish_ponds(id) ON DELETE SET NULL;
ALTER TABLE flock_logs ADD COLUMN feed_wasted_kg REAL NOT NULL DEFAULT 0 CHECK(feed_wasted_kg >= 0);

CREATE TABLE IF NOT EXISTS water_quality_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  pond_id INTEGER NOT NULL REFERENCES fish_ponds(id) ON DELETE CASCADE,
  record_date TEXT NOT NULL,
  -- Only what was actually measured is stored; a missing reading stays NULL and is never treated as safe.
  temp_c REAL, ph REAL, dissolved_oxygen REAL, ammonia REAL,
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS flock_transfers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  from_flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  to_flock_id INTEGER REFERENCES flocks(id) ON DELETE CASCADE,
  transfer_date TEXT NOT NULL,
  quantity INTEGER NOT NULL CHECK(quantity > 0),
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
-- Animals bought or received into an existing batch (to_flock_id) with no source batch.
CREATE TABLE IF NOT EXISTS flock_receipts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  receipt_date TEXT NOT NULL,
  quantity INTEGER NOT NULL CHECK(quantity > 0),
  cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(cost_kobo >= 0),
  source TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS egg_movements (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  move_date TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('sold','used','lost')),
  eggs INTEGER NOT NULL CHECK(eggs > 0),
  amount_kobo INTEGER NOT NULL DEFAULT 0 CHECK(amount_kobo >= 0),
  buyer TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS livestock_animals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  tag TEXT NOT NULL UNIQUE COLLATE NOCASE,
  name TEXT NOT NULL DEFAULT '',
  sex TEXT NOT NULL DEFAULT 'unknown' CHECK(sex IN ('female','male','unknown')),
  breed TEXT NOT NULL DEFAULT '',
  birth_date TEXT NOT NULL DEFAULT '',
  acquired_date TEXT NOT NULL DEFAULT '',
  dam_id INTEGER REFERENCES livestock_animals(id) ON DELETE SET NULL,
  sire_id INTEGER REFERENCES livestock_animals(id) ON DELETE SET NULL,
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','sold','dead','culled')),
  status_date TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS animal_weights (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  animal_id INTEGER NOT NULL REFERENCES livestock_animals(id) ON DELETE CASCADE,
  weigh_date TEXT NOT NULL,
  weight_kg REAL NOT NULL CHECK(weight_kg > 0),
  user_name TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS breeding_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  dam_id INTEGER REFERENCES livestock_animals(id) ON DELETE SET NULL,
  sire_id INTEGER REFERENCES livestock_animals(id) ON DELETE SET NULL,
  sire_text TEXT NOT NULL DEFAULT '',
  mating_date TEXT NOT NULL,
  method TEXT NOT NULL DEFAULT 'natural' CHECK(method IN ('natural','artificial')),
  expected_due TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'mated' CHECK(status IN ('mated','pregnant','not_pregnant','delivered','lost')),
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS birth_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flock_id INTEGER NOT NULL REFERENCES flocks(id) ON DELETE CASCADE,
  breeding_id INTEGER REFERENCES breeding_records(id) ON DELETE SET NULL,
  dam_id INTEGER REFERENCES livestock_animals(id) ON DELETE SET NULL,
  birth_date TEXT NOT NULL,
  born_alive INTEGER NOT NULL DEFAULT 0 CHECK(born_alive >= 0),
  stillborn INTEGER NOT NULL DEFAULT 0 CHECK(stillborn >= 0),
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------- feed
CREATE TABLE IF NOT EXISTS feed_guides (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  species TEXT NOT NULL,
  age_from_day INTEGER NOT NULL CHECK(age_from_day >= 0),
  age_to_day INTEGER NOT NULL CHECK(age_to_day >= age_from_day),
  grams_per_animal_day REAL NOT NULL CHECK(grams_per_animal_day > 0),
  source TEXT NOT NULL,
  review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','approved')),
  reviewer TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS feed_formulations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  species TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS feed_formulation_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  formulation_id INTEGER NOT NULL REFERENCES feed_formulations(id) ON DELETE CASCADE,
  ingredient TEXT NOT NULL,
  quantity_kg REAL NOT NULL CHECK(quantity_kg > 0),
  unit_cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(unit_cost_kobo >= 0),
  protein_pct REAL,
  energy_kcal_kg REAL
);

-- ---------------------------------------------------------------- vaccination schedules
CREATE TABLE IF NOT EXISTS vaccination_protocols (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  species TEXT NOT NULL,
  source_text TEXT NOT NULL DEFAULT '',
  review_status TEXT NOT NULL DEFAULT 'pending' CHECK(review_status IN ('pending','approved')),
  reviewer TEXT NOT NULL DEFAULT '',
  review_date TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS vaccination_protocol_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  protocol_id INTEGER NOT NULL REFERENCES vaccination_protocols(id) ON DELETE CASCADE,
  day_offset INTEGER NOT NULL CHECK(day_offset >= 0),
  vaccine_name TEXT NOT NULL,
  disease TEXT NOT NULL DEFAULT '',
  route TEXT NOT NULL DEFAULT '',
  dose TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT ''
);

-- ---------------------------------------------------------------- crops
CREATE TABLE IF NOT EXISTS farm_fields (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  area REAL NOT NULL DEFAULT 0 CHECK(area >= 0),
  area_unit TEXT NOT NULL DEFAULT 'hectare',
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
ALTER TABLE farm_plots ADD COLUMN field_id INTEGER REFERENCES farm_fields(id) ON DELETE SET NULL;
CREATE TABLE IF NOT EXISTS crop_varieties (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  crop TEXT NOT NULL,
  variety TEXT NOT NULL,
  maturity_days INTEGER,
  notes TEXT NOT NULL DEFAULT '',
  UNIQUE(crop, variety)
);
CREATE TABLE IF NOT EXISTS crop_health_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  cycle_id INTEGER NOT NULL REFERENCES crop_cycles(id) ON DELETE CASCADE,
  observed_date TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('pest','disease','weed','nutrient','other')),
  observation TEXT NOT NULL,
  affected_area REAL NOT NULL DEFAULT 0 CHECK(affected_area >= 0),
  action_taken TEXT NOT NULL DEFAULT '',
  product_applied TEXT NOT NULL DEFAULT '',
  quantity_applied TEXT NOT NULL DEFAULT '',
  cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(cost_kobo >= 0),
  advice_source TEXT NOT NULL DEFAULT '',
  review_status TEXT NOT NULL DEFAULT 'unconfirmed' CHECK(review_status IN ('unconfirmed','confirmed')),
  reviewer TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ---------------------------------------------------------------- storage
CREATE TABLE IF NOT EXISTS warehouses (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  kind TEXT NOT NULL DEFAULT 'store',
  notes TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived')),
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
-- Where the business stock (products.stock_qty) physically sits. Locations
-- never hold more than the business owns; transfers never change the total.
CREATE TABLE IF NOT EXISTS stock_locations (
  warehouse_id INTEGER NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
  product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  quantity REAL NOT NULL DEFAULT 0 CHECK(quantity >= 0),
  PRIMARY KEY (warehouse_id, product_id)
);
CREATE TABLE IF NOT EXISTS storage_movements (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  warehouse_id INTEGER NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
  product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  move_date TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('put_away','issue','transfer_out','transfer_in','spoilage','damage','loss','count','inspection')),
  quantity REAL NOT NULL DEFAULT 0,
  other_warehouse_id INTEGER REFERENCES warehouses(id) ON DELETE SET NULL,
  grade TEXT NOT NULL DEFAULT '',
  moisture_pct REAL,
  cost_kobo INTEGER NOT NULL DEFAULT 0 CHECK(cost_kobo >= 0),
  note TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_storage_moves ON storage_movements(warehouse_id, move_date);

-- ---------------------------------------------------------------- weather and markets
CREATE TABLE IF NOT EXISTS weather_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  record_date TEXT NOT NULL,
  rainfall_mm REAL, temp_min_c REAL, temp_max_c REAL,
  conditions TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS climate_outlooks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  year INTEGER NOT NULL,
  state TEXT NOT NULL,
  lga TEXT NOT NULL DEFAULT '',
  onset_date TEXT NOT NULL DEFAULT '',
  cessation_date TEXT NOT NULL DEFAULT '',
  season_days INTEGER,
  dry_spell_notes TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL,
  published_date TEXT NOT NULL DEFAULT '',
  imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(year, state, lga)
);
CREATE TABLE IF NOT EXISTS market_prices (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  product TEXT NOT NULL,
  market TEXT NOT NULL DEFAULT '',
  state TEXT NOT NULL DEFAULT '',
  lga TEXT NOT NULL DEFAULT '',
  price_date TEXT NOT NULL,
  unit TEXT NOT NULL,
  wholesale_kobo INTEGER, retail_kobo INTEGER, buyer_offer_kobo INTEGER,
  quantity_available TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1)),
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_market_prices ON market_prices(product, price_date);

-- ---------------------------------------------------------------- advisory and content
CREATE TABLE IF NOT EXISTS recommendation_rules (
  rule_key TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  priority INTEGER NOT NULL DEFAULT 2 CHECK(priority BETWEEN 1 AND 3),
  param_days INTEGER
);
CREATE TABLE IF NOT EXISTS advisory_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  flow TEXT NOT NULL,
  answers_json TEXT NOT NULL,
  results_json TEXT NOT NULL,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS content_versions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  package_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  published_date TEXT NOT NULL DEFAULT '',
  signed INTEGER NOT NULL DEFAULT 0 CHECK(signed IN (0,1)),
  summary TEXT NOT NULL DEFAULT '',
  imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  user_name TEXT NOT NULL DEFAULT ''
);

-- ---------------------------------------------------------------- demo flags on commerce records
ALTER TABLE products ADD COLUMN is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1));
ALTER TABLE receipts ADD COLUMN is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1));
ALTER TABLE purchases ADD COLUMN is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1));
ALTER TABLE expenses ADD COLUMN is_demo INTEGER NOT NULL DEFAULT 0 CHECK(is_demo IN (0,1));

INSERT OR IGNORE INTO settings(key, value) VALUES
  ('content_require_signature', '0'),
  ('last_content_update', ''),
  ('feed_low_days', '7');

-- Assets (equipment, land, buildings) and liabilities (loans, debts) the farmer
-- enters for the farm finance summary. FarmBuddy never approves or offers credit.
CREATE TABLE IF NOT EXISTS finance_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL CHECK(kind IN ('asset','liability')),
  name TEXT NOT NULL,
  value_kobo INTEGER NOT NULL CHECK(value_kobo >= 0),
  as_of TEXT NOT NULL,
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
