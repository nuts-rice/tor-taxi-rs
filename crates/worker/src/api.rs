use leptos::prelude::*;
use crate::components::Link;
#[cfg(feature = "ssr")]
#[derive(Debug)]
pub enum LinkError {
    RequestError(anyhow::Error),
    ResponseError(anyhow::Error),
}

#[cfg(feature = "ssr")]
impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::RequestError(e) => write!(f, "request error: {e}"),
            LinkError::ResponseError(e) => write!(f, "response error: {e}"),
        }
    }
}

#[cfg(feature = "ssr")]
impl std::error::Error for LinkError {}

/// This is a D1 read  
#[server(GetLinks)]
pub async fn get_links()-> Result<Vec<Link>, ServerFnError> {
    use anyhow::anyhow;
    use worker::send::SendFuture;
    let Extension(env) = leptos_axum::extract::<Extension<Arc<worker::Env>>>().await?;
    SendFuture::new(async move {
        let db = env.d1("links-prod")?;
        db.prepare("SELECT url, category, description, status, latency_ms, \
            last_checked_at FROM links ORDER BY category, url ")
            .all().await?.results()
    }).await
}

#[server(SayHello)]
pub async fn say_hello(num: i32) -> Result<String, ServerFnError> {
    Ok(format!("Hello from the API!!! I got {num}"))
}
