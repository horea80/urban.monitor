//! Componente refolosite de pagini: cardul unui proiect, rândul de agendă fără proiect, cardul unui
//! certificat de urbanism, insignele de categorie și de tip, data ultimei sincronizări din antet,
//! subsolul cu sursa.

use chrono::{DateTime, NaiveDate, Utc};
use dioxus::prelude::*;
use urban_shared::text::{fmt_thousands, normalize};
use urban_shared::{AgendaRowView, Category, Certificate, CertificateKind, Item};

use super::Route;
use crate::query::{CertificateParams, SearchParams};
use crate::server_fns;

/// „16 septembrie 2026”
pub fn fmt_date(d: NaiveDate) -> String {
    urban_shared::time::fmt_date_ro(d)
}

/// Moment ISO 8601 (UTC) → „20.09.2026, 23:14” în ora României; textul original dacă nu se poate parsa.
pub fn fmt_timestamp(iso: &str) -> String {
    DateTime::parse_from_rfc3339(iso)
        .map(|t| {
            urban_shared::time::to_romania_local(t.with_timezone(&Utc))
                .format("%d.%m.%Y, %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| iso.to_owned())
}

#[component]
pub fn CategoryBadge(category: Category) -> Element {
    rsx! {
        Link {
            class: "badge badge-{category.as_str()}",
            to: Route::Home { params: SearchParams::only_category(category) },
            "{category.label()}"
        }
    }
}

#[component]
pub fn ItemCard(item: Item) -> Element {
    let date = fmt_date(item.meeting_date);
    rsx! {
        li { class: "item",
            div { class: "meta",
                Link { to: Route::MeetingPage { id: item.meeting_id }, "Ședința din {date}" }
                CategoryBadge { category: item.category }
                if item.revenire {
                    span { class: "tag", "revenire CTATU" }
                }
                if let Some(nr) = &item.reg_number {
                    span { class: "muted",
                        "nr. {nr}"
                        if let Some(d) = &item.reg_date { " / {d}" }
                    }
                }
            }
            h3 { class: "title",
                a { href: "{item.url}", target: "_blank", rel: "noopener", "{item.title}" }
            }
            if let Some(a) = &item.address {
                p { class: "address",
                    "{a}"
                    if let Some(b) = &item.beneficiary {
                        span { class: "beneficiary", " · {b}" }
                    }
                }
            } else if let Some(b) = &item.beneficiary {
                p { class: "beneficiary", "{b}" }
            }
            div { class: "docs",
                a { class: "project-link", href: "{item.url}", target: "_blank", rel: "noopener", "Pagina proiectului ↗" }
                for d in item.documents.iter() {
                    a { href: "{d.url}", target: "_blank", rel: "noopener", "{d.label}" }
                }
            }
        }
    }
}

#[component]
pub fn AgendaRowCard(row: AgendaRowView) -> Element {
    let date = fmt_date(row.meeting_date);
    rsx! {
        li { class: "item agenda-only",
            div { class: "meta",
                Link { to: Route::MeetingPage { id: row.meeting_id }, "Ședința din {date}" }
                CategoryBadge { category: row.category }
                span { class: "tag", "doar în ordinea de zi" }
                if row.revenire {
                    span { class: "tag", "revenire CTATU" }
                }
                if let Some(nr) = &row.reg_number {
                    span { class: "muted", "nr. {nr}" }
                }
            }
            p { class: "title", "{row.description}" }
            if let Some(b) = &row.beneficiary {
                p { class: "beneficiary", "{b}" }
            }
        }
    }
}

#[component]
pub fn KindBadge(kind: CertificateKind) -> Element {
    rsx! {
        Link {
            class: "badge badge-{kind.as_str()}",
            to: Route::Certificates { params: CertificateParams::only_kind(kind) },
            "{kind.label()}"
        }
    }
}

/// Un certificat de urbanism (FR-10.3): data emiterii, tipul, numărul cu link către primărie,
/// adresa, scopul declarat (ascuns când e doar „informare”, adică tipul însuși) și, la PUZ și PUD,
/// suprafața, UTR-urile, folosința actuală, CF și cadastralul de pe pagina certificatului (FR-10.6).
#[component]
pub fn CertificateCard(cert: Certificate) -> Element {
    let date = fmt_date(cert.date);
    let show_scop = !cert.scop.is_empty() && normalize(&cert.scop) != "informare";
    let has_land = cert.surface_mp.is_some() || cert.utr.is_some() || cert.land_use.is_some();
    let has_cf = cert.cf.is_some() || cert.cadastral.is_some();
    rsx! {
        li { class: "item certificate",
            div { class: "meta",
                span { "Emis {date}" }
                KindBadge { kind: cert.kind }
            }
            h3 { class: "title",
                a { href: "{cert.url}", target: "_blank", rel: "noopener", "{cert.title()}" }
            }
            if let Some(a) = &cert.address {
                p { class: "address", "{a}" }
            }
            if has_land {
                p { class: "details",
                    if let Some(mp) = cert.surface_mp {
                        span { class: "detail", title: "Suprafața terenului", "{fmt_thousands(mp)} mp" }
                    }
                    if let Some(u) = &cert.utr {
                        span { class: "detail", title: "Unitatea teritorială de referință din PUG", "UTR {u}" }
                    }
                    if let Some(l) = &cert.land_use {
                        span { class: "detail", title: "Folosința actuală", "{l}" }
                    }
                }
            }
            if has_cf {
                p { class: "details muted",
                    if let Some(cf) = &cert.cf {
                        span { class: "detail", "CF {cf}" }
                    }
                    if let Some(cad) = &cert.cadastral {
                        span { class: "detail", "cadastral {cad}" }
                    }
                }
            }
            if show_scop {
                p { class: "scop", "{cert.scop}" }
            }
            div { class: "docs",
                a { class: "project-link", href: "{cert.url}", target: "_blank", rel: "noopener", "Certificatul pe site-ul primăriei ↗" }
            }
        }
    }
}

/// „Actualizat 21.09.2026, 14:22” în antet: ultima sincronizare reușită, în ora României.
#[component]
pub fn LastUpdate() -> Element {
    let status = use_server_future(server_fns::status)?;
    let text = match status() {
        Some(Ok(s)) => match s.last_run.and_then(|r| r.finished_at) {
            Some(t) => format!("Actualizat {}", fmt_timestamp(&t)),
            None => "Nicio sincronizare încă".to_owned(),
        },
        _ => String::new(),
    };
    rsx! {
        span { class: "updated", title: "Ultima sincronizare cu site-ul primăriei, ora României", "{text}" }
    }
}

#[component]
pub fn Footer() -> Element {
    rsx! {
        footer {
            div { class: "inner",
                a {
                    href: "https://primariaclujnapoca.ro/strategii-urbane/comisia-tehnica-de-amenajare-a-teritoriului-si-urbanism/sedinte-comisie/",
                    target: "_blank",
                    rel: "noopener",
                    "Sursa: primariaclujnapoca.ro"
                }
            }
        }
    }
}
