//! Funcțiile `#[server]` apelate de UI (design §6.2). Corpul rulează doar pe server; din browser
//! ajung aici prin cererile generate de Dioxus. Interogările SQLite rulează în `spawn_blocking`.

use dioxus::prelude::*;
use urban_shared::{Meeting, MeetingDetail, SearchQuery, SearchResult, Status};

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
