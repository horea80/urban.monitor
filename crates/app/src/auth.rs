//! Identitatea apelantului și cheia de limitare a ratei, într-un singur loc (NFR-4, NFR-5, ADR-0004).
//! În v1 nu există chei API: orice apel e `Caller::Anonymous`, iar rata se limitează per IP.
//! Când apar cheile, validarea intră în `identify` și `RateKey` trece de la IP la cheie, fără a
//! atinge handlerele.

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts, Request};
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use tower_governor::GovernorError;
use tower_governor::key_extractor::KeyExtractor;
use tracing::debug;

#[derive(Debug, Clone)]
pub enum Caller {
    Anonymous { ip: Option<IpAddr> },
    // ApiKey { id: i64, plan: String } — rezervat (tabelul api_keys, ADR-0004)
}

impl Caller {
    pub fn rate_key(&self) -> String {
        match self {
            Caller::Anonymous { ip: Some(ip) } => ip.to_string(),
            Caller::Anonymous { ip: None } => "anonim".to_owned(),
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Infallible> {
        Ok(parts
            .extensions
            .get::<Caller>()
            .cloned()
            .unwrap_or(Caller::Anonymous { ip: None }))
    }
}

/// Middleware: stabilește `Caller` și îl pune în extensiile cererii. Rulează înaintea limitării de rată.
pub async fn identify(ConnectInfo(addr): ConnectInfo<SocketAddr>, mut req: Request, next: Next) -> Response {
    let headers = req.headers();
    if headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("Bearer "))
    {
        debug!("cheie API primită; autentificarea nu este activă în v1, apelul e tratat ca anonim");
    }
    let ip = forwarded_ip(headers).unwrap_or(addr.ip());
    req.extensions_mut().insert(Caller::Anonymous { ip: Some(ip) });
    next.run(req).await
}

/// IP-ul real din spatele proxy-ului. Caddy setează `X-Forwarded-For`; serverul nu e expus direct
/// (docker-compose doar `expose`), altfel antetul ar putea fi falsificat.
fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse().ok())
        })
}

/// Cheia de limitare a ratei, derivată din `Caller`: trecerea de la IP la cheie API se face aici.
#[derive(Debug, Clone, Copy)]
pub struct RateKey;

impl KeyExtractor for RateKey {
    type Key = String;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        Ok(req
            .extensions()
            .get::<Caller>()
            .map(Caller::rate_key)
            .unwrap_or_else(|| "anonim".to_owned()))
    }
}
