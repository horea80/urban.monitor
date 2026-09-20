//! Handlerele `/api/v1`. Fiecare primește `Caller` (identitate în v1) și rulează interogarea în
//! `spawn_blocking`. Erorile ies ca JSON `{ "error": "..." }`.

use axum::Json;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};
use tracing::error;
use urban_core::db::Db;
use urban_shared::{Category, Item, Meeting, MeetingDetail, SearchQuery, SearchResult, Status, StreetCount};
use utoipa::{IntoParams, ToSchema};

use crate::auth::Caller;
use crate::state;

/// Eroare JSON.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiError {
    /// Mesaj pentru om, în română.
    pub error: String,
    #[serde(skip)]
    #[schema(ignore)]
    status: StatusCode,
}

impl ApiError {
    fn new(status: StatusCode, msg: impl Into<String>) -> Self {
        ApiError {
            error: msg.into(),
            status,
        }
    }

    fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }

    fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}

impl From<urban_core::Error> for ApiError {
    fn from(e: urban_core::Error) -> Self {
        error!(error = %e, "eroare internă în API");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "eroare internă")
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(e: tokio::task::JoinError) -> Self {
        error!(error = %e, "task-ul de interogare a eșuat");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "eroare internă")
    }
}

type ApiResult<T> = Result<T, ApiError>;

async fn blocking<T, F>(f: F) -> ApiResult<T>
where
    T: Send + 'static,
    F: FnOnce(Db) -> urban_core::Result<T> + Send + 'static,
{
    let db = state::db();
    Ok(tokio::task::spawn_blocking(move || f(db)).await??)
}

/// Parametrii căutării (FR-3, FR-5.1).
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchArgs {
    /// Text liber: stradă, beneficiar, cuvinte din titlu. Fără diacritice merge la fel;
    /// fiecare cuvânt se potrivește pe început de cuvânt.
    pub q: Option<String>,
    /// Categorii separate prin virgulă: `PUZ,PUD,AVIZ_OPORTUNITATE,ALTELE`.
    pub tip: Option<String>,
    /// Doar ședințele din anul dat.
    pub an: Option<i32>,
    /// Câte proiecte, implicit 50, maxim 200.
    pub limit: Option<u32>,
    /// De la ce poziție, pentru paginare.
    pub offset: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/search",
    tag = "cautare",
    params(SearchArgs),
    responses(
        (status = 200, description = "Proiectele potrivite, cele mai recente întâi, plus rândurile din ordinea de zi fără proiect publicat", body = SearchResult),
        (status = 400, description = "Parametri invalizi", body = ApiError)
    )
)]
pub async fn search(_caller: Caller, Query(a): Query<SearchArgs>) -> ApiResult<Json<SearchResult>> {
    let mut categories = Vec::new();
    for t in a
        .tip
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        categories.push(t.parse::<Category>().map_err(ApiError::bad_request)?);
    }
    let query = SearchQuery {
        q: a.q.unwrap_or_default(),
        categories,
        year: a.an,
        limit: a.limit.unwrap_or_else(SearchQuery::default_limit),
        offset: a.offset.unwrap_or(0),
    };
    Ok(Json(blocking(move |db| db.search(&query)).await?))
}

#[utoipa::path(
    get,
    path = "/meetings",
    tag = "sedinte",
    responses((status = 200, description = "Toate ședințele indexate, cele mai noi întâi", body = Vec<Meeting>))
)]
pub async fn list_meetings(_caller: Caller) -> ApiResult<Json<Vec<Meeting>>> {
    Ok(Json(blocking(|db| db.list_meetings()).await?))
}

#[utoipa::path(
    get,
    path = "/meetings/{id}",
    tag = "sedinte",
    params(("id" = i64, Path, description = "Identificatorul ședinței")),
    responses(
        (status = 200, description = "Ședința, proiectele ei și rândurile din ordinea de zi", body = MeetingDetail),
        (status = 404, description = "Ședința nu există", body = ApiError)
    )
)]
pub async fn get_meeting(_caller: Caller, Path(id): Path<i64>) -> ApiResult<Json<MeetingDetail>> {
    let detail = blocking(move |db| {
        let Some(meeting) = db.get_meeting(id)? else {
            return Ok(None);
        };
        Ok(Some(MeetingDetail {
            items: db.items_for_meeting(id)?,
            agenda_rows: db.agenda_rows_for_meeting(id)?,
            meeting,
        }))
    })
    .await?;
    detail.map(Json).ok_or_else(|| ApiError::not_found("ședința nu există"))
}

#[utoipa::path(
    get,
    path = "/items/{id}",
    tag = "sedinte",
    params(("id" = i64, Path, description = "Identificatorul proiectului")),
    responses(
        (status = 200, description = "Proiectul, cu documentele lui", body = Item),
        (status = 404, description = "Proiectul nu există", body = ApiError)
    )
)]
pub async fn get_item(_caller: Caller, Path(id): Path<i64>) -> ApiResult<Json<Item>> {
    blocking(move |db| db.get_item(id))
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("proiectul nu există"))
}

#[utoipa::path(
    get,
    path = "/streets",
    tag = "cautare",
    responses((status = 200, description = "Străzile distincte și numărul de proiecte pe fiecare", body = Vec<StreetCount>))
)]
pub async fn streets(_caller: Caller) -> ApiResult<Json<Vec<StreetCount>>> {
    let list = blocking(|db| {
        Ok(db
            .streets()?
            .into_iter()
            .map(|(street, count)| StreetCount { street, count })
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(Json(list))
}

/// Starea serviciului (FR-7.3).
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiStatus {
    pub version: String,
    /// Pornirea procesului, ISO 8601 UTC.
    pub started_at: String,
    /// O sincronizare rulează acum.
    pub sync_running: bool,
    /// Contoare și ultima sincronizare.
    pub data: Status,
}

#[utoipa::path(
    get,
    path = "/status",
    tag = "operare",
    responses((status = 200, description = "Versiune, contoare, ultima sincronizare", body = ApiStatus))
)]
pub async fn status(_caller: Caller) -> ApiResult<Json<ApiStatus>> {
    let st = state::get();
    let data = blocking(|db| db.status()).await?;
    Ok(Json(ApiStatus {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        started_at: st.started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        sync_running: state::sync_running(),
        data,
    }))
}
