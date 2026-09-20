//! Pornirea serverului: axum + Dioxus SSR, API, `/healthz`, job-ul de sincronizare.
//! Adresa vine din `IP` și `PORT` (implicit 127.0.0.1:8080), la fel ca la `dx serve`.

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use dioxus::server::{DioxusRouterExt, ServeConfig};
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use urban_core::Config;
use urban_core::db::Db;

use crate::{api, jobs, state, ui};

pub fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    let rt = tokio::runtime::Runtime::new().expect("runtime tokio");
    if let Err(e) = rt.block_on(run()) {
        eprintln!("eroare fatală: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cfg = Config::from_env();
    let db = Db::open(&cfg.db_path())?;
    let stale = db.close_stale_runs()?;
    if stale > 0 {
        warn!(
            stale,
            "sincronizări rămase „în curs” de la o oprire anterioară, marcate ca întrerupte"
        );
    }
    let st = state::init(cfg, db);
    jobs::spawn(st);

    let (api_router, openapi) = api::router();
    let app = Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1", api_router)
        .merge(api::docs_router(openapi))
        .merge(ui_router())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = dioxus::cli_config::fullstack_address_or_localhost();
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, data_dir = %st.cfg.data_dir.display(), "urban-app pornit");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

/// 200 când procesul și baza răspund (FR-7.2).
async fn healthz() -> Result<&'static str, StatusCode> {
    let db = state::db();
    match tokio::task::spawn_blocking(move || db.status()).await {
        Ok(Ok(_)) => Ok("ok"),
        _ => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Cu `dx` există `public/` lângă binar (wasm, CSS și index.html). Fără el, la `cargo run`, servim
/// doar SSR și funcțiile server: paginile merg, dar fără stiluri și fără hidratare.
fn ui_router() -> Router {
    let public = std::env::var_os("DIOXUS_PUBLIC_PATH").map(PathBuf::from).or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("public")))
    });
    match public {
        Some(dir) if dir.is_dir() => Router::new().serve_dioxus_application(ServeConfig::new(), ui::App),
        _ => {
            warn!("nu există public/ lângă binar (pornit fără dx): servesc doar SSR, fără CSS și wasm");
            Router::new().serve_api_application(ServeConfig::new(), ui::App)
        }
    }
}

async fn shutdown() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => info!("oprire cerută; închid conexiunile"),
        Err(e) => {
            warn!(error = %e, "nu pot asculta Ctrl-C; procesul se oprește doar la kill");
            std::future::pending::<()>().await;
        }
    }
}
