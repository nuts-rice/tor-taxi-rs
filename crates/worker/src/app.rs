use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_meta::MetaTags;
use leptos_meta::{provide_meta_context, Meta, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

use crate::api::get_links;
use crate::components::chrome::{Footer, Legend, Masthead};
use crate::components::directory::Directory;

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
        <Title text="tor-rs.cab"/>
        <Meta name="description" content="A status-checked directory of onion services."/>
        <Meta name="color-scheme" content="dark"/>

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
            <Masthead/>
            <Legend/>
            <Suspense fallback=|| view! { <p class="loading">"Loading directory…"</p> }>
                {move || Suspend::new(async move {
                    match links.await {
                        Ok(links) => view! { <Directory links/> }.into_any(),
                        Err(e) => {
                            view! { <p class="error">{format!("Could not load links: {e}")}</p> }
                                .into_any()
                        }
                    }
                })}
            </Suspense>
            <Footer/>
        </div>
    }
}
