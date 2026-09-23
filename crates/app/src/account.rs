//! Rutele contului (FR-9): formulare clasice și redirecturi, ca totul să meargă și fără wasm
//! (NFR-7). Sesiunea e un cookie `HttpOnly` cu un jeton aleator; în bază stă doar hash-ul lui.
//! Mesajele pentru utilizator se întorc pe `/cont?ok=…` sau `/cont?eroare=…`, unde le afișează
//! pagina (ca text, deci fără risc de injecție).

use axum::Router;
use axum::extract::{Form, Path, Query};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use tracing::{error, info, warn};
use urban_core::accounts::{self, SESSION_TTL_DAYS, User};

use crate::state;

pub const COOKIE: &str = "urban_session";

pub fn router() -> Router {
    Router::new()
        .route("/cont/login", post(login))
        .route("/cont/verifica", get(verify))
        .route("/cont/iesire", post(logout))
        .route("/cont/cuvinte", post(add_keyword))
        .route("/cont/cuvinte/{id}/sterge", post(delete_keyword))
        .route("/cont/sterge", post(delete_account))
}

/// Jetonul de sesiune din antetul `Cookie`, dacă există.
pub fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .map(str::trim)
        .find_map(|kv| kv.strip_prefix(&format!("{COOKIE}=")).map(str::to_owned))
}

/// Utilizatorul autentificat prin cookie, cu ciclul de credite la zi; `None` pentru vizitatori.
pub async fn current_user(headers: &HeaderMap) -> Option<User> {
    let token = session_token(headers)?;
    let db = state::db();
    match tokio::task::spawn_blocking(move || db.user_by_session(&token)).await {
        Ok(Ok(user)) => user,
        Ok(Err(e)) => {
            error!(error = %e, "nu am putut citi sesiunea");
            None
        }
        Err(e) => {
            error!(error = %e, "task-ul sesiunii a eșuat");
            None
        }
    }
}

fn secure_flag() -> &'static str {
    if state::get().cfg.public_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    }
}

fn with_cookie(to: &str, cookie: String) -> Response {
    let mut resp = Redirect::to(to).into_response();
    if let Ok(v) = cookie.parse() {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    resp
}

fn login_cookie(token: &str) -> String {
    format!(
        "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        SESSION_TTL_DAYS * 86_400,
        secure_flag()
    )
}

fn clear_cookie() -> String {
    format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}", secure_flag())
}

fn back(ok: Option<&str>, eroare: Option<&str>) -> Redirect {
    let mut qs = form_urlencoded::Serializer::new(String::new());
    if let Some(v) = ok {
        qs.append_pair("ok", v);
    }
    if let Some(v) = eroare {
        qs.append_pair("eroare", v);
    }
    let qs = qs.finish();
    if qs.is_empty() {
        Redirect::to("/cont")
    } else {
        Redirect::to(&format!("/cont?{qs}"))
    }
}

async fn blocking<T, F>(f: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce(urban_core::db::Db) -> urban_core::Result<T> + Send + 'static,
{
    let db = state::db();
    match tokio::task::spawn_blocking(move || f(db)).await {
        Ok(Ok(v)) => Some(v),
        Ok(Err(e)) => {
            error!(error = %e, "eroare la baza de date (cont)");
            None
        }
        Err(e) => {
            error!(error = %e, "task blocant eșuat (cont)");
            None
        }
    }
}

const DB_ERROR: &str = "A apărut o eroare; încearcă din nou în câteva minute.";

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    #[serde(default)]
    acord: Option<String>,
}

