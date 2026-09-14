use leptos::prelude::*;

pub use shared::Link;

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
                <span class="category">{link.category.label()}</span>
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
