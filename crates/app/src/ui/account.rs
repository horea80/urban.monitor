//! Pagina contului (FR-9): autentificare fără parolă (link pe email), cuvintele-cheie urmărite și
//! creditele. Formularele sunt clasice (POST către rutele din `crate::account`), deci pagina merge
//! și fără wasm; mesajele vin înapoi în query string (`?ok=…`, `?eroare=…`).

use dioxus::prelude::*;
use urban_shared::AccountView;
use urban_shared::time::fmt_date_ro;

use super::Route;
use crate::query::AccountParams;
use crate::server_fns;

#[component]
pub fn Account(params: AccountParams) -> Element {
    let me = use_server_future(server_fns::me)?;
    rsx! {
        h1 { "Alerte pe cuvinte-cheie" }
        if let Some(msg) = params.ok_message() {
            p { class: "notice", "{msg}" }
        }
        if !params.eroare.is_empty() {
            p { class: "error", "{params.eroare}" }
        }
        match me() {
            Some(Ok(Some(acc))) => rsx! { Panel { acc } },
            Some(Ok(None)) => rsx! { Login {} },
            Some(Err(e)) => rsx! { p { class: "error", "Eroare: {e}" } },
            None => rsx! { p { class: "muted", "Se încarcă…" } },
        }
    }
}

#[component]
fn Login() -> Element {
    rsx! {
        p {
            "Primești un email de fiecare dată când un proiect nou de pe ordinea de zi CTATU se potrivește cu un cuvânt-cheie "
            "ales de tine: o stradă, un cartier, un beneficiar. Planul gratuit are 2 credite pe an; un credit înseamnă un "
            "cuvânt-cheie urmărit un an, cu alerte nelimitate."
        }
        p { class: "muted", "Fără parolă: îți trimitem un link de autentificare pe email, valabil 15 minute." }
        form { class: "account-form", method: "post", action: "/cont/login",
            label {
                "Adresa de email"
                input { r#type: "email", name: "email", required: true, autocomplete: "email", placeholder: "nume@exemplu.ro" }
            }
            label { class: "check",
                input { r#type: "checkbox", name: "acord", value: "da" }
                span {
                    "Sunt de acord ca adresa mea de email să fie folosită pentru autentificare și pentru alertele pe care le cer ("
                    Link { to: Route::Privacy {}, "confidențialitate" }
                    "). Obligatoriu doar la primul cont."
                }
            }
            button { r#type: "submit", "Trimite-mi linkul de autentificare" }
        }
    }
}

#[component]
fn Panel(acc: AccountView) -> Element {
    let reset = fmt_date_ro(acc.cycle_end);
    rsx! {
        p {
            "Autentificat ca " strong { "{acc.email}" } " · "
            form { class: "inline", method: "post", action: "/cont/iesire",
                button { r#type: "submit", class: "link", "Ieși din cont" }
            }
        }
        section { class: "credits",
            p {
                strong { "{acc.credits_balance} din {acc.credits_per_year} credite" }
                " disponibile; se reînnoiesc pe {reset}."
            }
            p { class: "muted",
                "Un credit = un cuvânt-cheie urmărit un an, cu alerte nelimitate pe email. Creditul se consumă la adăugare "
                "și nu se recuperează la ștergere; la reînnoire, cuvintele păstrate se reînnoiesc din creditele noi."
            }
        }
        h2 { "Cuvintele tale cheie" }
        if acc.keywords.is_empty() {
            p { class: "muted", "Niciun cuvânt-cheie încă." }
        } else {
            ul { class: "keywords",
                for k in acc.keywords.iter() {
                    li { key: "{k.id}",
                        span { class: "kw", "{k.text}" }
                        Link { to: Route::Home { params: crate::query::SearchParams::for_query(&k.text) }, class: "muted", "vezi pe site" }
                        form { class: "inline", method: "post", action: "/cont/cuvinte/{k.id}/sterge",
                            button { r#type: "submit", class: "link danger", "Șterge" }
                        }
                    }
                }
            }
        }
        if acc.credits_balance > 0 {
            form { class: "account-form", method: "post", action: "/cont/cuvinte",
                label {
                    "Cuvânt-cheie nou (o stradă, un cartier, un beneficiar)"
                    input { r#type: "text", name: "cuvant", required: true, maxlength: "60", placeholder: "ex.: Viile Dâmbul Rotund" }
                }
                button { r#type: "submit", "Adaugă (1 credit)" }
            }
        } else {
            p { class: "muted", "Nu mai ai credite anul acesta; se reînnoiesc pe {reset}." }
        }
        details { class: "danger-zone",
            summary { "Șterge contul" }
            p { class: "muted", "Șterge adresa de email, cuvintele-cheie și istoricul alertelor. Nu se poate anula." }
            form { class: "inline", method: "post", action: "/cont/sterge",
                input { r#type: "hidden", name: "confirm", value: "da" }
                button { r#type: "submit", class: "link danger", "Șterge contul definitiv" }
            }
        }
    }
}
