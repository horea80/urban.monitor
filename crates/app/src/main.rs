//! urban-app: UI Dioxus fullstack (SSR + hidratare) și API JSON documentat OpenAPI.
//!
//! Module planificate:
//! - `ui`      : pagini `/` (căutare, stare în query string) și `/sedinte/:id`
//! - `server_fns` : funcții `#[server]` apelate de UI
//! - `api`     : axum, rute `/api/v1/*`, spec la `/api/v1/openapi.json`, docs la `/api/docs`
//! - `auth`    : strat tower, identitate acum; loc rezervat pentru chei API și cote
//! - `jobs`    : task tokio cu sync periodic (6h) și lock single-flight

use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        h1 { "{urban_shared::NAME}" }
        p { "Ședințe CTATU Cluj-Napoca — schelet, UI-ul urmează." }
    }
}
