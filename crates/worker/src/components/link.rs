use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api::get_link_status;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub enum LinkCategory {
    News,
    Search,
    Email,
    Market,
    Exchange,
    ImageUpload,
    Info,
    Escrow,
    Forum,
    Service,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct Link {
    pub id: usize,
    pub url: String,
    pub status: Status,
    pub category: LinkCategory,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub enum LinkStatus {
    Red,
    Orange,
    White,
}

#[component]
pub fn ShowLink(link: Link) -> impl IntoView {
    let url = link.url.clone();
    let status = Resource::new_blocking(
        move || url.clone(),
        |url| async move { get_link_status(url).await },
    );

    view! {
        <li class="link">
            <a href=link.url.clone()>{link.url.clone()}</a>
            <span class="category">{format!("{:?}", link.category)}</span>
            <p class="description">{link.description.clone()}</p>
            <Suspense fallback=|| view! { <span class="status">"checking…"</span> }>
                {move || Suspend::new(async move {
                    match status.await {
                        Ok(code) => view! { <span class="status">{code}</span> }.into_any(),
                        Err(e) => {
                            view! { <span class="status error">{e.to_string()}</span> }.into_any()
                        }
                    }
                })}
            </Suspense>
        </li>
    }
}
