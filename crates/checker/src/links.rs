//! The canonical link set, read from `links.toml`.
//!
//! Git-tracked on purpose: adding a link to a darknet directory should cost a
//! commit and a review, because the directory's whole value is editorial trust.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};

pub use shared::LinkEntry;

/// Keyed by slug -- the table names in `links.toml` (`[dread]`, ...) -- which is
/// also the primary key in D1. A `BTreeMap` rather than a `HashMap` so sweeps
/// probe in a stable order and the logs are diffable between runs.
pub type LinkSet = BTreeMap<String, LinkEntry>;

/// Category validation is serde's job: [`LinkEntry::category`] is an enum, so an
/// unknown category fails here with `unknown variant ...` naming the offending
/// table, rather than as a row D1 rejects mid-sweep.
pub fn load(path: &Path) -> Result<LinkSet> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading link set from {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}
