use leptos::prelude::*;

pub use shared::{Link, UrlStatus};

fn humanize(secs: i64) -> String {
    match secs {
        s if s < 0 => "just now".to_string(),
        s if s < 90 => format!("{s}s ago"),
        s if s < 5400 => format!("{}m ago", s / 60),
        s => format!("{}h ago", s / 3600),
    }
}

fn address_dot(s: Option<&UrlStatus>) -> (&'static str, &'static str) {
    match s {
        None => ("status unknown", "not yet checked"),
        Some(s) if s.up => ("status white", "up since last checked"),
        Some(_) => ("status orange", "unreachable since last checked"),
    }
}

/// Purely presentational -- the whole page is fetched once by `HomePage`.
/// Takes the link by value and moves its strings into the view, so a row costs
/// no copies of its own.
#[component]
pub fn ShowLink(link: Link) -> impl IntoView {
    let Link {
        slug,
        url,
        description,
        other_urls,
        status,
        url_status,
        latency_ms,
        avg_latency_ms,
        checked_ago_secs,
        ..
    } = link;

    let (class, label) = match status {
        Some(s) => (s.css_class(), s.label()),
        None => ("status unknown", "not yet checked"),
    };

    let latency = latency_ms.map(|ms| match avg_latency_ms {
        Some(avg) => format!(" · {ms} ms (avg {avg})"),
        None => format!(" · {ms} ms"),
    });
    // Freshness is the product: a stale dot is worth less than no dot, so say
    // out loud how old this reading is.
    let freshness = checked_ago_secs.map(|s| format!(" · checked {}", humanize(s)));

    let statuses = url_status.unwrap_or_default();
    let state_of = |u: &str| statuses.iter().find(|s| s.url == u);

    let mirror_being_used = (state_of(&url).is_some_and(|s| !s.up)
        && statuses.iter().any(|s| s.up))
    .then_some("· primary url unreachable, listing mirror");

    let mirrors = other_urls.filter(|urls| !urls.is_empty()).map(|urls| {
        view! {
            <details class="mirrors">
                <summary>{format!("{} mirror{}", urls.len(), if urls.len() == 1 { "" } else { "s" })}</summary>
                <ul>
                    {urls
                        .into_iter()
                        .map(|u| {
                            let (dot, dot_label) = address_dot(state_of(&u));
                            let href = u.clone();
                            view! {
                                <li>
                                <span class=dot title=dot_label></span>
                                <a href=href rel="noopener noreferrer">{u}</a>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </details>
        }
        .into_any()
    });

    view! {
        <li class="link">
            <span class=class title=label></span>
            <div class="body">
                <a class="name" href=url.clone() title=url rel="noopener noreferrer">{slug}</a>
                <p class="description">{description}</p>
                <p class="meta">{label}{latency}{freshness}{mirror_being_used}</p>
                {mirrors}
            </div>
        </li>
    }
    // Erased so the page's view type doesn't nest every row's full shape --
    // without it the release build overflows rustc's query depth limit.
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::humanize;

    #[test]
    fn humanize_reads_as_a_person_would_say_it() {
        assert_eq!(
            humanize(-3),
            "just now",
            "clock skew must not read as the future"
        );
        assert_eq!(humanize(45), "45s ago");
        assert_eq!(humanize(89), "89s ago");
        assert_eq!(humanize(90), "1m ago");
        assert_eq!(humanize(5_399), "89m ago");
        // 5400s is the minutes/hours boundary, and the one the old test got
        // wrong: 7000s is past it, so it reads in hours, not "116m ago".
        assert_eq!(humanize(5_400), "1h ago");
        assert_eq!(humanize(7_000), "1h ago");
    }
}
