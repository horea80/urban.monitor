//! Pagina unei ședințe (FR-4.2): antet cu linkurile oficiale, proiectele și ordinea de zi completă.

use dioxus::prelude::*;

use super::Route;
use super::components::{ItemCard, fmt_date};
use crate::server_fns;

#[component]
pub fn MeetingPage(id: i64) -> Element {
    let detail = use_server_future(use_reactive!(|id| server_fns::meeting_detail(id)))?;

    match detail() {
        None => rsx! {
            p { class: "muted", "Se încarcă…" }
        },
        Some(Err(e)) => rsx! {
            p { class: "error", "Nu am putut încărca ședința: {e}" }
        },
        Some(Ok(None)) => rsx! {
            h1 { "Ședința nu există" }
            Link { to: Route::Meetings {}, "Toate ședințele" }
        },
        Some(Ok(Some(d))) => {
            let date = fmt_date(d.meeting.date);
            let n_items = d.items.len();
            let n_rows = d.agenda_rows.len();
            let unmatched = d.agenda_rows.iter().filter(|r| r.item_id.is_none()).count();
            rsx! {
                section { class: "meeting-head",
                    h1 { "Ședința CTATU din {date}" }
                    p { class: "muted",
                        if d.meeting.upcoming {
                            span { class: "tag upcoming", "programată" }
                            " · "
                        }
                        if let Some(t) = &d.meeting.time { "Ora {t} · " }
                        "{n_items} proiecte"
                        if n_rows > 0 { " · {n_rows} poziții pe ordinea de zi" }
                    }
                    div { class: "links",
                        a { href: "{d.meeting.url}", target: "_blank", rel: "noopener", "Pagina ședinței pe primariaclujnapoca.ro" }
                        if let Some(u) = &d.meeting.agenda_url {
                            a { href: "{u}", target: "_blank", rel: "noopener", "Ordinea de zi (PDF)" }
                        }
                        if let Some(u) = &d.meeting.conclusions_pdf_url {
                            a { href: "{u}", target: "_blank", rel: "noopener", "Concluziile ședinței (PDF)" }
                        }
                        if let Some(u) = &d.meeting.announcement_url {
                            a { href: "{u}", target: "_blank", rel: "noopener", "Anunțul ședinței" }
                        }
                    }
                }

                h2 { "Proiecte" }
                if d.items.is_empty() {
                    p { class: "notice", "Pagina ședinței nu are proiecte publicate." }
                }
                ul { class: "items",
                    for it in d.items.iter() {
                        ItemCard { key: "{it.id}", item: it.clone() }
                    }
                }

                if n_rows > 0 {
                    h2 { "Ordinea de zi" }
                    if unmatched > 0 {
                        p { class: "muted", "{unmatched} poziții nu au un proiect publicat pe site și apar doar aici." }
                    }
                    div { class: "table-wrap",
                        table { class: "agenda",
                            thead {
                                tr {
                                    th { "Nr." }
                                    th { "Nr. înregistrare" }
                                    th { "Beneficiar" }
                                    th { "Lucrare" }
                                }
                            }
                            tbody {
                                for r in d.agenda_rows.iter() {
                                    tr { key: "{r.id}",
                                        td { class: "nr",
                                            if let Some(n) = r.nr { "{n}" }
                                        }
                                        td {
                                            if let Some(x) = &r.reg_number { "{x}" }
                                            if let Some(dd) = &r.reg_date {
                                                br {}
                                                span { class: "muted", "{dd}" }
                                            }
                                            if r.revenire {
                                                br {}
                                                span { class: "tag", "revenire" }
                                            }
                                        }
                                        td {
                                            if let Some(b) = &r.beneficiary { "{b}" }
                                        }
                                        td {
                                            "{r.description}"
                                            if r.item_id.is_none() {
                                                " "
                                                span { class: "tag", "fără proiect publicat" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if d.meeting.agenda_url.is_some() {
                    p { class: "notice", "Ordinea de zi nu a putut fi citită: PDF scanat, fără strat de text." }
                }
            }
        }
    }
}
