//! The canonical link set, read from `links.toml`.
//!
//! Git-tracked on purpose: adding a link to a darknet directory should cost a
//! commit and a review, because the directory's whole value is editorial trust.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};

use data_encoding::BASE32_NOPAD;
use reqwest::Url;
use sha3::{Digest, Sha3_256};
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
    let set: LinkSet =
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    check_unique_urls(&set).with_context(|| format!("parsing {}", path.display()))?;
    validate_set(&set).with_context(|| format!("validating {}", path.display()))?;
    Ok(set)
}

fn validate_set(set: &LinkSet) -> Result<()> {
    for (slug, entry) in set {
        validate_onion_address(&entry.url).with_context(|| format!("`{slug}`: {}", entry.url))?;
    }
    Ok(())
}

fn validate_onion_address(raw: &str) -> Result<()> {
    const CHECKSUM_PREFIX: &[u8] = b".onion checksum";
    const VERSION: u8 = 3;
    let url = Url::parse(raw).context("not an url ")?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("scheme must be http or https: {}", url.scheme());
    };

    let host = url.host_str().context("no host")?;
    let Some(rest) = host.strip_suffix(".onion") else {
        bail!("{host} is not .onion url");
    };
    let label = rest.rsplit(".").next().unwrap_or(rest);
    match label.len() {
        56 => {}
        n => bail!("{host} is {n} characters. v3 addresses are 56."),
    };

    let bytes = BASE32_NOPAD
        .decode(label.to_ascii_uppercase().as_bytes())
        .with_context(|| format!("{label} is not bse32"))?;

    let (pubkey, tail) = bytes.split_at(32);
    let (checksum, version) = (&tail[..2], tail[2]);
    if version != VERSION {
        bail!("{label} claims {version} not {VERSION} ");
    }

    let digest = Sha3_256::new()
        .chain_update(CHECKSUM_PREFIX)
        .chain_update(pubkey)
        .chain_update([VERSION])
        .finalize();
    if checksum != &digest[..2] {
        bail!("{label} fails checksum -- maybe a typo?  ");
    }
    Ok(())
}

/// Rejects two entries claiming the same url.
///
/// D1 enforces this too, as `UNIQUE(url)` -- but it enforces it by rejecting
/// the whole batch, so one duplicate costs *every* link's status update, every
/// sweep, until someone reads the journal. The checker's own retry does not
/// help: the next sweep sends the identical batch. Failing here instead means
/// a bad edit is caught at load, by the process that can name both slugs.
fn check_unique_urls(set: &LinkSet) -> Result<()> {
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for (slug, entry) in set {
        if let Some(first) = seen.insert(entry.url.as_str(), slug.as_str()) {
            bail!("`{first}` and `{slug}` both claim {}", entry.url);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_str(body: &str) -> Result<LinkSet> {
        let path =
            std::env::temp_dir().join(format!("links-{}-{:p}.toml", std::process::id(), body));
        std::fs::write(&path, body).unwrap();
        let out = load(&path);
        let _ = std::fs::remove_file(&path);
        out
    }

    #[test]
    fn a_duplicate_url_is_rejected_by_name() {
        let err = load_str(
            r#"
[dread]
url = "http://d.onion/"
category = "Forum"

[dread_mirror]
url = "http://d.onion/"
category = "Forum"
"#,
        )
        .unwrap_err();

        // Both slugs, because the point is to say which two lines to look at.
        let msg = format!("{err:#}");
        assert!(
            msg.contains("dread") && msg.contains("dread_mirror"),
            "{msg}"
        );
    }

    #[test]
    fn an_unknown_category_names_the_offending_table() {
        let err = load_str("[x]\nurl = \"http://x.onion/\"\ncategory = \"Forumm\"\n").unwrap_err();
        assert!(format!("{err:#}").contains("Forumm"));
    }

    #[test]
    fn distinct_urls_load() {
        let set = load_str(
            r#"
[a]
url = "http://a.onion/"
category = "Info"

[b]
url = "http://b.onion/"
category = "News"
description = "b"
"#,
        )
        .unwrap();
        assert_eq!(set.len(), 2);
    }

    /// The real link set has to satisfy the same rule.
    #[test]
    fn the_shipped_links_toml_is_valid() {
        load(std::path::Path::new("links.toml")).unwrap();
    }
}
