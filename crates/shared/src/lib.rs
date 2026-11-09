//! Wire types shared by the checker (writes D1) and the Worker (reads it).
//!
//! These live in one crate so the two sides cannot drift: a category the
//! checker can write is, by construction, one the Worker can render.
//!
//! The one copy this crate does *not* subsume is the `CHECK` constraint in
//! `crates/worker/migrations/0000_initial.sql`. SQLite cannot take its allowed
//! values from Rust, so that list stays hand-maintained -- keep it in step with
//! [`LinkCategory`]. `schema_drift_is_guarded` fails the build if it drifts.

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
SELECT slug, url, category, other_urls, description, status, latency_ms, \
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
    #[serde(default, deserialize_with = "other_urls_from_sql")]
    pub other_urls: Option<Vec<String>>,
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
    urls.filter(|u| !u.is_empty())
        .map(|u| serde_json::to_string(u).expect("a list of strings always serializes"))
}

/// The read half. Accepts the JSON text D1 hands back, and also a plain list,
/// since `Link` serializes it as one and must survive its own round trip.
///
/// A cell that does not parse reads as no mirrors rather than an error: one
/// bad mirrors cell must not fail `get_links` and blank the whole directory.
fn other_urls_from_sql<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<String>>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Json(String),
        List(Vec<String>),
    }
    Ok(match Option::<Raw>::deserialize(d)? {
        None => None,
        Some(Raw::List(urls)) => Some(urls),
        Some(Raw::Json(text)) => serde_json::from_str(&text).ok(),
    }
    .filter(|urls: &Vec<String>| !urls.is_empty()))
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
        let back: Link = serde_json::from_value(serde_json::to_value(row(json!(cell))).unwrap()).unwrap();
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
    fn a_bad_cell_does_not_fail_the_row() {
        assert_eq!(row(json!("http://b.onion/,http://c.onion/")).other_urls, None);
    }
}
