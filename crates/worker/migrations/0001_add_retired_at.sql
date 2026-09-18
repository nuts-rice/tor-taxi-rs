-- Migration number: 0001 	 2026-09-18T19:00:31.826Z
--
-- Soft delete for delisting. A slug that disappears from links.toml gets a
-- retired_at stamp instead of a DELETE, and the Worker filters the column out
-- of its SELECT -- so the row stops rendering without losing the probe history
-- that took weeks of sweeps to accumulate.
--
-- Soft rather than hard because delisting is reversible: services come back,
-- and the checker's upsert clears retired_at on sight, so a re-listed link
-- arrives with its consecutive_failures and last_good_at intact. It also means
-- a links.toml that briefly reads short -- a half-written file caught mid-save
-- -- costs one sweep of visibility rather than the table.
ALTER TABLE links ADD COLUMN retired_at INTEGER;

-- The Worker's every query filters on this, and it is overwhelmingly NULL.
-- A partial index stays small: it holds only the retired rows.
CREATE INDEX IF NOT EXISTS links_retired ON links (retired_at) WHERE retired_at IS NOT NULL;
