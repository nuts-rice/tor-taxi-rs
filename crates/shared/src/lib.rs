//! Wire types shared by the checker (writes D1) and the Worker (reads it).
//!
//! These live in one crate so the two sides cannot drift: a category the
//! checker can write is, by construction, one the Worker can render.
//!
//! The one copy this crate does *not* subsume is the `CHECK` constraint in
//! `crates/worker/migrations/0000_initial.sql`. SQLite cannot take its allowed
//! values from Rust, so that list stays hand-maintained -- keep it in step with
//! [`LinkCategory`]. `schema_drift_is_guarded` fails the build if it drifts.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

/// The Worker's one query, and the read half of the schema contract.
///
/// Here rather than inline in `crates/worker/src/api.rs` so the checker's
/// `schema_drift_is_guarded` test can prepare it against the real migrations.
/// The Worker cannot be tested against SQLite directly -- it only compiles to
/// wasm32 -- so this const is the only way the read side gets checked at all.
///
/// Column order and names match [`Link`]'s fields: D1 deserializes rows into it
/// by name, so renaming a field without renaming it here fails at runtime.
///
/// `retired_at IS NULL` is the delisting filter. A retired row keeps its probe
/// history but stops rendering; see `0001_add_retired_at.sql`.
pub const SELECT_LINKS_SQL: &str = "\
SELECT slug, url, category, other_urls, url_status, description, status, latency_ms, \
       (strftime('%s', 'now') - last_checked_at) AS checked_ago_secs, \
    (SELECT CAST(round(p.avg_latency_ms) AS INTEGER) \
        FROM probes p \
    WHERE p.slug = links.slug \
    ORDER BY p.checked_at DESC LIMIT 1) AS avg_latency_ms \
  FROM links \
 WHERE retired_at IS NULL \
 ORDER BY category, slug";

/// Mirrors the `CHECK (category IN (...))` list in `0000_initial.sql`.
///
/// Serde matches variant names exactly, so the SQL spelling and the Rust
/// spelling are the same string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum LinkCategory {
    News,
    Search,
    Email,
    Market,
    Exchange,
    ImageUpload,
    Info,
    Escrow,
    Forum,
    Service,
    Unknown,
}

impl LinkCategory {
    /// How the category reads on the page. Separate from the variant name so
    /// the wire format can stay `ImageUpload` while the reader sees two words.
    pub fn label(self) -> &'static str {
        match self {
            LinkCategory::News => "News",
            LinkCategory::Search => "Search",
            LinkCategory::Email => "Email",
            LinkCategory::Market => "Market",
            LinkCategory::Exchange => "Exchange",
            LinkCategory::ImageUpload => "Image Upload",
            LinkCategory::Info => "Info",
            LinkCategory::Escrow => "Escrow",
            LinkCategory::Forum => "Forum",
            LinkCategory::Service => "Service",
            LinkCategory::Unknown => "Unknown",
        }
    }
}

/// The dot next to a link. See `crate::status` in the checker for the rule that
/// picks one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum LinkStatus {
    White,
    Orange,
    Red,
}

impl LinkStatus {
    pub fn css_class(self) -> &'static str {
        match self {
            LinkStatus::Red => "status red",
            LinkStatus::Orange => "status orange",
            LinkStatus::White => "status white",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LinkStatus::Red => "down",
            LinkStatus::Orange => "degraded",
            LinkStatus::White => "up",
        }
    }

    /// The literal the checker writes into the `status` column. Built from the
    /// enum rather than typed into the SQL, so renaming a variant cannot leave
    /// the write side emitting a string the read side no longer accepts.
    pub fn as_sql(self) -> &'static str {
        match self {
            LinkStatus::Red => "Red",
            LinkStatus::Orange => "Orange",
            LinkStatus::White => "White",
        }
    }
}

/// What a reachable page's body said about who is serving it, checked against
/// `expected_content` in links.toml.
///
/// Three-valued on purpose. Onion services often answer through an anti-DDoS
/// queue page that carries none of the real site's text, and treating that as
/// a mismatch would flag every busy service as an impostor. `Inconclusive` is
/// "we got a page, but not one that can tell us anything".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ContentCheck {
    Match,
    Mismatch,
    Inconclusive,
}

/// The latest sweep's result for one address -- a link's primary url or one
/// of its mirrors.
///
/// A single sample, not a status: the consecutive-failure rule that turns
/// probes into a dot applies to the link as a whole, not to each address.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct UrlStatus {
    pub url: String,
    pub up: bool,
    pub latency_ms: Option<u64>,
    /// `None` when the link has no `expected_content`, or the address was down.
    #[serde(default)]
    pub content: Option<ContentCheck>,
}

/// One entry in `crates/checker/links.toml` -- the editorial input.

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkEntry {
    pub url: String,
    pub other_urls: Option<Vec<String>>,
    pub category: LinkCategory,
    #[serde(default)]
    pub description: String,
    // Optional for asserting matches etc...
    pub expected_content: Option<String>,
}

