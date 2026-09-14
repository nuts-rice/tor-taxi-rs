//! One sweep: probe every link over its own Tor circuit.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt, TryStreamExt};

use crate::links::LinkSet;
use crate::proxy::{build_client, random_identity};

#[derive(Debug, Clone, Copy, clap::Args)]
pub struct ProbePolicy {
    /// Per-probe network timeout. Generous on purpose: a cold onion probe pays
    /// circuit setup, often a redirect onto a second circuit, a TLS handshake,
    /// and whatever anti-DDoS interstitial the service runs.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "60s")]
    pub timeout: Duration,

    /// How many links to probe at once, each on its own Tor circuit.
    #[arg(long, default_value_t = 8)]
    pub concurrency: usize,
}

/// What the network said about one link.
///
/// An enum rather than `ok: bool` plus two `Option`s, so "reachable with no
/// latency" and "down with no reason" cannot be constructed at all. Callers
/// match instead of re-deriving the invariant at each use site.
#[derive(Debug, Clone)]
pub enum Outcome {
    Reachable { latency: Duration },
    Unreachable { reason: String },
}

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub slug: String,
    pub outcome: Outcome,
}

impl ProbeResult {
    pub fn is_up(&self) -> bool {
        matches!(self.outcome, Outcome::Reachable { .. })
    }

    /// `None` when the link is down: an unreachable service has no meaningful
    /// latency, and writing the time spent failing would read as a slow site.
    pub fn latency_ms(&self) -> Option<u64> {
        match &self.outcome {
            Outcome::Reachable { latency } => Some(latency.as_millis() as u64),
            Outcome::Unreachable { .. } => None,
        }
    }
}

/// Probes the whole set, bounded to `concurrency` circuits at a time.
///
/// Fails the sweep if a probe client cannot be built. That is a local
/// configuration fault -- a malformed `--socks-addr`, a broken TLS backend --
/// and not evidence about any hidden service. Reporting it as an outcome would
/// mark every link down and, after `--red-after` sweeps, turn the whole
/// directory Red on the strength of one bad flag.
pub async fn probe_all(
    links: &LinkSet,
    socks_addr: &str,
    policy: &ProbePolicy,
) -> Result<Vec<ProbeResult>> {
    stream::iter(links.iter())
        .map(|(slug, link)| async move {
            // A fresh identity per link, so each gets its own circuit and a slow
            // service cannot drag down the one probed after it.
            let identity = random_identity();
            let client = build_client(socks_addr, &identity, policy.timeout, false)
                .with_context(|| format!("building the probe client for `{slug}`"))?;
            Ok(probe_one(&client, slug, &link.url).await)
        })
        .buffer_unordered(policy.concurrency)
        .try_collect()
        .await
}

async fn probe_one(client: &reqwest::Client, slug: &str, url: &str) -> ProbeResult {
    let started = Instant::now();
    let response = client.get(url).send().await;
    let elapsed = started.elapsed();

    let outcome = match response {
        // Any response at all means the hidden service answered. A 404 or a 503
        // is the site talking, which is what we are actually testing for -- only
        // a transport failure counts as down.
        Ok(resp) => {
            tracing::info!(slug, status = %resp.status(), latency_ms = elapsed.as_millis() as u64, "up");
            Outcome::Reachable { latency: elapsed }
        }
        Err(e) => {
            tracing::warn!(slug, error = ?e, waited_ms = elapsed.as_millis() as u64, "down");
            Outcome::Unreachable {
                reason: e.to_string(),
            }
        }
    };

    ProbeResult {
        slug: slug.to_string(),
        outcome,
    }
}
