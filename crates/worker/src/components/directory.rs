use leptos::prelude::*;

use shared::{Link, LinkCategory, LinkStatus};

use super::link::ShowLink;

/// Display order, top to bottom. `rank` must agree with it; the test below
/// holds the two together.
const ORDER: [LinkCategory; 11] = [
    LinkCategory::News,
    LinkCategory::Info,
    LinkCategory::Forum,
    LinkCategory::ImageUpload,
    LinkCategory::Search,
    LinkCategory::Market,
    LinkCategory::Escrow,
    LinkCategory::Exchange,
    LinkCategory::Service,
    LinkCategory::Email,
    LinkCategory::Unknown,
];

/// Exhaustive on purpose: a new category fails to compile here instead of
/// silently vanishing from the page.
fn rank(category: LinkCategory) -> usize {
    match category {
        LinkCategory::News => 0,
        LinkCategory::Info => 1,
        LinkCategory::Forum => 2,
        LinkCategory::ImageUpload => 3,
        LinkCategory::Search => 4,
        LinkCategory::Market => 5,
        LinkCategory::Escrow => 6,
        LinkCategory::Exchange => 7,
        LinkCategory::Service => 8,
        LinkCategory::Email => 9,
        LinkCategory::Unknown => 10,
    }
}

/// One pass that moves each link into its bucket. The page used to clone the
/// whole list once per category; on a Worker that is ten copies of every
/// string in the directory, per request, to render each one once.
fn group(links: Vec<Link>) -> Vec<(LinkCategory, Vec<Link>)> {
    let mut buckets: [Vec<Link>; ORDER.len()] = Default::default();
    for link in links {
        buckets[rank(link.category)].push(link);
    }
    ORDER
        .into_iter()
        .zip(buckets)
        .filter(|(_, links)| !links.is_empty())
        .collect()
}

fn anchor(category: LinkCategory) -> String {
    category.label().to_lowercase().replace(' ', "-")
}

#[derive(Default)]
struct Tally {
    up: usize,
    degraded: usize,
    down: usize,
    unchecked: usize,
}

fn tally(links: &[Link]) -> Tally {
    let mut t = Tally::default();
    for link in links {
        match link.status {
            Some(LinkStatus::White) => t.up += 1,
            Some(LinkStatus::Orange) => t.degraded += 1,
            Some(LinkStatus::Red) => t.down += 1,
            None => t.unchecked += 1,
        }
    }
    t
}

#[component]
pub fn Directory(links: Vec<Link>) -> impl IntoView {
    let t = tally(&links);
    let groups = group(links);

    let nav = groups
        .iter()
        .map(|(category, links)| {
            view! {
                <li>
                    <a href=format!("#{}", anchor(*category))>{category.label()}</a>
                    <span class="count">{links.len()}</span>
                </li>
            }
        })
        .collect_view();

    view! {
        <p class="tally">
            <span><span class="status white"></span>{t.up}" up"</span>
            <span><span class="status orange"></span>{t.degraded}" degraded"</span>
            <span><span class="status red"></span>{t.down}" down"</span>
            {(t.unchecked > 0).then(|| view! {
                <span><span class="status unknown"></span>{t.unchecked}" unchecked"</span>
            })}
        </p>
        <nav aria-label="Categories"><ul class="toc">{nav}</ul></nav>
        {groups
            .into_iter()
            .map(|(category, links)| view! { <Section category links/> })
            .collect_view()}
    }
}

#[component]
fn Section(category: LinkCategory, links: Vec<Link>) -> impl IntoView {
    view! {
        <section id=anchor(category)>
            <h2>
                <span class="hash" aria-hidden="true">"## "</span>
                {category.label()}
                <span class="count">{links.len()}</span>
            </h2>
            <ul class="links">
                {links.into_iter().map(|link| view! { <ShowLink link/> }).collect_view()}
            </ul>
        </section>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(slug: &str, category: LinkCategory) -> Link {
        Link {
            slug: slug.into(),
            url: String::new(),
            category,
            description: String::new(),
            other_urls: None,
            status: None,
            latency_ms: None,
            checked_ago_secs: None,
            avg_latency_ms: None,
        }
    }

    #[test]
    fn rank_agrees_with_order() {
        for (i, category) in ORDER.into_iter().enumerate() {
            assert_eq!(rank(category), i, "{category:?} is out of place");
        }
    }

    #[test]
    fn group_keeps_display_order_and_drops_empty_sections() {
        let groups = group(vec![
            link("b", LinkCategory::Forum),
            link("x", LinkCategory::News),
            link("a", LinkCategory::Forum),
        ]);
        let shape: Vec<_> = groups
            .iter()
            .map(|(c, ls)| (*c, ls.iter().map(|l| l.slug.as_str()).collect::<Vec<_>>()))
            .collect();
        // Within a section, SQL's ORDER BY slug is preserved, not re-sorted.
        assert_eq!(
            shape,
            [
                (LinkCategory::News, vec!["x"]),
                (LinkCategory::Forum, vec!["b", "a"]),
            ]
        );
    }

    #[test]
    fn anchors_are_url_safe() {
        assert_eq!(anchor(LinkCategory::ImageUpload), "image-upload");
    }
}
