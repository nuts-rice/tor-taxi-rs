//! Status policy: the rule that turns a probe outcome into a dot on the page.
//!
//! | condition                        | status   |
//! |----------------------------------|----------|
//! | reachable, faster than `orange_after` | `White`  |
//! | reachable, at or over it         | `Orange` |
//! | 1..`red_after` consecutive failures   | `Orange` |
//! | `red_after`+ consecutive failures     | `Red`    |
//!

use std::time::Duration;

#[derive(Debug, Clone, Copy, clap::Args)]
pub struct StatusPolicy {
    /// A reachable link slower than this is reported as degraded, not healthy.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "10s")]
    pub orange_after: Duration,

    /// Consecutive failed sweeps before a link is reported as down.
    #[arg(long, default_value_t = 3)]
    pub red_after: u32,
}

impl StatusPolicy {
    /// The table above, for a caller that counts failures itself (the TUI).
    /// `d1::RECORD_SQL` applies the same rule in SQL; keep the two in step.
    pub fn classify(
        &self,
        latency: Option<Duration>,
        consecutive_failures: u32,
    ) -> shared::LinkStatus {
        use shared::LinkStatus::{Orange, Red, White};
        match latency {
            Some(l) if l >= self.orange_after => Orange,
            Some(_) => White,
            None if consecutive_failures >= self.red_after => Red,
            None => Orange,
        }
    }
}
