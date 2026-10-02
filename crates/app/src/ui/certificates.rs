//! Certificatele de urbanism emise (FR-10.3): formular clasic GET (merge și fără wasm) și lista
//! rezultatelor. Fără text și filtre arată cele mai recente certificate.

use chrono::{Datelike, Utc};
use dioxus::prelude::*;
use urban_shared::CertificateKind;

use super::Route;
use super::components::CertificateCard;
use crate::query::CertificateParams;
use crate::server_fns;

/// Primul an din filtrul de ani; sincronizarea pornește implicit tot din 2024 (`URBAN_CERT_START_YEAR`).
const FIRST_YEAR: i32 = 2024;

#[component]
pub fn Certificates(params: CertificateParams) -> Element {
    let results = use_server_future(use_reactive!(|params| {
        server_fns::search_certificates(params.to_query())
    }))?;

    let this_year = Utc::now().year().max(FIRST_YEAR);
    let years: Vec<i32> = (FIRST_YEAR..=this_year).rev().collect();

    rsx! {
        document::Title { "Certificate de urbanism · urban.monitor" }
        h1 { "Certificate de urbanism emise" }
        p { class: "muted",
            "Primăria publică fiecare certificat emis, cu scopul și adresa lucrării. Un certificat de informare nu "
            "anunță o lucrare; unul pentru autorizare, PUZ sau PUD, de obicei, da."
        }
        form { class: "search", method: "get", action: "/certificate",
            div { class: "row",
                input {
                    r#type: "search",
                    name: "q",
                    value: "{params.q}",
                    placeholder: "Strada, numărul sau un cuvânt din scop",
                    autocomplete: "off",
                    autofocus: true,
                }
                button { r#type: "submit", "Caută" }
            }
            fieldset { class: "filters",
                for k in CertificateKind::ALL {
                    label {
                        input { r#type: "checkbox", name: "tip", value: "{k.as_str()}", checked: params.has_kind(k) }
                        "{k.label()}"
                    }
                }
                label {
                    "Anul: "
                    select { name: "an",
                        option { value: "", selected: params.an.is_none(), "toți" }
                        for y in years.iter() {
                            option { value: "{y}", selected: params.an == Some(*y), "{y}" }
                        }
                    }
                }
            }
        }
        section { class: "results",
            match results() {
                Some(Ok(r)) => rsx! {
                    p { class: "count",
                        if params.is_blank() {
                            "Cele mai recente certificate emise ({r.total} în total)"
                        } else if r.total == 0 {
                            "Niciun certificat nu se potrivește. Încearcă doar numele străzii, fără „strada” și fără număr."
                        } else {
                            "{r.total} certificate găsite"
                        }
                    }
                    ul { class: "items",
                        for c in r.items.iter() {
                            CertificateCard { key: "{c.id}", cert: c.clone() }
                        }
                    }
                    Pager { params: params.clone(), total: r.total }
                },
                Some(Err(e)) => rsx! {
                    p { class: "error", "Căutarea a eșuat: {e}" }
                },
                None => rsx! {
                    p { class: "muted", "Se încarcă…" }
                },
            }
        }
    }
}

#[component]
fn Pager(params: CertificateParams, total: u32) -> Element {
    let page = CertificateParams::PAGE;
    let prev = (params.offset > 0).then(|| params.with_offset(params.offset.saturating_sub(page)));
    let next = (params.offset + page < total).then(|| params.with_offset(params.offset + page));
    if prev.is_none() && next.is_none() {
        return rsx! {};
    }
    rsx! {
        nav { class: "pager",
            if let Some(p) = prev {
                Link { to: Route::Certificates { params: p }, "← Anterioarele {page}" }
            } else {
                span {}
            }
            if let Some(n) = next {
                Link { to: Route::Certificates { params: n }, "Următoarele {page} →" }
            }
        }
    }
}
