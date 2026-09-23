//! urban-app: UI Dioxus fullstack (SSR + hidratare) și API JSON documentat OpenAPI (ADR-0002, ADR-0004).
//!
//! - `ui`         : pagini `/` (căutare, stare în query string), `/sedinte`, `/sedinte/{id}`
//! - `query`      : `SearchParams`, starea căutării serializată în query string
//! - `server_fns` : funcții `#[server]` apelate de UI
//! - `api`        : rute `/api/v1/*`, spec la `/api/v1/openapi.json`, explorare la `/api/docs`
//! - `auth`       : identitatea `Caller` și cheia de limitare a ratei, într-un singur loc (NFR-5)
//! - `jobs`       : task tokio cu sincronizarea periodică (FR-7.1) și alertele (FR-8, FR-9)
//! - `account`    : rutele contului: autentificare prin link pe email, cuvinte-cheie, credite (FR-9)
//! - `state`      : starea procesului (config, bază), setată o singură dată la pornire

mod query;
mod server_fns;
mod ui;

#[cfg(feature = "server")]
mod account;
#[cfg(feature = "server")]
mod api;
#[cfg(feature = "server")]
mod auth;
#[cfg(feature = "server")]
mod jobs;
#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
mod state;

fn main() {
    #[cfg(feature = "server")]
    server::main();

    #[cfg(not(feature = "server"))]
    dioxus::launch(ui::App);
}
