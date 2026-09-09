//! Out-of-band onion link prober.
//!
//! Runs on a host with a Tor daemon, probes each link through the SOCKS port,
//! and writes the resulting status into D1. The Cloudflare Worker only ever
//! reads that table — the Workers runtime has no Tor and cannot reach .onion.

mod d1;
mod links;
mod probe;
mod proxy;
mod status;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(about = "Probe onion links over Tor and write their status to D1")]
pub struct Args {
    /// Time between sweeps, e.g. `15m`, `90s`, `1h`.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "15m")]
    pub interval: Duration,

    /// The canonical link set.
    #[arg(long, default_value = "crates/checker/links.toml")]
    pub links: PathBuf,

    /// Tor's SOCKS port.
    #[arg(long, env = "TOR_SOCKS_ADDR", default_value = "127.0.0.1:9050")]
    pub socks_addr: String,

    /// Probe once and exit, instead of looping. Useful for a first run.
    #[arg(long)]
    pub once: bool,

    /// Probe and report, but do not write to D1. Needs no credentials, so it is
    /// the way to try a new link before committing it to links.toml.
    #[arg(long)]
    pub dry_run: bool,

    /// Per-probe network timeout.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "60s")]
    pub timeout: Duration,

    /// A reachable link slower than this is reported as degraded, not healthy.
    #[arg(long, value_parser = humantime::parse_duration, default_value = "10s")]
    pub orange_after: Duration,

    /// Consecutive failed sweeps before a link is reported as down.
    #[arg(long, default_value_t = 3)]
    pub red_after: u32,

    /// How many links to probe at once, each on its own Tor circuit.
    #[arg(long, default_value_t = 8)]
    pub concurrency: usize,
}

impl Args {
    fn policy(&self) -> status::Policy {
        status::Policy {
            timeout: self.timeout,
            orange_after: self.orange_after,
            red_after: self.red_after,
            concurrency: self.concurrency,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tor_taxi_checker=info".into()),
        )
        .init();

    let args = Args::parse();
    // Fail fast on missing credentials rather than after the first full sweep.
    let d1 = if args.dry_run {
        None
    } else {
        Some(d1::D1Client::from_env()?)
    };

    // Advisory only. If Tor is not up yet, the sweep below will say so far more
    // clearly than a startup probe can.
    match proxy::verify_isolation(&args.socks_addr).await {
        Ok(true) => tracing::info!("circuit isolation confirmed"),
        Ok(false) => tracing::warn!("circuit isolation looks disabled"),
        Err(e) => tracing::warn!(error = ?e, "could not verify circuit isolation"),
    }

    loop {
        if let Err(e) = sweep(&args, d1.as_ref()).await {
            // A Tor hiccup or a transient API error must not kill the service;
            // the next sweep gets another go.
            tracing::error!(error = ?e, "sweep failed");
        }

        if args.once {
            return Ok(());
        }
        tokio::time::sleep(args.interval).await;
    }
}

async fn sweep(args: &Args, d1: Option<&d1::D1Client>) -> Result<()> {
    // Re-read every sweep so adding a link does not need a restart.
    let links = links::load(&args.links)?;
    tracing::info!(count = links.len(), "starting sweep");

    let policy = args.policy();
    let results = probe::probe_all(&links, &args.socks_addr, &policy).await;

    let up = results.iter().filter(|r| r.ok).count();
    for failed in results.iter().filter(|r| !r.ok) {
        tracing::warn!(
            slug = %failed.slug,
            reason = failed.error.as_deref().unwrap_or("unknown"),
            "link down"
        );
    }
    tracing::info!(up, down = results.len() - up, "sweep complete");

    match d1 {
        Some(d1) => d1.flush(&links, &results, &policy).await,
        None => {
            tracing::info!("dry run: skipping the D1 write");
            Ok(())
        }
    }
}
