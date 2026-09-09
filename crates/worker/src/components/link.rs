use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
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
}

/// One row of the `links` table. Field names match the columns selected in
/// [`crate::api::get_links`]; the `CHECK` constraints in `schema.sql` are what
/// guarantee `category` and `status` deserialize.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Link {
    pub slug: String,
    pub url: String,
    pub category: LinkCategory,
    pub description: String,
    /// `None` until the checker has probed it at least once. Not the same as
    /// down -- we simply do not know yet.
    pub status: Option<Status>,
    pub latency_ms: Option<u64>,
    pub checked_ago_secs: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Copy)]
pub enum Status {
    Red,
    Orange,
    White,
}

impl Status {
    fn css_class(self) -> &'static str {
        match self {
            Status::Red => "status red",
            Status::Orange => "status orange",
            Status::White => "status white",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Status::Red => "down",
            Status::Orange => "degraded",
            Status::White => "up",
        }
    }
}

fn humanize(secs: i64) -> String {
    match secs {
        s if s < 0 => "just now".to_string(),
        s if s < 90 => format!("{s}s ago"),
        s if s < 5400 => format!("{}m ago", s / 60),
        s => format!("{}h ago", s / 3600),
    }
}

/// Purely presentational -- the whole page is fetched once by `HomePage`.
#[component]
pub fn ShowLink(link: Link) -> impl IntoView {
    let (class, label) = match link.status {
        Some(s) => (s.css_class(), s.label()),
        None => ("status unknown", "not yet checked"),
    };

    let freshness = link.checked_ago_secs.map(humanize);
    let latency = link.latency_ms.map(|ms| format!("{ms} ms"));

    view! {
        <li class="link">
            <span class=class title=label></span>
            <div class="body">
                <a href=link.url.clone() rel="noopener noreferrer">{link.slug.clone()}</a>
                <span class="category">{format!("{:?}", link.category)}</span>
                <p class="description">{link.description.clone()}</p>
                <p class="meta">
                    {label}
                    {latency.map(|l| format!(" · {l}"))}
                    // Freshness is the product: a stale dot is worth less than
                    // no dot, so say out loud how old this reading is.
                    {freshness.map(|f| format!(" · checked {f}"))}
                </p>
            </div>
        </li>
    }
}
