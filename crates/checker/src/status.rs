//! Status policy: the rule that turns a probe outcome into a dot on the page.
//!
//!
//! | condition                     | status   |
//! |-------------------------------|----------|
//! | reachable, under the latency  | `White`  |
//! | reachable, at or over it      | `Orange` |
//! | 1-2 consecutive failures      | `Orange` |
//! | 3+ consecutive failures       | `Red`    |
//!

use std::time::Duration;

/// A reachable service slower than this is treated as struggling, not healthy.
pub const ORANGE_STATUS_THRESHOLD: Duration = Duration::from_secs(10);

/// Consecutive failed sweeps before a link is called down.
pub const RED_STATUS_THRESHOLD: u32 = 3;

/// Per-probe network timeout
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// How many links to probe at once.
pub const PROBE_CONCURRENCY: usize = 8;
