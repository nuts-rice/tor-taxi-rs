use anyhow::{Context, Result};
use rand::Rng;
use reqwest::{Client, Proxy};
use std::time::Duration;
use tracing::{info, warn};

/// A fresh SOCKS username. Tor isolates circuits per username/password pair, so
/// a new identity here means a new circuit for the next probe.
pub fn random_identity() -> String {
    let n: u64 = rand::rng().random();
    format!("tor-taxi-rs{n:x}")
}

pub fn build_client(
    socks_addr: &str,
    identity: &str,
    timeout: Duration,
    reuse_connections: bool,
) -> Result<Client> {
    // socks5h:// so the *proxy* resolves the hostname; resolving .onion locally
    // would fail before the request ever reached Tor.
    let proxy = Proxy::all(format!("socks5h://{socks_addr}"))
        .with_context(|| format!("invalid SOCKS address: {socks_addr}"))?
        .basic_auth(identity, identity);

    let mut builder = Client::builder()
        .proxy(proxy)
        .timeout(timeout)
        .danger_accept_invalid_certs(true); // onion services are rarely CA-signed

    if reuse_connections {
        info!(identity, "reusing connections for this circuit");
    } else {
        warn!(identity, "pooling disabled; every probe opens a fresh circuit");
        builder = builder.pool_max_idle_per_host(0);
    }

    builder.build().context("failed to build probe client")
}
