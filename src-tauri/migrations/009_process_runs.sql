-- FarmBuddy 1.0.4: processing runs filled in with one simple form.
-- The same runs now cover palm oil, garri, fufu and cassava flour; the
-- answers are kept as typed, and what each run put into stock is recorded so
-- editing a run moves only the difference.
ALTER TABLE palm_runs ADD COLUMN product TEXT NOT NULL DEFAULT 'palm_oil';
ALTER TABLE palm_runs ADD COLUMN answers TEXT NOT NULL DEFAULT '{}';
ALTER TABLE palm_runs ADD COLUMN stock_sync TEXT NOT NULL DEFAULT '{}';
