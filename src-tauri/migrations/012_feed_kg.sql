-- 1.0.8 Feed by the kg: the starter/finisher split of the feed plan and the kg given each day.
ALTER TABLE pb_batches ADD COLUMN starter_pct REAL NOT NULL DEFAULT 30;
ALTER TABLE pb_batches ADD COLUMN starter_weeks INTEGER NOT NULL DEFAULT 4;
ALTER TABLE pb_daily ADD COLUMN kg_given REAL NOT NULL DEFAULT 0;
UPDATE pb_daily SET kg_given = bags_issued * COALESCE((SELECT bag_kg FROM pb_feeds f WHERE f.id = pb_daily.feed_id), 25) WHERE kg_given = 0 AND bags_issued > 0;
