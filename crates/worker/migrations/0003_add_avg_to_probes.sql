-- Migration number: 0003 	 2026-09-30T00:00:00.000Z
--
-- Rolling mean latency, stamped onto each sample as it is written: the average
-- of that slug's up samples inside the retention window, this one included.
-- Stored rather than computed on read so the history keeps what the average
-- *was* at each sweep, which a later avg() over the pruned table cannot recover.
--
-- NULL when the window holds no up samples -- avg() ignores the NULL latencies
-- of down probes, so an all-down window has no mean, not a mean of zero.
ALTER TABLE probes ADD COLUMN avg_latency_ms REAL;
