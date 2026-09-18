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
    use crate::d1::{build_batch, retire_sql, RECORD_SQL, RELEASE_URL_SQL, UPSERT_SQL};
    use crate::links::LinkSet;
    use crate::probe::{Outcome, ProbeResult};
    use crate::status::StatusPolicy;
    use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
    use serde_json::Value;
    use shared::{LinkCategory, LinkEntry, SELECT_LINKS_SQL};
    use std::time::Duration;

    /// Every migration, in the order wrangler applies them. Listed rather than
    /// globbed because `include_str!` is a compile-time read: a new migration
    /// has to be added here, which is the moment to ask whether it breaks any
    /// of the statements below.
    const MIGRATIONS: &[&str] = &[
        include_str!("../../worker/migrations/0000_initial.sql"),
        include_str!("../../worker/migrations/0001_add_retired_at.sql"),
    ];

    const POLICY: StatusPolicy = StatusPolicy {
        orange_after: Duration::from_secs(10),
        red_after: 3,
    };

    /// A database in the shape production is actually in.
    fn migrated() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        for (i, m) in MIGRATIONS.iter().enumerate() {
            conn.execute_batch(m)
                .unwrap_or_else(|e| panic!("migration {i:04} does not apply: {e}"));
        }
        conn
    }

    /// Runs one sweep by replaying what `build_batch` actually produced.
    ///
    /// Deliberately not a hand-written sequence of statements: the ordering is
    /// the thing under test, and a test that restates it would pass against a
    /// `flush` that had been reordered underneath it.
    ///
    /// `toml` is the link set as links.toml spells it; `probe` is what the
    /// network said about each one.
    fn sweep(
        conn: &Connection,
        toml: &[(&str, &str)],
        probe: &[(&str, bool, Option<u64>)],
        now: u64,
    ) {
        let links: LinkSet = toml
            .iter()
            .map(|(slug, url)| {
                (
                    slug.to_string(),
                    LinkEntry {
                        url: url.to_string(),
                        category: LinkCategory::Info,
                        description: "desc".into(),
                    },
                )
            })
            .collect();

        let results: Vec<ProbeResult> = probe
            .iter()
            .map(|(slug, up, latency)| ProbeResult {
                slug: slug.to_string(),
                outcome: if *up {
                    Outcome::Reachable {
                        latency: Duration::from_millis(latency.expect("an up probe has a latency")),
                    }
                } else {
                    Outcome::Unreachable {
                        reason: "test".into(),
                    }
                },
            })
            .collect();

        for stmt in build_batch(&links, &results, &POLICY, now) {
            let sql = stmt["sql"].as_str().unwrap();
            let bound: Vec<SqlValue> = stmt["params"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| match v {
                    Value::Null => SqlValue::Null,
                    Value::Number(n) => SqlValue::Integer(n.as_i64().unwrap()),
                    Value::String(s) => SqlValue::Text(s.clone()),
                    other => panic!("unexpected bound parameter {other}"),
                })
                .collect();
            conn.execute(sql, params_from_iter(bound))
                .unwrap_or_else(|e| panic!("{sql}\n  failed: {e}"));
        }
    }

    fn status_of(conn: &Connection, slug: &str) -> Option<String> {
        conn.query_row(
            "SELECT status FROM links WHERE slug = ?1",
            params![slug],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// The point of this test: every statement the two sides run is prepared
    /// against the real migrations. All of it is SQL in Rust string literals,
    /// so none of it is checked by the compiler -- a clause in the wrong order
    /// or a missing comma reaches production as a healthy-looking binary whose
    /// every write silently fails.
    #[test]
    fn schema_drift_is_guarded() {
        let conn = migrated();
        for (name, sql) in [
            ("RELEASE_URL_SQL", RELEASE_URL_SQL.to_string()),
            ("UPSERT_SQL", UPSERT_SQL.to_string()),
            ("RECORD_SQL", RECORD_SQL.to_string()),
            ("retire_sql", retire_sql(3)),
            // The Worker's read. It only compiles to wasm32 and cannot be
            // tested in place, so this is the one thing standing between a
            // malformed SELECT and a blank directory.
            ("SELECT_LINKS_SQL", SELECT_LINKS_SQL.to_string()),
        ] {
            conn.prepare(&sql)
                .unwrap_or_else(|e| panic!("{name} does not match the schema: {e}"));
        }
    }

    /// The CHECK list in 0000_initial.sql is the one copy of the category set
    /// that Rust cannot own, so it is the one that can drift.
    #[test]
    fn schema_rejects_an_unknown_category() {
        let conn = migrated();
        let err = conn.execute(
            UPSERT_SQL,
            params!["new", "http://x.onion/", "NewCategory", ""],
        );
        assert!(
            err.is_err(),
            "the CHECK constraint accepted a category no LinkCategory variant spells"
        );
    }

    /// The table in status.rs, as SQL.
    #[test]
    fn status_rule_is_matching() {
        let conn = migrated();
        const A: &[(&str, &str)] = &[("a", "http://a.onion/")];

        sweep(&conn, A, &[("a", true, Some(200))], 1_000);
        assert_eq!(
            status_of(&conn, "a").as_deref(),
            Some("White"),
            "reachable and fast"
        );

        sweep(&conn, A, &[("a", true, Some(25_000))], 2_000);
        assert_eq!(
            status_of(&conn, "a").as_deref(),
            Some("Orange"),
            "reachable but over orange_after"
        );

        // Failures below red_after are Orange: one missed sweep is not an outage.
        for (i, now) in [3_000, 4_000].iter().enumerate() {
            sweep(&conn, A, &[("a", false, None)], *now);
            assert_eq!(
                status_of(&conn, "a").as_deref(),
                Some("Orange"),
                "failure {}",
                i + 1
            );
        }

        sweep(&conn, A, &[("a", false, None)], 5_000);
        assert_eq!(
            status_of(&conn, "a").as_deref(),
            Some("Red"),
            "at red_after consecutive failures"
        );

        // last_good_at must still point at the last sweep that actually
        // succeeded -- it is the only evidence the service ever worked.
        let (good, failures): (i64, i64) = conn
            .query_row(
                "SELECT last_good_at, consecutive_failures FROM links WHERE slug = 'a'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(good, 2_000);
        assert_eq!(failures, 3);
    }

    /// Delisting: the row stops rendering but keeps everything it learned.
    #[test]
    fn retiring_hides_a_link_without_losing_its_history() {
        const A: &[(&str, &str)] = &[("a", "http://a.onion/")];
        const AB: &[(&str, &str)] = &[("a", "http://a.onion/"), ("b", "http://b.onion/")];
        let conn = migrated();
        sweep(
            &conn,
            AB,
            &[("a", true, Some(200)), ("b", false, None)],
            1_000,
        );

        // b drops out of links.toml; only a is swept.
        sweep(&conn, A, &[("a", true, Some(200))], 2_000);

        let visible: Vec<String> = conn
            .prepare(SELECT_LINKS_SQL)
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(visible, ["a"], "a retired link must not render");

        let (retired, failures): (Option<i64>, i64) = conn
            .query_row(
                "SELECT retired_at, consecutive_failures FROM links WHERE slug = 'b'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(retired, Some(2_000));
        assert_eq!(failures, 1, "the probe history must survive the delisting");

        // Stamped once: a later sweep must not keep moving the date.
        sweep(&conn, A, &[("a", true, Some(200))], 3_000);
        let retired: Option<i64> = conn
            .query_row("SELECT retired_at FROM links WHERE slug = 'b'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            retired,
            Some(2_000),
            "retired_at records when it went missing, not when we last looked"
        );
    }

    /// Re-listing is the reason this is a soft delete.
    #[test]
    fn relisting_restores_the_row_with_its_history() {
        const A: &[(&str, &str)] = &[("a", "http://a.onion/")];
        const AB: &[(&str, &str)] = &[("a", "http://a.onion/"), ("b", "http://b.onion/")];
        let conn = migrated();
        sweep(&conn, AB, &[("b", false, None)], 1_000);
        sweep(&conn, A, &[("a", true, Some(200))], 2_000);

        sweep(&conn, AB, &[("b", false, None)], 3_000);

        let (retired, failures): (Option<i64>, i64) = conn
            .query_row(
                "SELECT retired_at, consecutive_failures FROM links WHERE slug = 'b'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(retired, None, "the upsert must clear retired_at");
        assert_eq!(
            failures, 2,
            "a re-listed link keeps its record rather than starting clean"
        );
    }

    /// The bug this ordering exists to prevent: a slug rename arrives as a new
    /// row whose url the old row still holds, and UNIQUE(url) rejects the whole
    /// batch -- so no link's status lands, every sweep, until someone notices.
    #[test]
    fn a_slug_rename_does_not_break_the_batch() {
        let conn = migrated();
        sweep(
            &conn,
            &[("dread", "http://d.onion/")],
            &[("dread", true, Some(200))],
            1_000,
        );

        // links.toml now spells it dread_forum, at the same address.
        sweep(
            &conn,
            &[("dread_forum", "http://d.onion/")],
            &[("dread_forum", true, Some(200))],
            2_000,
        );

        let rows: Vec<(String, String)> = conn
            .prepare("SELECT slug, url FROM links ORDER BY slug")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            [("dread_forum".to_string(), "http://d.onion/".to_string())]
        );
    }

    /// A live row must never be deleted to make room. Two live entries sharing
    /// a url is an editorial mistake, caught in `links::load`; if it ever got
    /// this far it has to fail loudly rather than quietly drop a row.
    #[test]
    fn release_url_spares_a_live_row() {
        let conn = migrated();
        sweep(
            &conn,
            &[("a", "http://shared.onion/")],
            &[("a", true, Some(200))],
            1_000,
        );

        conn.execute(RELEASE_URL_SQL, params!["b", "http://shared.onion/"])
            .unwrap();
        let live: i64 = conn
            .query_row("SELECT count(*) FROM links WHERE slug = 'a'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(live, 1, "a live row was deleted to free its url");

        assert!(
            conn.execute(UPSERT_SQL, params!["b", "http://shared.onion/", "Info", ""])
                .is_err(),
            "a duplicate url must fail rather than silently rewrite the table"
        );
    }
}
