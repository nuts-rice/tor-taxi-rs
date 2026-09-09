//! The write side: D1 over Cloudflare's REST API.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::links::LinkSet;
use crate::probe::ProbeResult;
use crate::status::{ORANGE_STATUS_THRESHOLD, RED_STATUS_THRESHOLD};

/// Brings the row in line with `links.toml`
const UPSERT_SQL: &str = "\
INSERT INTO links (slug, url, category, description) VALUES (?1, ?2, ?3, ?4) \
ON CONFLICT(slug) DO UPDATE SET \
  url = excluded.url, category = excluded.category, description = excluded.description";

/// Applies the status rule.
const RECORD_SQL: &str = "\
UPDATE links SET \
  consecutive_failures = CASE WHEN ?2 = 1 THEN 0 ELSE consecutive_failures + 1 END, \
  latency_ms = ?3, \
  last_good_at = CASE WHEN ?2 = 1 THEN ?4 ELSE last_good_at END, \
  last_checked_at = ?4, \
  status = CASE \
    WHEN ?2 = 1 AND ?3 >= ?5 THEN 'Orange' \
    WHEN ?2 = 1 THEN 'White' \
    WHEN consecutive_failures + 1 >= ?6 THEN 'Red' \
    ELSE 'Orange' \
  END \
WHERE slug = ?1";

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

    /// One request per sweep rather than one per link.
    pub async fn flush(&self, links: &LinkSet, results: &[ProbeResult]) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("system clock is before the unix epoch")?
            .as_secs();

        let orange_ms = ORANGE_STATUS_THRESHOLD.as_millis() as u64;
        let mut batch: Vec<Value> = Vec::with_capacity(results.len() * 2);

        for result in results {
            let Some(link) = links.get(&result.slug) else {
                continue;
            };

            batch.push(json!({
                "sql": UPSERT_SQL,
                "params": [result.slug, link.url, link.category, link.description],
            }));
            batch.push(json!({
                "sql": RECORD_SQL,
                "params": [
                    result.slug,
                    if result.ok { 1 } else { 0 },
                    result.latency_ms,
                    now,
                    orange_ms,
                    RED_STATUS_THRESHOLD,
                ],
            }));
        }

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

        tracing::info!(statements = batch.len(), "flushed to D1");
        Ok(())
    }
}

fn env(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("{key} must be set"))
}