/// One row of the `links` table -- the rendered output.
///
/// Field names match the columns selected in the Worker's `get_links`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Link {
    pub slug: String,
    pub url: String,
    pub category: LinkCategory,
    pub description: String,
    /// Mirrors of the same service. Stored as a JSON array in a TEXT column --
    /// SQLite has no array type, and `TEXT[]` in `0004_add_urls_to_links.sql`
    /// is just TEXT affinity. See [`other_urls_to_sql`].
    #[serde(default, deserialize_with = "json_list_from_sql")]
    pub other_urls: Option<Vec<String>>,
    /// Per-address results from the latest sweep, primary first. Same JSON
    /// encoding as `other_urls`; see [`json_list_to_sql`].
    #[serde(default, deserialize_with = "json_list_from_sql")]
    pub url_status: Option<Vec<UrlStatus>>,
    /// `None` until the checker has probed it at least once. Not the same as
    /// down -- we simply do not know yet.
    pub status: Option<LinkStatus>,
    pub latency_ms: Option<u64>,
    pub checked_ago_secs: Option<i64>,
    pub avg_latency_ms: Option<u64>,
}

impl Link {
    pub fn slug(&self) -> &str {
        &self.slug
    }

    pub fn status(&self) -> &Option<LinkStatus> {
        &self.status
    }

    pub fn latency_ms(&self) -> &Option<u64> {
        &self.latency_ms
    }
}

/// The write half of the `other_urls` encoding: a JSON array, or NULL when
/// there are none. An empty list is stored as NULL so "no mirrors" has one
/// spelling in the table, not two.
pub fn other_urls_to_sql(urls: Option<&[String]>) -> Option<String> {
    json_list_to_sql(urls)
}

/// The write half of every JSON-list column (`other_urls`, `url_status`).
pub fn json_list_to_sql<T: Serialize>(items: Option<&[T]>) -> Option<String> {
    items
        .filter(|i| !i.is_empty())
        .map(|i| serde_json::to_string(i).expect("plain data always serializes"))
}

/// The read half. Accepts the JSON text D1 hands back, and also a plain list,
/// since `Link` serializes it as one and must survive its own round trip.
///
/// A cell that does not parse reads as empty rather than an error: one bad
/// cell must not fail `get_links` and blank the whole directory.
fn json_list_from_sql<'de, D, T>(d: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw<T> {
        Json(String),
        List(Vec<T>),
    }
    Ok(match Option::<Raw<T>>::deserialize(d)? {
        None => None,
        Some(Raw::List(items)) => Some(items),
        Some(Raw::Json(text)) => serde_json::from_str(&text).ok(),
    }
    .filter(|items: &Vec<T>| !items.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(other_urls: serde_json::Value) -> Link {
        serde_json::from_value(json!({
            "slug": "a", "url": "http://a.onion/", "category": "Info",
            "description": "", "other_urls": other_urls, "status": null,
            "latency_ms": null, "checked_ago_secs": null, "avg_latency_ms": null,
        }))
        .unwrap()
    }

    #[test]
    fn other_urls_round_trips_through_its_column() {
        let urls = vec!["http://b.onion/".to_string(), "http://c.onion/".to_string()];
        let cell = other_urls_to_sql(Some(&urls)).unwrap();
        assert_eq!(row(json!(cell)).other_urls, Some(urls.clone()));
        // And through Link's own Serialize, which emits a list.
        let back: Link =
            serde_json::from_value(serde_json::to_value(row(json!(cell))).unwrap()).unwrap();
        assert_eq!(back.other_urls, Some(urls));
    }

    #[test]
    fn no_mirrors_has_one_spelling() {
        assert_eq!(other_urls_to_sql(None), None);
        assert_eq!(other_urls_to_sql(Some(&[])), None);
        assert_eq!(row(json!(null)).other_urls, None);
        assert_eq!(row(json!("[]")).other_urls, None);
    }

    #[test]
    fn url_status_round_trips_through_its_column() {
        let statuses = vec![
            UrlStatus {
                url: "http://a.onion/".into(),
                up: false,
                latency_ms: None,
                content: None,
            },
            UrlStatus {
                url: "http://b.onion/".into(),
                up: true,
                latency_ms: Some(900),
                content: Some(ContentCheck::Mismatch),
            },
        ];
        let cell = json_list_to_sql(Some(&statuses)).unwrap();
        let r: Link = serde_json::from_value(json!({
            "slug": "a", "url": "http://a.onion/", "category": "Info",
            "description": "", "other_urls": null, "url_status": cell, "status": null,
            "latency_ms": null, "checked_ago_secs": null, "avg_latency_ms": null,
        }))
        .unwrap();
        assert_eq!(r.url_status, Some(statuses));
    }

    #[test]
    fn a_bad_cell_does_not_fail_the_row() {
        assert_eq!(
            row(json!("http://b.onion/,http://c.onion/")).other_urls,
            None
        );
    }
}
