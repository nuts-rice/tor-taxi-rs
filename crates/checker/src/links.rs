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
    parse(&raw).with_context(|| format!("loading {}", path.display()))
}

/// Everything [`load`] does after the read. Split out so the fuzz targets and
/// property tests can drive it without touching the filesystem.
pub fn parse(raw: &str) -> Result<LinkSet> {
    let set: LinkSet = toml::from_str(raw).context("parsing")?;
    check_unique_urls(&set).context("parsing")?;
    validate_set(&set).context("validating")?;
    Ok(set)
}

pub fn validate_set(set: &LinkSet) -> Result<()> {
    for (slug, entry) in set {
        validate_onion_address(&entry.url).with_context(|| format!("`{slug}`: {}", entry.url))?;
        // Mirrors are probed and rendered as links too, so they meet the same bar.
        for mirror in entry.other_urls.iter().flatten() {
            validate_onion_address(mirror).with_context(|| format!("`{slug}` mirror: {mirror}"))?;
        }
    }
    Ok(())
}

pub fn validate_onion_address(raw: &str) -> Result<()> {
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
pub fn check_unique_urls(set: &LinkSet) -> Result<()> {
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

    /// A real v3 onion label for `pubkey`, built the way Tor builds one. The
    /// encode direction, written independently of the decode in
    /// [`validate_onion_address`], so the two can check each other.
    pub(super) fn v3_label(pubkey: &[u8; 32]) -> String {
        let checksum = Sha3_256::new()
            .chain_update(b".onion checksum")
            .chain_update(pubkey)
            .chain_update([3u8])
            .finalize();
        let mut bytes = pubkey.to_vec();
        bytes.extend_from_slice(&checksum[..2]);
        bytes.push(3);
        BASE32_NOPAD.encode(&bytes).to_ascii_lowercase()
    }

    pub(super) fn v3_url(pubkey: &[u8; 32]) -> String {
        format!("http://{}.onion/", v3_label(pubkey))
    }

    #[test]
    fn a_duplicate_url_is_rejected_by_name() {
        let err = parse(
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
        let err = parse("[x]\nurl = \"http://x.onion/\"\ncategory = \"Forumm\"\n").unwrap_err();
        assert!(format!("{err:#}").contains("Forumm"));
    }

    #[test]
    fn distinct_urls_load() {
        let set = parse(&format!(
            r#"
[a]
url = "{}"
category = "Info"

[b]
url = "{}"
category = "News"
description = "b"
"#,
            v3_url(&[1; 32]),
            v3_url(&[2; 32]),
        ))
        .unwrap();
        assert_eq!(set.len(), 2);
    }

    /// The real link set has to satisfy the same rule.
    #[test]
    fn the_shipped_links_toml_is_valid() {
        load(std::path::Path::new("links.toml")).unwrap();
    }
}

#[cfg(test)]
mod props {
    use super::tests::{v3_label, v3_url};
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Any real v3 address must pass validation.
        #[test]
        fn a_real_v3_address_is_accepted(key in any::<[u8; 32]>()) {
            prop_assert!(validate_onion_address(&v3_url(&key)).is_ok())
        }

        #[test]
        fn a_real_v3_address_with_either_url_scheme_is_accepted(https in any::<bool>(), key in any::<[u8; 32]>()) {
            let label = v3_label(&key);
            let url_scheme = if https {"https"} else {"http"};
            let url = format!("{url_scheme}://{}.onion", label);
            let res = validate_onion_address(&url);
            prop_assert!(res.is_ok(), "{url}: {:#}", res.unwrap_err());
        }
        #[test]
        fn a_typo_in_checksum_is_rejected(
            key in any::<[u8; 32]>(),
            pos in 52usize..56,
            shift in 1usize..32,
        )  {
            const BASE32: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
            let label = v3_label(&key).into_bytes();
        }
    }
}
