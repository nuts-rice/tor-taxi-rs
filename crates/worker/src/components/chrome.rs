//! Everything around the directory: masthead, legend, footer. Static, so it
//! renders even when D1 does not answer.

use leptos::prelude::*;

#[component]
pub fn Masthead() -> impl IntoView {
    view! {
        <header class="masthead">
            <h1>
                <span class="prompt" aria-hidden="true">"$ "</span>
                "tor-rs.cab"
                <span class="cursor" aria-hidden="true"></span>
            </h1>
            <p class="tagline">"a link resource for the darknet, kept honest by a probe over Tor"</p>
            <p class="aside">
                "Inspired by the original tor.taxi. Frontend in Rust on Leptos, "
                "checker in Rust on a Pi. 🦀 Onion routing for this site should "
                "work in Tor Browser."
            </p>
            <p class="notice" role="note">
                <strong>"heads up: "</strong>
                "Dark Matter Market is suspected of an exit scam."
            </p>
        </header>
    }
}

/// The dots themselves, not a paragraph describing them.
#[component]
pub fn Legend() -> impl IntoView {
    view! {
        <dl class="legend">
            <div><dt><span class="status white"></span></dt><dd>"up"</dd></div>
            <div><dt><span class="status orange"></span></dt><dd>"slow, DDoS or maintenance"</dd></div>
            <div><dt><span class="status red"></span></dt><dd>"down"</dd></div>
            <div><dt><span class="status unknown"></span></dt><dd>"not yet checked"</dd></div>
        </dl>
    }
}

#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <footer class="colophon">
            <p>"Intended for research purposes. Nothing here is an endorsement."</p>
            <p>
                "Think a link should be added or removed? "
                <span class="contact">"0x0fSoftworks@protonmail.com"</span>
            </p>
            <p class="verify">
                <a href="/canary.txt" target="_blank" rel="noopener noreferrer">"canary"</a>
                " · "
                <a href="/pgp.txt" target="_blank" rel="noopener noreferrer">"pgp"</a>
            </p>
        </footer>
    }
}