async fn login(Form(form): Form<LoginForm>) -> Response {
    let Some(email) = accounts::normalize_email(&form.email) else {
        return back(None, Some("Adresa de email nu arată bine.")).into_response();
    };
    let consent = form.acord.is_some();
    let e2 = email.clone();
    let Some(token) = blocking(move |db| db.create_magic_link(&e2, consent)).await else {
        return back(None, Some(DB_ERROR)).into_response();
    };
    // fără jeton = a cerut deja unul de curând; răspundem la fel, ca să nu dezvăluim nimic
    if let Some(token) = token {
        let url = format!("{}/cont/verifica?token={token}", state::get().cfg.public_url);
        match state::get().mailer.as_ref() {
            Some(m) => {
                if let Err(e) = m.send_login_link(&email, &url).await {
                    warn!(error = %e, "nu am putut trimite linkul de autentificare");
                    return back(None, Some("Nu am putut trimite emailul; încearcă din nou.")).into_response();
                }
            }
            None => info!(%email, %url, "fără RESEND_API_KEY: linkul de autentificare (doar în jurnal)"),
        }
    }
    back(Some("trimis"), None).into_response()
}

#[derive(Deserialize)]
struct VerifyQuery {
    #[serde(default)]
    token: String,
}

async fn verify(Query(q): Query<VerifyQuery>) -> Response {
    let token = q.token.trim().to_owned();
    if token.is_empty() {
        return back(None, Some("Linkul e incomplet.")).into_response();
    }
    let Some(link) = blocking(move |db| db.consume_magic_link(&token)).await else {
        return back(None, Some(DB_ERROR)).into_response();
    };
    let Some((email, consent)) = link else {
        return back(None, Some("Linkul nu mai e valid; cere unul nou.")).into_response();
    };
    let db = state::db();
    let created = tokio::task::spawn_blocking(move || {
        let user = db.find_or_create_user(&email, consent)?;
        let session = db.create_session(user.id)?;
        Ok::<_, urban_core::Error>((user, session))
    })
    .await;
    match created {
        Ok(Ok((user, session))) => {
            info!(user = user.id, "autentificare reușită");
            with_cookie("/cont", login_cookie(&session))
        }
        Ok(Err(urban_core::Error::Other(msg))) if msg.contains("acord") => back(
            None,
            Some("Pentru un cont nou trebuie bifat acordul din formular; cere un link nou."),
        )
        .into_response(),
        Ok(Err(e)) => {
            error!(error = %e, "autentificarea a eșuat");
            back(None, Some(DB_ERROR)).into_response()
        }
        Err(e) => {
            error!(error = %e, "task-ul de autentificare a eșuat");
            back(None, Some(DB_ERROR)).into_response()
        }
    }
}

async fn logout(headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        blocking(move |db| db.delete_session(&token)).await;
    }
    with_cookie("/", clear_cookie())
}

#[derive(Deserialize)]
struct KeywordForm {
    #[serde(default)]
    cuvant: String,
}

async fn add_keyword(headers: HeaderMap, Form(form): Form<KeywordForm>) -> Response {
    let Some(user) = current_user(&headers).await else {
        return back(None, Some("Intră în cont mai întâi.")).into_response();
    };
    let text = form.cuvant;
    match blocking(move |db| db.add_keyword(&user, &text)).await {
        Some(Ok(_)) => back(Some("adaugat"), None).into_response(),
        Some(Err(reason)) => back(None, Some(&reason)).into_response(),
        None => back(None, Some(DB_ERROR)).into_response(),
    }
}

async fn delete_keyword(headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let Some(user) = current_user(&headers).await else {
        return back(None, Some("Intră în cont mai întâi.")).into_response();
    };
    blocking(move |db| db.delete_keyword(user.id, id)).await;
    back(None, None).into_response()
}

#[derive(Deserialize)]
struct ConfirmForm {
    #[serde(default)]
    confirm: String,
}

async fn delete_account(headers: HeaderMap, Form(form): Form<ConfirmForm>) -> Response {
    let Some(user) = current_user(&headers).await else {
        return back(None, Some("Intră în cont mai întâi.")).into_response();
    };
    if form.confirm != "da" {
        return back(None, Some("Ștergerea contului cere confirmarea.")).into_response();
    }
    let id = user.id;
    if blocking(move |db| db.delete_user(id)).await.is_none() {
        return back(None, Some(DB_ERROR)).into_response();
    }
    info!(user = id, "cont șters la cererea utilizatorului");
    with_cookie("/cont?ok=sters", clear_cookie())
}
