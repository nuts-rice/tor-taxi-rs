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
    // One fetch for the whole page. Blocking so the directory is in the HTML
    // the crawler and the no-JS reader get, not painted in afterwards.
    let links = Resource::new_blocking(|| (), |_| get_links());

    view! {
        <div class="container">
            <h1>"tor-taxi.rs - A link resource for darknet"</h1>
            <p>"Links in red are experiencing downtime"</p>
            <p>"Links in orange are experiencing DDoS attack or maintenance"</p>
            <p>
                "Inspired by original tor.taxi. Written in Rust using Leptos framework. 🦀"
            </p>

            <Suspense fallback=|| view! { <p class="loading">"Loading directory…"</p> }>
                {move || Suspend::new(async move {
                    match links.await {
                        Ok(links) => {
                            view! {
                                <ul class="links">
                                    {links
                                        .into_iter()
                                        .map(|link| view! { <ShowLink link=link /> })
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
            </Suspense>
        </div>
    }
}
