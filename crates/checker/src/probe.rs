//! One sweep: probe every link over its own Tor circuit.

use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};

use crate::links::LinkSet;
use crate::proxy::{build_client, random_identity};

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub slug: String,
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// Probes the whole set, bounded to `concurrency` circuits at a time.
pub async fn probe_all(
    links: &LinkSet,
    socks_addr: &str,
    timeout: Duration,
    concurrency: usize,
) -> Vec<ProbeResult> {
    stream::iter(links.iter())
        .map(|(slug, link)| async move {
            // A fresh identity per link, so each gets its own circuit and a slow
            // service cannot drag down the one probed after it.
            let identity = random_identity();
            match build_client(socks_addr, &identity, timeout, false) {
                Ok(client) => probe_one(&client, slug, &link.url).await,
                Err(e) => ProbeResult {
                    slug: slug.clone(),
                    ok: false,
                    latency_ms: None,
                    error: Some(format!("client build failed: {e}")),
                },
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await
}

async fn probe_one(client: &reqwest::Client, slug: &str, url: &str) -> ProbeResult {
    let started = Instant::now();
    let outcome = client.get(url).send().await;
    let latency_ms = started.elapsed().as_millis() as u64;

    match outcome {
        // Any response at all means the hidden service answered. A 404 or a 503
        // is the site talking, which is what we are actually testing for -- only
        // a transport failure counts as down.
        Ok(resp) => {
            tracing::info!(slug, status = %resp.status(), latency_ms, "up");
            ProbeResult {
                slug: slug.to_string(),
                ok: true,
                latency_ms: Some(latency_ms),
                error: None,
            }
        }
        Err(e) => {
            tracing::warn!(slug, error = ?e, latency_ms, "down");
            ProbeResult {
                slug: slug.to_string(),
                ok: false,
                latency_ms: None,
                error: Some(e.to_string()),
            }
        }
    }
}
