//! API JSON read-only `/api/v1` (FR-5): descris OpenAPI generat din cod (utoipa), servit la
//! `/api/v1/openapi.json` și explorabil la `/api/docs`. Public în v1, cu `bearerAuth` doar declarat.

mod handlers;

use std::sync::Arc;
use std::time::Duration;

use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router, middleware};
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_scalar::Scalar;

use crate::auth::{self, RateKey};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "urban.monitor API",
        description = "Ședințele Comisiei Tehnice de Amenajare a Teritoriului și Urbanism (CTATU) \
                       Cluj-Napoca, din 2026 încoace: proiecte PUZ, PUD și avize de oportunitate, \
                       căutabile după stradă. Read-only, public. Sursa: primariaclujnapoca.ro.",
        license(name = "MIT")
    ),
    servers((url = "/api/v1", description = "acest server")),
    modifiers(&SecurityAddon),
    tags(
        (name = "cautare", description = "Căutare după stradă și filtre"),
        (name = "sedinte", description = "Ședințele comisiei și proiectele lor"),
        (name = "operare", description = "Starea serviciului")
    )
)]
struct ApiDoc;

/// Declară schema `bearerAuth` fără a o cere (NFR-5): clienții o pot trimite de acum.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("cheie API")
                    .description(Some(
                        "Rezervat pentru chei API (ADR-0004). Neactiv în v1: apelurile sunt anonime.",
                    ))
                    .build(),
            ),
        );
    }
}

/// Rutele API (serverul le montează sub `/api/v1`) și documentul OpenAPI rezultat.
pub fn router() -> (Router, utoipa::openapi::OpenApi) {
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(handlers::search))
        .routes(routes!(handlers::list_meetings))
        .routes(routes!(handlers::get_meeting))
        .routes(routes!(handlers::get_item))
        .routes(routes!(handlers::streets))
        .routes(routes!(handlers::status))
        .split_for_parts();

    // limitare de rată per apelant (NFR-4); cheia vine din `auth::RateKey`
    let governor = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(60)
            .key_extractor(RateKey)
            .finish()
            .expect("configurație validă de limitare a ratei"),
    );
    let limiter = governor.limiter().clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            limiter.retain_recent();
        }
    });

    let spec = api.clone();
    let router = router
        .route(
            "/openapi.json",
            get(move || {
                let spec = spec.clone();
                async move { Json(spec) }
            }),
        )
        .layer(GovernorLayer::new(governor))
        // `identify` rulează primul (stratul cel mai exterior), ca `RateKey` să găsească `Caller`
        .layer(middleware::from_fn(auth::identify));
    (router, api)
}

/// Interfața de explorare (Scalar) la `/api/docs`. Încarcă scriptul de pe cdn.jsdelivr.net;
/// CSP-ul din Caddyfile îl permite.
pub fn docs_router(api: utoipa::openapi::OpenApi) -> Router {
    let html = Scalar::new(api).to_html();
    Router::new().route(
        "/api/docs",
        get(move || {
            let html = html.clone();
            async move { Html(html) }
        }),
    )
}
