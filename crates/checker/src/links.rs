//! The canonical link set, read from `links.toml`.
//!
//! Git-tracked on purpose: adding a link to a darknet directory should cost a
//! commit and a review, because the directory's whole value is editorial trust.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Mirrors the `LinkCategory` variants in the worker, and the `CHECK` constraint
/// in `schema.sql`. Validated on load so a typo fails here, loudly and locally,
/// rather than as a rejected row halfway through a sweep.
const CATEGORIES: &[&str] = &[
    "News",
    "Search",
    "Email",
    "Market",
    "Exchange",
    "ImageUpload",
    "Info",
    "Escrow",
    "Forum",
    "Service",
];

#[derive(Debug, Clone, Deserialize)]
pub struct LinkEntry {
    pub url: String,
    pub category: String,
    #[serde(default)]
    pub description: String,
}

/// Keyed by slug -- the table names in `links.toml` (`[dread]`, ...) -- which is
/// also the primary key in D1. A `BTreeMap` rather than a `HashMap` so sweeps
/// probe in a stable order and the logs are diffable between runs.
pub type LinkSet = BTreeMap<String, LinkEntry>;

pub fn load(path: &Path) -> Result<LinkSet> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading link set from {}", path.display()))?;
    let links: LinkSet =
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;

    for (slug, link) in &links {
        if !CATEGORIES.contains(&link.category.as_str()) {
            bail!(
                "link `{slug}` has unknown category `{}`; expected one of {}",
                link.category,
                CATEGORIES.join(", ")
            );
        }
    }

    Ok(links)
}
