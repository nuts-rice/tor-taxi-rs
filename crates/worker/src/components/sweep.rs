use super::link::humanize;
use leptos::prelude::*;

const STALE_AFTER_SECS: i64 = 3 * 15 * 60;

fn stale_for(ago_secs: Option<i64>) -> Option<i64> {
    ago_secs.filter(|&s| s >= STALE_AFTER_SECS)
}

#[component]
pub fn SweepBanner(ago_secs: Option<i64>) -> impl IntoView {
    stale_for(ago_secs).map(|s| {
        view! {
            <p class="stale" role="alert">
            <strong>"Checker offline: "</strong>
            {
                format!(
                "the last sweep was {}. it's time to poke the pi or something",
                    humanize(s)
                )}
            </p>
        }
    })
}
