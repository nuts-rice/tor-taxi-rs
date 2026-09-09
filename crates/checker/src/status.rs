//! Status policy: the rule that turns a probe outcome into a dot on the page.
//!
//! Kept in one place because it is the only thing the directory really asserts.
//! The rule itself is applied in SQL (see [`crate::d1`]) so that
//! `consecutive_failures` is read and written in the same statement and the
//! checker stays stateless across restarts.
//!
//! | condition                     | status   |
//! |-------------------------------|----------|
//! | reachable, under the latency  | `White`  |
//! | reachable, at or over it      | `Orange` |
//! | 1-2 consecutive failures      | `Orange` |
//! | 3+ consecutive failures       | `Red`    |
//!
//! Two failures of grace before Red keeps one dropped circuit from turning the
//! whole page red; onion services are flaky by nature.

use std::time::Duration;

// The sweep interval lives on `--interval` in main.rs, not here -- one source
// of truth beats a constant and a clap default that can drift apart.

/// A reachable service slower than this is treated as struggling, not healthy.
pub const ORANGE_STATUS_THRESHOLD: Duration = Duration::from_secs(10);

/// Consecutive failed sweeps before a link is called down.
pub const RED_STATUS_THRESHOLD: u32 = 3;

/// Per-probe network timeout.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// How many links to probe at once. Each one opens its own Tor circuit, so this
/// is a courtesy limit on the local daemon as much as anything.
pub const PROBE_CONCURRENCY: usize = 8;
