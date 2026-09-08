use leptos::prelude::*;

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

/// Probes `url` and reports back the HTTP status the origin answered with.
#[server(GetLinkStatus)]
pub async fn get_link_status(url: String) -> Result<String, ServerFnError> {
    use anyhow::anyhow;

    // The Workers runtime is single-threaded and `reqwest`'s wasm futures are
    // `!Send`, so wrap the request so the server fn future stays `Send`.
    let status = send_wrapper::SendWrapper::new(async move {
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| LinkError::RequestError(anyhow!("request failed: {e}")))?;

        Ok::<_, LinkError>(resp.status())
    })
    .await?;

    if status.is_server_error() {
        return Err(LinkError::ResponseError(anyhow!("origin returned {status}")).into());
    }

    Ok(status.to_string())
}

#[server(SayHello)]
pub async fn say_hello(num: i32) -> Result<String, ServerFnError> {
    Ok(format!("Hello from the API!!! I got {num}"))
}
