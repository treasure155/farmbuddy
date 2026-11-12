-- 1.0.6 Livestock batches with plain questions: intake → daily records → feed
-- store and restocking → vaccines and medicine (withdrawal) → weighing → expenses
-- → sales with receipts → actual and projected profit or loss.

CREATE TABLE IF NOT EXISTS pb_batches (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  bird_type TEXT NOT NULL DEFAULT 'Broilers',
  breed TEXT NOT NULL DEFAULT '',
  house TEXT NOT NULL DEFAULT '',
  manager TEXT NOT NULL DEFAULT '',
  planned_birds INTEGER NOT NULL DEFAULT 0 CHECK(planned_birds >= 0),
  arrival_date TEXT NOT NULL,
  age_at_arrival INTEGER NOT NULL DEFAULT 1 CHECK(age_at_arrival >= 0),
  weeks INTEGER NOT NULL DEFAULT 8 CHECK(weeks >= 0),
  sell_age_weeks INTEGER NOT NULL DEFAULT 8 CHECK(sell_age_weeks >= 0),
  expected_sell INTEGER NOT NULL DEFAULT 0,
  expected_weight_kg REAL NOT NULL DEFAULT 0,
  price_per TEXT NOT NULL DEFAULT 'bird' CHECK(price_per IN ('bird','kg')),
  expected_price_kobo INTEGER NOT NULL DEFAULT 0 CHECK(expected_price_kobo >= 0),
  budget_kobo INTEGER NOT NULL DEFAULT 0 CHECK(budget_kobo >= 0),
  bags_per_100 REAL NOT NULL DEFAULT 17 CHECK(bags_per_100 >= 0),
  bag_kg REAL NOT NULL DEFAULT 25 CHECK(bag_kg > 0),
  plan_starter REAL NOT NULL DEFAULT 0,
  plan_grower REAL NOT NULL DEFAULT 0,
  plan_finisher REAL NOT NULL DEFAULT 0,
  feed_change TEXT NOT NULL DEFAULT '',
  chicks_ordered INTEGER NOT NULL DEFAULT 0,
  chicks_delivered INTEGER NOT NULL DEFAULT 0,
  dead_on_arrival INTEGER NOT NULL DEFAULT 0,
  chicks_paid_for INTEGER NOT NULL DEFAULT 0,
  chick_price_kobo INTEGER NOT NULL DEFAULT 0,
  chick_transport_kobo INTEGER NOT NULL DEFAULT 0,
  chick_supplier TEXT NOT NULL DEFAULT '',
  chick_amount_paid_kobo INTEGER NOT NULL DEFAULT 0,
  supplier_receipt INTEGER NOT NULL DEFAULT 0,
  hatchery_vaccines TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','closed')),
  end_date TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  is_demo INTEGER NOT NULL DEFAULT 0,
  created_by TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- One daily record per batch per day (saving again updates it; edits are logged).
CREATE TABLE IF NOT EXISTS pb_daily (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id INTEGER NOT NULL REFERENCES pb_batches(id) ON DELETE CASCADE,
  day TEXT NOT NULL,
  deaths INTEGER NOT NULL DEFAULT 0 CHECK(deaths >= 0),
  cause TEXT NOT NULL DEFAULT '',
  disposed INTEGER NOT NULL DEFAULT 1,
  sick INTEGER NOT NULL DEFAULT 0 CHECK(sick >= 0),
  removed INTEGER NOT NULL DEFAULT 0 CHECK(removed >= 0),
  added INTEGER NOT NULL DEFAULT 0 CHECK(added >= 0),
  feed_id INTEGER REFERENCES pb_feeds(id) ON DELETE SET NULL,
  bags_issued REAL NOT NULL DEFAULT 0 CHECK(bags_issued >= 0),
  kg_eaten REAL NOT NULL DEFAULT 0 CHECK(kg_eaten >= 0),
  kg_wasted REAL NOT NULL DEFAULT 0 CHECK(kg_wasted >= 0),
  water_l REAL NOT NULL DEFAULT 0,
  feed_unusual TEXT NOT NULL DEFAULT '',
  observations TEXT NOT NULL DEFAULT '',
  problems TEXT NOT NULL DEFAULT '',
  tomorrow TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(batch_id, day)
);

-- The feed store: one row per feed type (starter, grower, finisher…).
CREATE TABLE IF NOT EXISTS pb_feeds (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  farm_id INTEGER REFERENCES farms(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  brand TEXT NOT NULL DEFAULT '',
  bag_kg REAL NOT NULL DEFAULT 25 CHECK(bag_kg > 0),
  min_bags REAL NOT NULL DEFAULT 0,
  full_bags REAL NOT NULL DEFAULT 0,
  active INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS pb_feed_moves (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  feed_id INTEGER NOT NULL REFERENCES pb_feeds(id) ON DELETE CASCADE,
  move_date TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('opening','purchase','issue','damaged','returned')),
  bags REAL NOT NULL CHECK(bags > 0),
  price_kobo INTEGER NOT NULL DEFAULT 0,
  amount_kobo INTEGER NOT NULL DEFAULT 0,
  paid_kobo INTEGER NOT NULL DEFAULT 0,
  supplier TEXT NOT NULL DEFAULT '',
  good_condition INTEGER NOT NULL DEFAULT 1,
  has_receipt INTEGER NOT NULL DEFAULT 0,
  batch_id INTEGER REFERENCES pb_batches(id) ON DELETE CASCADE,
  daily_id INTEGER REFERENCES pb_daily(id) ON DELETE CASCADE,
  note TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_pb_feed_moves ON pb_feed_moves(feed_id, move_date);

-- Vaccines, medicines, vitamins and the planned timetable.
CREATE TABLE IF NOT EXISTS pb_health (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id INTEGER NOT NULL REFERENCES pb_batches(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(kind IN ('plan','vaccine','medicine','vitamin')),
  name TEXT NOT NULL,
  purpose TEXT NOT NULL DEFAULT '',
  due_from TEXT NOT NULL DEFAULT '',
  due_to TEXT NOT NULL DEFAULT '',
  done_date TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'done' CHECK(status IN ('planned','done','skipped')),
  brand TEXT NOT NULL DEFAULT '',
  method TEXT NOT NULL DEFAULT '',
  doses INTEGER NOT NULL DEFAULT 0,
  birds INTEGER NOT NULL DEFAULT 0,
  expiry TEXT NOT NULL DEFAULT '',
  cost_kobo INTEGER NOT NULL DEFAULT 0,
  given_by TEXT NOT NULL DEFAULT '',
  success INTEGER NOT NULL DEFAULT 1,
  reaction TEXT NOT NULL DEFAULT '',
  end_date TEXT NOT NULL DEFAULT '',
  withdrawal_days INTEGER NOT NULL DEFAULT 0,
  next_due TEXT NOT NULL DEFAULT '',
  plan_id INTEGER,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS pb_weights (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id INTEGER NOT NULL REFERENCES pb_batches(id) ON DELETE CASCADE,
  weigh_date TEXT NOT NULL,
  sampled INTEGER NOT NULL CHECK(sampled > 0),
  total_kg REAL NOT NULL CHECK(total_kg > 0),
  smaller TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS pb_expenses (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id INTEGER NOT NULL REFERENCES pb_batches(id) ON DELETE CASCADE,
  spent_date TEXT NOT NULL,
  category TEXT NOT NULL,
  item TEXT NOT NULL,
  quantity REAL NOT NULL DEFAULT 1,
  unit_price_kobo INTEGER NOT NULL DEFAULT 0,
  amount_kobo INTEGER NOT NULL CHECK(amount_kobo > 0),
  paid_kobo INTEGER NOT NULL DEFAULT 0,
  method TEXT NOT NULL DEFAULT '',
  bought_by TEXT NOT NULL DEFAULT '',
  supplier TEXT NOT NULL DEFAULT '',
  has_receipt INTEGER NOT NULL DEFAULT 0,
  approved INTEGER NOT NULL DEFAULT 0,
  approved_by TEXT NOT NULL DEFAULT '',
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Sales (status 'reserved' holds birds for a buyer; 'sold' makes a receipt).
CREATE TABLE IF NOT EXISTS pb_sales (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id INTEGER NOT NULL REFERENCES pb_batches(id) ON DELETE RESTRICT,
  sale_date TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'sold' CHECK(status IN ('reserved','sold')),
  birds INTEGER NOT NULL CHECK(birds > 0),
  price_per TEXT NOT NULL DEFAULT 'bird' CHECK(price_per IN ('bird','kg')),
  avg_kg REAL NOT NULL DEFAULT 0,
  price_kobo INTEGER NOT NULL DEFAULT 0,
  amount_kobo INTEGER NOT NULL DEFAULT 0,
  delivery TEXT NOT NULL DEFAULT '',
  delivery_cost_kobo INTEGER NOT NULL DEFAULT 0,
  buyer TEXT NOT NULL DEFAULT '',
  phone TEXT NOT NULL DEFAULT '',
  evidence TEXT NOT NULL DEFAULT '',
  receipt_id INTEGER REFERENCES receipts(id) ON DELETE SET NULL,
  user_name TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_pb_sales_batch ON pb_sales(batch_id);
