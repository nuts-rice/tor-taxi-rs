-- Migration number: 0002 	 2026-09-28T00:00:00.000Z
--
-- Per-probe history, so latency can be averaged over a window instead of
-- read off the single latest sample in links.latency_ms. The checker appends
-- one row per link per sweep and prunes anything older than its retention
-- window in the same batch, so the table stays bounded without a second job.
--
-- latency_ms is NULL when the link was down, matching ProbeResult::latency_ms().
-- avg() skips NULLs, so an outage never reads as a slow site, and
-- count(*) - count(latency_ms) is the number of failed probes in the window.
--
-- No FOREIGN KEY to links on purpose: D1 enforces foreign keys, and
-- RELEASE_URL_SQL deletes retired rows -- a reference here would fail the
-- whole batch the first time a renamed slug frees its url. Orphaned samples
-- age out with the prune like any other.
CREATE TABLE IF NOT EXISTS probes (
  slug        TEXT    NOT NULL,
  -- Unix seconds, the same `now` the sweep stamps on links.last_checked_at.
  checked_at  INTEGER NOT NULL,
  latency_ms  INTEGER,
  PRIMARY KEY (slug, checked_at)
) WITHOUT ROWID;

-- The prune filters on checked_at alone, which the (slug, checked_at) key
-- cannot serve. Without this every sweep scans the whole table, and D1 bills
-- rows read.
CREATE INDEX IF NOT EXISTS probes_checked_at ON probes (checked_at);
