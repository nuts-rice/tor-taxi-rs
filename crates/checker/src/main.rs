//! Out-of-band onion link prober.
//!
//! Runs on a host with a Tor daemon, probes each link through the SOCKS port,
//! and writes the resulting status into D1. The Cloudflare Worker only ever
//! reads that table — the Workers runtime has no Tor and cannot reach .onion.

mod proxy;

use std::time::Duration;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tor_taxi_checker=info".into()),
        )
        .init();

    let socks_addr =
        std::env::var("TOR_SOCKS_ADDR").unwrap_or_else(|_| "127.0.0.1:9050".to_string());
    let identity = proxy::random_identity();
    let _client = proxy::build_client(&socks_addr, &identity, Duration::from_secs(30), false)?;

    // TODO: read the link set, probe each URL over `_client`, and push the
    // results into D1 (Cloudflare REST API or `wrangler d1 execute`).
    tracing::info!(%socks_addr, %identity, "checker ready; probe loop not wired up yet");

    Ok(())
}
