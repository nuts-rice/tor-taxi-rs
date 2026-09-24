//! The write side: D1 over Cloudflare's REST API.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::LazyLock;

use shared::LinkStatus;

use crate::links::LinkSet;
use crate::probe::ProbeResult;
use crate::status::StatusPolicy;

/// Frees a url still held by a *retired* row, so a re-key or a slug rename can
/// take it without tripping `UNIQUE(url)`.
///
/// Scoped to retired rows on purpose. Unscoped, this drops a live row -- and
/// with it the probe history the soft delete exists to protect -- to make way
/// for whichever slug happened to be upserted second. Two *live* entries
/// sharing a url is an editorial mistake, and `links::load` rejects it before
/// a sweep starts rather than letting it quietly rewrite the table here.
///
/// Must run after `retire_sql` and before `UPSERT_SQL` for the slug; see the
/// ordering rationale in `build_batch`.
pub const RELEASE_URL_SQL: &str =
    "DELETE FROM links WHERE url = ?2 AND slug <> ?1 AND retired_at IS NOT NULL";

/// Brings the row in line with `links.toml`. Must run before RECORD_SQL for a
/// brand-new slug: RECORD_SQL's `consecutive_failures + 1` reads a row that
/// only exists because this statement created it with the column's DEFAULT 0.
///
/// Clearing `retired_at` is the un-delist path: a link that comes back keeps
/// the history it had before it was retired, rather than starting over at
/// consecutive_failures = 0 and claiming a clean record it has not earned.
pub const UPSERT_SQL: &str = "\
INSERT INTO links (slug, url, category, description) VALUES (?1, ?2, ?3, ?4) \
ON CONFLICT(slug) DO UPDATE SET \
  url = excluded.url, category = excluded.category, \
  description = excluded.description, retired_at = NULL";

/// Retires every row whose slug is no longer in `links.toml`.
///
/// Built per sweep because the placeholder count follows the link set: `?1` is
/// the timestamp, `?2..` are the live slugs. `retired_at IS NULL` in the WHERE
/// keeps it idempotent, so the stamp records the first sweep a link went
/// missing rather than being rewritten to `now` on every pass afterwards.
///
/// Self-contained by design -- it names the live set outright instead of
/// relying on some earlier statement in the batch having marked the rows it
/// should spare. A batch that lands only halfway cannot retire a live link.
pub(crate) fn retire_sql(live: usize) -> String {
    let placeholders: Vec<String> = (2..=live + 1).map(|i| format!("?{i}")).collect();
    format!(
        "UPDATE links SET retired_at = ?1 \
         WHERE retired_at IS NULL AND slug NOT IN ({})",
        placeholders.join(", ")
    )
}

/// Applies the status rule. Built once at first use because the status literals
/// come from `LinkStatus::as_sql()` rather than being typed in here.
pub static RECORD_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "UPDATE links SET \
           consecutive_failures = CASE WHEN ?2 = 1 THEN 0 ELSE consecutive_failures + 1 END, \
           latency_ms = ?3, \
           last_good_at = CASE WHEN ?2 = 1 THEN ?4 ELSE last_good_at END, \
           last_checked_at = ?4, \
           status = CASE \
             WHEN ?2 = 1 AND ?3 >= ?5 THEN '{orange}' \
             WHEN ?2 = 1 THEN '{white}' \
             WHEN consecutive_failures + 1 >= ?6 THEN '{red}' \
             ELSE '{orange}' \
           END \
         WHERE slug = ?1",
        white = LinkStatus::White.as_sql(),
        orange = LinkStatus::Orange.as_sql(),
        red = LinkStatus::Red.as_sql(),
    )
});

