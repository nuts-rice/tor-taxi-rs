//! One sweep: probe every link over its own Tor circuit.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::future;
use futures::stream::{self, StreamExt, TryStreamExt};
use shared::{ContentCheck, UrlStatus};

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

/// What the network said about one address, or -- summarised -- one link.
///
/// An enum rather than `ok: bool` plus two `Option`s, so "reachable with no
/// latency" and "down with no reason" cannot be constructed at all. Callers
/// match instead of re-deriving the invariant at each use site.
#[derive(Debug, Clone)]
pub enum Outcome {
    Reachable { latency: Duration },
    Unreachable { reason: String },
}

impl Outcome {
    fn latency(&self) -> Option<Duration> {
        match self {
            Outcome::Reachable { latency } => Some(*latency),
            Outcome::Unreachable { .. } => None,
        }
    }
}

/// One address's probe: the primary url or a single mirror, on its own circuit.
#[derive(Debug, Clone)]
pub struct UrlProbe {
    pub url: String,
    pub outcome: Outcome,
    /// `None` when there was nothing to check: no `expected_content`, or down.
    pub content: Option<ContentCheck>,
}

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub slug: String,
    /// The link-level summary the status rule runs on. See [`Self::from_urls`].
    pub outcome: Outcome,
    /// Every address, primary first.
    pub urls: Vec<UrlProbe>,
}

impl ProbeResult {
    /// Summarises per-address probes into the link's outcome.
    ///
    /// The link is up if any address is: a user with a working mirror can
    /// reach the service. Its latency is the primary's when the primary is up,
    /// since that is the address the page leads with, and otherwise the
    /// fastest mirror's -- never the time spent failing on the primary first,
    /// which the old sequential fallback folded in and read as a slow site.
    pub fn from_urls(slug: impl Into<String>, urls: Vec<UrlProbe>) -> Self {
        let primary = urls.first().map(|u| &u.outcome);
        let outcome = match primary {
            Some(o @ Outcome::Reachable { .. }) => o.clone(),
            _ => urls
                .iter()
                .filter_map(|u| u.outcome.latency())
                .min()
                .map(|latency| Outcome::Reachable { latency })
                .or_else(|| primary.cloned())
                .unwrap_or(Outcome::Unreachable {
                    reason: "no addresses".into(),
                }),
        };
        ProbeResult {
            slug: slug.into(),
            outcome,
            urls,
        }
    }

    pub fn is_up(&self) -> bool {
        matches!(self.outcome, Outcome::Reachable { .. })
    }

    /// `None` when the link is down: an unreachable service has no meaningful
    /// latency, and writing the time spent failing would read as a slow site.
    pub fn latency_ms(&self) -> Option<u64> {
        self.outcome.latency().map(|l| l.as_millis() as u64)
    }

    /// Any address serving the wrong page taints the link. A clone sitting on
    /// one mirror is exactly the case this exists to surface, and the healthy
    /// primary beside it is no reason to stay quiet.
    pub fn content_mismatch(&self) -> bool {
        self.urls
            .iter()
            .any(|u| u.content == Some(ContentCheck::Mismatch))
    }

    /// What the Worker renders per address.
    pub fn url_status(&self) -> Vec<UrlStatus> {
        self.urls
            .iter()
            .map(|u| UrlStatus {
                url: u.url.clone(),
                up: u.outcome.latency().is_some(),
                latency_ms: u.outcome.latency().map(|l| l.as_millis() as u64),
                content: u.content,
            })
            .collect()
    }
}

/// Bodies are read only this far when checking `expected_content`. Enough for
/// any landing page's markers; small enough that a service streaming garbage
/// cannot hold a probe's memory hostage.
const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Probes the whole set, bounded to `concurrency` links at a time. Within a
/// link every address is probed at once, each on its own circuit.
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
            let addresses = std::iter::once(&link.url).chain(link.other_urls.iter().flatten());
            let mut probes = Vec::new();
            for url in addresses {
                // A fresh identity per address, so each gets its own circuit:
                // a slow mirror cannot stall the primary, and a mirror is
                // judged on its own path rather than reusing the primary's.
                let client = build_client(socks_addr, &random_identity(), policy.timeout, false)
                    .with_context(|| format!("building the probe client for `{slug}`"))?;
                let expected = link.expected_content.as_deref();
                probes.push(async move { probe_url(&client, slug, url, expected).await });
            }
            Ok(ProbeResult::from_urls(
                slug.as_str(),
                future::join_all(probes).await,
            ))
        })
        .buffer_unordered(policy.concurrency)
        .try_collect()
        .await
}

async fn probe_url(
    client: &reqwest::Client,
    slug: &str,
    url: &str,
    expected: Option<&str>,
) -> UrlProbe {
    let started = Instant::now();
    let response = client.get(url).send().await;
    // Latency is time to the response head, taken before any body is read, so
    // a link with `expected_content` is not reported slower than one without.
    let elapsed = started.elapsed();

    match response {
        // Any response at all means the hidden service answered. A 404 or a 503
        // is the site talking, which is what we are actually testing for -- only
        // a transport failure counts as down.
        Ok(resp) => {
            tracing::info!(slug, url, status = %resp.status(), latency_ms = elapsed.as_millis() as u64, "up");
            let content = match expected {
                None => None,
                Some(expected) => Some(match read_capped(resp, MAX_BODY_BYTES).await {
                    Ok(body) => content_verdict(expected, &body),
                    // The head arrived but the body did not: the service is
                    // up, and we learned nothing about who runs it.
                    Err(e) => {
                        tracing::warn!(slug, url, error = %e, "body read failed");
                        ContentCheck::Inconclusive
                    }
                }),
            };
            if content == Some(ContentCheck::Mismatch) {
                tracing::warn!(slug, url, "content does not match expected_content");
            }
            UrlProbe {
                url: url.to_string(),
                outcome: Outcome::Reachable { latency: elapsed },
                content,
            }
        }
        Err(e) => {
            tracing::warn!(slug, url, error = ?e, waited_ms = elapsed.as_millis() as u64, "down");
            UrlProbe {
                url: url.to_string(),
                outcome: Outcome::Unreachable {
                    reason: e.to_string(),
                },
                content: None,
            }
        }
    }
}

/// Reads at most `cap` bytes of the body, lossily decoded. Truncation is fine:
/// a marker is looked for, not the whole page validated.
async fn read_capped(mut resp: reqwest::Response, cap: usize) -> Result<String> {
    let mut buf = Vec::new();
    while buf.len() < cap {
        match resp.chunk().await? {
            Some(chunk) => buf.extend_from_slice(&chunk[..chunk.len().min(cap - buf.len())]),
            None => break,
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Decides whether a page is the service links.toml says it is.
///
/// `expected` is the `expected_content` string from links.toml; `body` is the
/// first `MAX_BODY_BYTES` of the response, lossily decoded as UTF-8.
fn content_verdict(expected: &str, body: &str) -> ContentCheck {
    // TODO(human)
    let _ = (expected, body);
    ContentCheck::Inconclusive
}
