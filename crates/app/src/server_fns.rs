//! Funcțiile `#[server]` apelate de UI (design §6.2). Corpul rulează doar pe server; din browser
//! ajung aici prin cererile generate de Dioxus. Interogările SQLite rulează în `spawn_blocking`.

use dioxus::prelude::*;
use urban_shared::{AccountView, Meeting, MeetingDetail, SearchQuery, SearchResult, Status};

#[cfg(feature = "server")]
async fn blocking<T, F>(f: F) -> ServerFnResult<T>
where
    T: Send + 'static,
    F: FnOnce(urban_core::db::Db) -> urban_core::Result<T> + Send + 'static,
{
    let db = crate::state::db();
    tokio::task::spawn_blocking(move || f(db))
        .await
        .map_err(ServerFnError::new)?
        .map_err(ServerFnError::new)
}

#[server]
pub async fn search(query: SearchQuery) -> ServerFnResult<SearchResult> {
    blocking(move |db| db.search(&query)).await
}

#[server]
pub async fn list_meetings() -> ServerFnResult<Vec<Meeting>> {
    blocking(|db| db.list_meetings()).await
}

#[server]
pub async fn meeting_detail(id: i64) -> ServerFnResult<Option<MeetingDetail>> {
    blocking(move |db| {
        let Some(meeting) = db.get_meeting(id)? else {
            return Ok(None);
        };
        Ok(Some(MeetingDetail {
            items: db.items_for_meeting(id)?,
            agenda_rows: db.agenda_rows_for_meeting(id)?,
            meeting,
        }))
    })
    .await
}

#[server]
pub async fn status() -> ServerFnResult<Status> {
    blocking(|db| db.status()).await
}

/// Contul autentificat prin cookie-ul de sesiune, sau `None` pentru vizitatori (FR-9).
#[server]
pub async fn me() -> ServerFnResult<Option<AccountView>> {
    let headers: axum::http::HeaderMap = dioxus::fullstack::FullstackContext::extract().await?;
    let Some(user) = crate::account::current_user(&headers).await else {
        return Ok(None);
    };
    let view = blocking(move |db| {
        use urban_shared::KeywordView;
        let keywords = db
            .keywords(user.id)?
            .into_iter()
            .map(|k| KeywordView { id: k.id, text: k.text })
            .collect();
        Ok(AccountView {
            email: user.email.clone(),
            plan: user.plan.clone(),
            credits_balance: user.balance(),
            credits_per_year: urban_core::accounts::FREE_CREDITS_PER_YEAR,
            cycle_end: user.cycle_end_date(),
            keywords,
        })
    })
    .await?;
    Ok(Some(view))
}