/// Builds one sweep's statements, in the order they must run.
///
/// Separate from the HTTP send so the ordering can be tested: replaying this
/// against a migrated SQLite database is the only way a reorder here -- which
/// compiles fine and fails only against a real `UNIQUE(url)` -- gets caught
/// before a deploy.
pub(crate) fn build_batch(
    links: &LinkSet,
    results: &[ProbeResult],
    policy: &StatusPolicy,
    now: u64,
) -> Vec<Value> {
    let orange_ms = policy.orange_after.as_millis() as u64;
    let mut batch: Vec<Value> = Vec::with_capacity(results.len() * 3 + 1);

    // First, before any upsert. A slug renamed in links.toml arrives as a new
    // row whose url the *old* row still holds; retiring the old one here is
    // what lets RELEASE_URL_SQL -- which only touches retired rows -- free
    // that url a moment later. Run this last instead and the rename hits
    // UNIQUE(url) and takes the whole batch with it.
    //
    // Guarded on non-empty: an empty link set means links.toml failed to read,
    // not that the directory is empty, and must never retire everything.
    // `sweep` bails before this on a parse error, so reaching here with
    // nothing is already the unexpected case.
    if !links.is_empty() {
        let mut params: Vec<Value> = vec![json!(now)];
        params.extend(links.keys().map(|slug| json!(slug)));
        batch.push(json!({
            "sql": retire_sql(links.len()),
            "params": params,
        }));
    }

    for result in results {
        // Results are derived from `links`, so a miss means the two drifted
        // apart mid-sweep. Say so rather than dropping the row silently.
        let Some(link) = links.get(&result.slug) else {
            tracing::warn!(slug = %result.slug, "probe result has no matching link; skipping");
            continue;
        };

        batch.push(json!({
            "sql": RELEASE_URL_SQL,
            "params": [result.slug, link.url],
        }));
        batch.push(json!({
            "sql": UPSERT_SQL,
            "params": [result.slug, link.url, link.category, link.description],
        }));
        batch.push(json!({
            "sql": RECORD_SQL.as_str(),
            "params": [
                result.slug,
                if result.is_up() { 1 } else { 0 },
                result.latency_ms(),
                now,
                orange_ms,
                policy.red_after,
            ],
        }));
    }

    batch
}

pub struct D1Client {
    http: reqwest::Client,
    endpoint: String,
    token: String,
}

#[derive(Deserialize)]
struct Envelope {
    success: bool,
    #[serde(default)]
    errors: Vec<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    code: i64,
    message: String,
}

impl D1Client {
    pub fn from_env() -> Result<Self> {
        let account = env("CLOUDFLARE_ACCOUNT_ID")?;
        let database = env("CLOUDFLARE_D1_DATABASE_ID")?;
        let token = env("CLOUDFLARE_API_TOKEN")?;

        Ok(Self {
            // A plain client: this talks to Cloudflare directly, NOT through Tor.
            http: reqwest::Client::new(),
            endpoint: format!(
                "https://api.cloudflare.com/client/v4/accounts/{account}/d1/database/{database}/query"
            ),
            token,
        })
    }

    pub async fn flush(
        &self,
        links: &LinkSet,
        results: &[ProbeResult],
        policy: &StatusPolicy,
    ) -> Result<()> {
        tracing::info!(count = links.len(), "✏️ starting write to d1");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("system clock is before the unix epoch")?
            .as_secs();

        let batch = build_batch(links, results, policy, now);

        if batch.is_empty() {
            return Ok(());
        }

        let resp = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.token)
            .json(&json!({ "batch": batch }))
            .send()
            .await
            .context("D1 query request failed")?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        // Cloudflare answers 200 with success:false for SQL-level errors, so the
        // HTTP status alone is not enough to know the write landed.
        let envelope: Envelope = serde_json::from_str(&body)
            .with_context(|| format!("unexpected D1 response (HTTP {status}): {body}"))?;

        if !envelope.success {
            let detail = envelope
                .errors
                .iter()
                .map(|e| format!("[{}] {}", e.code, e.message))
                .collect::<Vec<_>>()
                .join("; ");
            bail!("D1 rejected the batch: {detail}");
        }
        let after_flush_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        tracing::info!("✅ flush complete in {:?}s", after_flush_now - now);
        tracing::info!(statements = batch.len(), "flushed to D1");
        Ok(())
    }
}

fn env(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("{key} must be set"))
}
