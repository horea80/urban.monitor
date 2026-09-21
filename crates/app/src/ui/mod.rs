//! Paginile aplicației (FR-4). Ruta `/` poartă starea căutării în query string, ca linkul să fie
//! partajabil și ca formularul clasic (fără wasm) să ducă în același loc.

use dioxus::prelude::*;

use crate::query::SearchParams;

mod components;
mod home;
mod meeting;
mod meetings;

use components::Footer;
use home::Home;
use meeting::MeetingPage;
use meetings::Meetings;

#[derive(Routable, Clone, PartialEq, Debug)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Shell)]
        #[route("/?:..params")]
        Home { params: SearchParams },
        #[route("/sedinte")]
        Meetings {},
        #[route("/sedinte/:id")]
        MeetingPage { id: i64 },
    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[component]
pub fn App() -> Element {
    rsx! {
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1" }
        document::Stylesheet { href: MAIN_CSS }
        Router::<Route> {}
    }
}

#[component]
fn Shell() -> Element {
    rsx! {
        header { class: "site-header",
            div { class: "inner",
                Link { class: "brand", to: Route::Home { params: SearchParams::default() }, "urban.monitor" }
                span { class: "tagline",
                    "Ședințele Comisiei Tehnice de Urbanism (CTATU) Cluj-Napoca, căutare după stradă, beneficiar, titlu"
                }
                nav {
                    Link { to: Route::Home { params: SearchParams::default() }, active_class: "active", "Căutare" }
                    Link { to: Route::Meetings {}, active_class: "active", "Ședințe" }
                }
            }
        }
        main { Outlet::<Route> {} }
        Footer {}
    }
}

#[component]
fn NotFound(segments: Vec<String>) -> Element {
    let path = segments.join("/");
    rsx! {
        document::Title { "Pagină inexistentă · urban.monitor" }
        h1 { "Pagina nu există" }
        p { "Adresa /{path} nu corespunde niciunei pagini." }
        Link { to: Route::Home { params: SearchParams::default() }, "Înapoi la căutare" }
    }
}
