use leptos::prelude::*;

use crate::components::link::Link;

/// Reads the directory out of D1.
///
/// The Worker never probes anything itself: the Workers runtime has no SOCKS
/// and no Tor, so it cannot reach a hidden service. `crates/checker` does the
/// probing from a host that can, and this only reports what it wrote.
#[server(GetLinks)]
pub async fn get_links() -> Result<Vec<Link>, ServerFnError> {
    use std::sync::Arc;

    use axum::Extension;
    use worker::send::SendFuture;

    // Layered onto the router in lib.rs so server fns can reach the bindings.
    let Extension(env): Extension<Arc<worker::Env>> = leptos_axum::extract()
        .await
        .map_err(|e| ServerFnError::new(format!("no Worker env: {e}")))?;

    // `Env` and `D1Database` are Send, but the futures off `.all()` wrap
    // `JsFuture` and are not. Workers is single-threaded, so this is sound.
    SendFuture::new(async move {
        let db = env
            .d1("prod_links")
            .map_err(|e| ServerFnError::new(format!("no D1 binding: {e}")))?;

        // Age is computed in SQL rather than from the clock at render time, so
        // the server and the hydrated client agree on the same number.
        let rows = db
            .prepare(
                "SELECT slug, url, category, description, status, latency_ms, \
                 (strftime('%s', 'now') - last_checked_at) AS checked_ago_secs \
                 FROM links ORDER BY category, slug",
            )
            .all()
            .await
            .map_err(|e| ServerFnError::new(format!("D1 query failed: {e}")))?;

        rows.results::<Link>()
            .map_err(|e| ServerFnError::new(format!("unexpected row shape: {e}")))
    })
    .await
}
