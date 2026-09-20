//! Pagina principală (FR-4.1): formular clasic GET (merge și fără wasm) și rezultatele căutării.
//! Fără text și filtre arată cele mai recente proiecte.

use chrono::{Datelike, Utc};
use dioxus::prelude::*;
use urban_shared::Category;

use super::Route;
use super::components::{AgendaRowCard, ItemCard};
use crate::query::SearchParams;
use crate::server_fns;

#[component]
pub fn Home(params: SearchParams) -> Element {
    let results = use_server_future(use_reactive!(|params| server_fns::search(params.to_query())))?;

    let title = if params.q.trim().is_empty() {
        "urban.monitor · Ședințe CTATU Cluj-Napoca".to_owned()
    } else {
        format!("{} · urban.monitor", params.q.trim())
    };
    let this_year = Utc::now().year().max(2026);
    let years: Vec<i32> = (2026..=this_year).rev().collect();

    rsx! {
        document::Title { "{title}" }
        form { class: "search", method: "get", action: "/",
            div { class: "row",
                input {
                    r#type: "search",
                    name: "q",
                    value: "{params.q}",
                    placeholder: "Strada, beneficiarul sau un cuvânt din titlu",
                    autocomplete: "off",
                    autofocus: true,
                }
                button { r#type: "submit", "Caută" }
            }
            fieldset { class: "filters",
                for c in Category::ALL {
                    label {
                        input { r#type: "checkbox", name: "tip", value: "{c.as_str()}", checked: params.has_category(c) }
                        "{c.label()}"
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
                            "Cele mai recente proiecte discutate în comisie ({r.total} în total)"
                        } else if r.total == 0 {
                            "Niciun proiect nu se potrivește. Încearcă doar numele străzii, fără „strada” și fără număr."
                        } else {
                            "{r.total} proiecte găsite"
                        }
                    }
                    ul { class: "items",
                        for it in r.items.iter() {
                            ItemCard { key: "{it.id}", item: it.clone() }
                        }
                    }
                    if !r.agenda_only.is_empty() {
                        h2 { "În ordinea de zi, fără proiect publicat pe site" }
                        ul { class: "items",
                            for row in r.agenda_only.iter() {
                                AgendaRowCard { key: "{row.id}", row: row.clone() }
                            }
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
fn Pager(params: SearchParams, total: u32) -> Element {
    let page = SearchParams::PAGE;
    let prev = (params.offset > 0).then(|| params.with_offset(params.offset.saturating_sub(page)));
    let next = (params.offset + page < total).then(|| params.with_offset(params.offset + page));
    if prev.is_none() && next.is_none() {
        return rsx! {};
    }
    rsx! {
        nav { class: "pager",
            if let Some(p) = prev {
                Link { to: Route::Home { params: p }, "← Anterioarele {page}" }
            } else {
                span {}
            }
            if let Some(n) = next {
                Link { to: Route::Home { params: n }, "Următoarele {page} →" }
            }
        }
    }
}
