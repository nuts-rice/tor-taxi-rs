//! Wire types shared by the checker (writes D1) and the Worker (reads it).
//!
//! These live in one crate so the two sides cannot drift: a category the
//! checker can write is, by construction, one the Worker can render.
//!
//! The one copy this crate does *not* subsume is the `CHECK` constraint in
//! `crates/worker/schema.sql`. SQLite cannot take its allowed values from Rust,
//! so that list stays hand-maintained -- keep it in step with [`LinkCategory`].

use serde::{Deserialize, Serialize};

/// Mirrors the `CHECK (category IN (...))` list in `schema.sql`.
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
            _ => "Unknown",
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
///
/// `category` is the enum, not a `String`, so a typo fails at load with serde's
/// own "unknown variant `Forumm`, expected one of ..." rather than as a row
/// rejected by D1 halfway through a sweep.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LinkEntry {
    pub url: String,
    pub category: LinkCategory,
    #[serde(default)]
    pub description: String,
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
    /// `None` until the checker has probed it at least once. Not the same as
    /// down -- we simply do not know yet.
    pub status: Option<LinkStatus>,
    pub latency_ms: Option<u64>,
    pub checked_ago_secs: Option<i64>,
}
