use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Stylesheet};
#[cfg(feature = "ssr")]
use leptos_meta::MetaTags;
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::components::link::{Link, LinkCategory, ShowLink, Status};
use crate::components::show_data_from_api::ShowDataFromApi;

#[cfg(feature = "ssr")]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/tor-taxi-rs-worker.css"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    // Placeholder directory until the link set is loaded from D1.
    let links = vec![Link {
        id: 0,
        url: "https://tor.taxi".to_string(),
        status: Status::White,
        category: LinkCategory::Info,
        description: "The directory itself.".to_string(),
    }];

    view! {
        <div class="container">
            <h1>"tor-taxi.rs - A link resource for darknet"</h1>
            <p>"Links in red are experiencing downtime"</p>
            <p>"Links in orange are experiencing DDoS attack or maintenance"</p>
            <p>
                "Inspired by original tor.taxi. Written in Rust using Leptos framework. 🦀"
            </p>
        </div>
        <ul class="links">
            {links
                .into_iter()
                .map(|link| view! { <ShowLink link=link /> })
                .collect_view()}
        </ul>
        <ShowDataFromApi />
    }
}