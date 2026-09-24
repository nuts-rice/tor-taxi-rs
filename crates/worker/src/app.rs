use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_meta::MetaTags;
use leptos_meta::{provide_meta_context, Stylesheet};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::api::get_links;
use crate::components::link::ShowLink;

#[cfg(feature = "ssr")]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
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
    // One fetch for the whole page. Blocking so the directory is in the HTML
    // the crawler and the no-JS reader get, not painted in afterwards.
    let links = Resource::new_blocking(|| (), |_| get_links());
    view! {
        <div class="container">
            <h1>"tor-rs.cab - A link resource for darknet"</h1>
            <p>"Links with a red dot are experiencing downtime"</p>
            <p>"Links with an orange dot are experiencing DDoS attack or maintenance"</p>
            <p> "Inspired by original tor.taxi. Frontend in Rust using Leptos framework." </p>
            <p> "Checker probe is Rust as well.  🦀"
            <h4> "Onion routing for this site should be working in Tor Browser."</h4>


            </p>

            <Suspense fallback=|| view! { <p class="loading">"Loading directory…"</p> }>
                {move || Suspend::new(async move {
                    match links.await {
                        Ok(links) => {
                            view! {
                                <h2> "News" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "News")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>
                                <h2> "Info" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Info")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>

                                <h2> "Forum" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Forum")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>
                                <h2> "Image Upload" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Image Upload")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>
                                <h2> "Search" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Search")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>
                                <h2> "Market" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Market")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>
                                <h2> "Escrow" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Escrow")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>

                                <h2> "Exchange" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Exchange")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>
                                <h2> "Service" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Service")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>
                                <h2> "Email" </h2>
                                <ul class="links">
                                    {links
                                        .clone()
                                        .into_iter()
                                        .filter(|link| link.category.label() == "Email")
                                        .map(|link| view! { <ShowLink link=link  /> })
                                        .collect_view()}
                                </ul>






                            }
                                .into_any()
                        }
                        Err(e) => {
                            view! { <p class="error">{format!("Could not load links: {e}")}</p> }
                                .into_any()
                        }
                    }
                })}
        <p> "Intended for research purposes etc etc..." </p>
        <p> "If you believe a link should be added or removed please contact at:" </p>
        <p> "0x0fSoftworks@protonmail.com" </p>
            </Suspense>
        </div>
    }
}
