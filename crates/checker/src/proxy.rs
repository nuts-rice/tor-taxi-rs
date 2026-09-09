use anyhow::{Context, Result};
use rand::Rng;
use reqwest::{Client, Proxy};
use std::time::Duration;
use tracing::{info, warn};

/// Onion services commonly sit behind anti-DDoS layers that reject requests
/// with no User-Agent
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; rv:128.0) Gecko/20100101 Firefox/128.0";

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
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .danger_accept_invalid_certs(true); // onion services are rarely CA-signed

    if reuse_connections {
        info!(identity, "reusing connections for this circuit");
    } else {
        builder = builder.pool_max_idle_per_host(0);
    }

    builder.build().context("failed to build probe client")
}

/// Confirms the SOCKS port really is isolating circuits per identity, by asking
/// Tor what exit it would use for two different identities.
///
/// Worth running once at startup: if isolation is off, every probe shares one
/// circuit and a single slow service stalls the whole sweep.
pub async fn verify_isolation(socks_addr: &str) -> Result<bool> {
    const CHECK_URL: &str = "https://check.torproject.org/api/ip";

    async fn exit_ip(socks_addr: &str, identity: &str) -> Result<String> {
        let client = build_client(socks_addr, identity, Duration::from_secs(30), true)?;
        let body = client
            .get(CHECK_URL)
            .send()
            .await
            .context("could not reach the Tor check service")?
            .text()
            .await?;
        Ok(body)
    }

    let a = exit_ip(socks_addr, &random_identity()).await?;
    let b = exit_ip(socks_addr, &random_identity()).await?;

    let isolated = a != b;
    if !isolated {
        // Not fatal: two identities can legitimately land on the same exit. It
        // is only a hint that IsolateSOCKSAuth may not be on.
        warn!("both identities saw the same exit; circuit isolation may be off");
    }
    Ok(isolated)
}
