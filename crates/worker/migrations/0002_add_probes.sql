-- Migration number: 0002 	 2026-09-28T00:00:00.000Z
CREATE TABLE IF NOT EXISTS probes (
  slug        TEXT    NOT NULL,
  checked_at  INTEGER NOT NULL,
  latency_ms  INTEGER,
  PRIMARY KEY (slug, checked_at)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS probes_checked_at ON probes (checked_at);
