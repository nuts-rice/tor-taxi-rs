-- The contract between the checker (writes) and the Worker (reads).
--
-- Apply with:
--   npx wrangler d1 execute prod-links --remote --file=crates/worker/schema.sql
--
-- Safe to re-run: this file only ever creates. It must never DROP -- the
-- probe history in consecutive_failures and last_good_at is the only thing
-- that makes a status dot mean anything, and it cannot be recomputed.
--
-- The link set is deliberately NOT seeded here. crates/checker/links.toml is
-- the canonical list; the checker upserts it at the top of every sweep.

CREATE TABLE IF NOT EXISTS links (
  -- Editorial identity, taken from the table keys in links.toml ([dread], ...).
  -- Deliberately not the url: onion addresses rotate when a service re-keys,
  -- and a rotation should update the row in place rather than orphan it and
  -- silently start a second history under the new address.
  slug                 TEXT PRIMARY KEY,
  url                  TEXT NOT NULL UNIQUE,

  -- CHECK mirrors the LinkCategory variants in the Worker. Serde matches
  -- variant names exactly, so anything that passes here is guaranteed to
  -- deserialize on the read side.
  category             TEXT NOT NULL CHECK (category IN (
                         'News', 'Search', 'Email', 'Market', 'Exchange',
                         'ImageUpload', 'Info', 'Escrow', 'Forum', 'Service'
                       )),
  description          TEXT NOT NULL DEFAULT '',

  -- NULL until the checker has probed at least once. "Never checked" is a real
  -- state and it is not Red -- Red claims the service is down, which we do not
  -- yet know. Read it as Option<Status>, with last_checked_at IS NULL as the
  -- no-data-yet signal.
  status               TEXT CHECK (status IN ('Red', 'Orange', 'White')),
  latency_ms           INTEGER,
  consecutive_failures INTEGER NOT NULL DEFAULT 0,

  -- Unix seconds. SQLite has no real date type, and integers keep
  -- "checked 4m ago" to plain arithmetic on both sides.
  last_good_at         INTEGER,
  last_checked_at      INTEGER
);

CREATE INDEX IF NOT EXISTS links_category ON links (category);
