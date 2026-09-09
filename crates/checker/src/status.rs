//! Status policy: the rule that turns a probe outcome into a dot on the page.
//!
//! | condition                        | status   |
//! |----------------------------------|----------|
//! | reachable, faster than `orange_after` | `White`  |
//! | reachable, at or over it         | `Orange` |
//! | 1..`red_after` consecutive failures   | `Orange` |
//! | `red_after`+ consecutive failures     | `Red`    |
//!
//! Grace before Red keeps one dropped circuit from turning the whole page red;
//! onion services are flaky by nature.
//!
//! The rule is *applied* in SQL (see [`crate::d1`]) so `consecutive_failures`
//! is read and written in one statement and the checker stays stateless across
//! restarts. This module only carries the numbers.

use std::time::Duration;

/// Every value here is a judgement call about what the dots mean, so all of
/// them are exposed on the CLI rather than baked in. Defaults come from
/// measurement, not taste -- see each field.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    /// Per-probe network timeout.
    ///
    /// Generous on purpose: a cold onion probe pays circuit setup, often a
    /// redirect onto a second circuit, a TLS handshake, and whatever anti-DDoS
    /// interstitial the service runs. Measured on real traffic: healthy sites
    /// answer in 3-5s, but dread has taken 41s from cold. Timing out early
    /// reports a live service as down, the one error this tool must not make.
    pub timeout: Duration,

    /// A reachable service slower than this is treated as struggling.
    ///
    /// Note the tension: a site that is *reliably* slow will sit on Orange
    /// forever, which drains the colour of meaning. Raise it if that happens.
    pub orange_after: Duration,

    /// Consecutive failed sweeps before a link is called down.
    pub red_after: u32,

    /// How many links to probe at once. Each opens its own Tor circuit, so this
    /// is a courtesy limit on the local daemon as much as a throughput knob.
    pub concurrency: usize,
}
