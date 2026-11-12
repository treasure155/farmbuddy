-- Migration 003: offline account recovery for owners.
-- One row holds the recovery key hash and the three security questions with
-- hashed answers. Either one proves ownership for "Forgot password" and for
-- creating a new Owner account from the sign-in screen.
CREATE TABLE IF NOT EXISTS owner_recovery (
  id INTEGER PRIMARY KEY CHECK(id = 1),
  key_hash TEXT NOT NULL DEFAULT '',
  q1 TEXT NOT NULL DEFAULT '',
  q2 TEXT NOT NULL DEFAULT '',
  q3 TEXT NOT NULL DEFAULT '',
  a1_hash TEXT NOT NULL DEFAULT '',
  a2_hash TEXT NOT NULL DEFAULT '',
  a3_hash TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT OR IGNORE INTO owner_recovery(id) VALUES (1);

-- Sign-in and recovery events the owner can review in Settings -> Security.
CREATE TABLE IF NOT EXISTS security_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event TEXT NOT NULL,
  username TEXT NOT NULL DEFAULT '',
  detail TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_security_events_created_at ON security_events(created_at DESC);
