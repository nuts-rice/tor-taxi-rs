use crate::components::link::Link;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryData {
    pub links: Vec<Link>,
    pub last_sweep_ago_secs: Option<i64>,
}
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

    #[derive(Deserialize)]
    struct LastSweep {
        ago_secs: Option<i64>,
    }

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

        let res = db
            .batch(vec![
                db.prepare(shared::SELECT_LINKS_SQL),
                db.prepare(shared::SELECT_LAST_SWEEP_SQL),
            ])
            .await
            .map_err(|e| ServerFnError::new(format!("D1 query failed: {e}")))?;

        let [links, last_sweep]: [_; 2] = res
            .try_into()
            .map_err(|_| ServerFnError::new("D1 batch brought wrong number of results "))?;

        let links = links
            .results::<Link>()
            .map_err(|e| ServerFnError::new(format!("Links results are wrong shape: {e}")))?;
        let last_sweep_ago_secs = last_sweep
            .results::<LastSweep>()
            .map_err(|e| ServerFnError::new(format!("Last sweep is malformed: {e}")))?
            .into_iter()
            .next()
            .and_then(|r| r.ago_secs);

        Ok(DirectoryData {
            links,
            last_sweep_ago_secs,
        })

        // The query lives in `shared` so the checker's schema_drift test can
        // prepare it against the real migrations. Inline here, a clause in the
        // wrong order -- WHERE before FROM -- compiles fine and only fails once
        // it reaches D1, which for this Worker means the page never renders.
        //
        // Age is computed in SQL rather than from the clock at render time, so
        // the server and the hydrated client agree on the same number.
    })
    .await
}
