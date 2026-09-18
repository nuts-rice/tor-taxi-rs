//! Out-of-band onion link prober.
//!
//! Runs on a host with a Tor daemon, probes each link through the SOCKS port,
//! and writes the resulting status into D1. The Cloudflare Worker only ever
//! reads that table — the Workers runtime has no Tor and cannot reach .onion.

pub mod d1;
mod links;
mod probe;
mod proxy;
mod status;

use std::path::PathBuf;
use std::time::Duration;

use crate::probe::{Outcome, ProbePolicy};
use crate::status::StatusPolicy;
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

    #[command(flatten)]
    pub probe_policy: ProbePolicy,

    #[command(flatten)]
    pub status_policy: StatusPolicy,
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
        let swept = sweep(&args, d1.as_ref()).await;
        if let Err(e) = &swept {
            // A Tor hiccup or a transient API error must not kill the service;
            // the next sweep gets another go.
            tracing::error!(error = ?e, "sweep failed");
        }

        // Propagated rather than discarded: deploy/pi/entrypoint.sh treats the
        // checker's exit status as the verdict of a --once verification run.
        if args.once {
            return swept;
        }
        tokio::time::sleep(args.interval).await;
    }
}

async fn sweep(args: &Args, d1: Option<&d1::D1Client>) -> Result<()> {
    // Re-read every sweep so adding a link does not need a restart.
    let links = links::load(&args.links)?;
    tracing::info!(count = links.len(), "starting sweep");

    let results = probe::probe_all(&links, &args.socks_addr, &args.probe_policy).await?;

    let mut up = 0;
    for result in &results {
        match &result.outcome {
            Outcome::Reachable { .. } => up += 1,
            Outcome::Unreachable { reason } => {
                tracing::warn!(slug = %result.slug, reason, "link down");
            }
        }
    }
    tracing::info!(up, down = results.len() - up, "sweep complete");

    match d1 {
        Some(d1) => d1.flush(&links, &results, &args.status_policy).await,
        None => {
            tracing::info!("dry run: skipping the D1 write");
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::d1::RECORD_SQL;
    use crate::d1::UPSERT_SQL;
    use crate::links::LinkEntry;
    use crate::links::LinkSet;
    use shared::LinkStatus;
    use std::collections::BTreeMap;
    use std::sync::LazyLock;
    #[test]
    fn status_rule_is_matching() {
        use std::time::Duration;
        let expected_orange_dur = Duration::from_secs(25);
        let expected_red_fails = 4;
        let now = std::time::Instant::now();

        //sleep for 25 and then check baseline known url?
    }
    #[test]
    fn schema_drift_is_guarded() {
        use rusqlite::*;
        use serde_json::{json, Value};

        use crate::probe::ProbeResult;
        let schema = include_str!("../../worker/schema.sql");
        let conn = Connection::open_in_memory().unwrap();
        let actual_upsert_1 = conn.execute(UPSERT_SQL, []).unwrap();
        let actual_record_1 = conn.execute(&RECORD_SQL, []).unwrap();

        conn.execute(
            UPSERT_SQL,
            [
                "NewVariant",
                "http://2gzyxa5ihm7nsggfxnu52rck2vv4rvmdlkiu3zzui5du4xyclen53wid.onion/",
                "NewCategory",
                "New Variant",
            ],
        )
        .unwrap();
    }
}
