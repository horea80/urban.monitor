//! Componente refolosite de pagini: cardul unui proiect, rândul de agendă fără proiect, insigna de
//! categorie, data ultimei sincronizări din antet, subsolul cu sursa.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use dioxus::prelude::*;
use urban_shared::{AgendaRowView, Category, Item};

use super::Route;
use crate::query::SearchParams;
use crate::server_fns;

const LUNI: [&str; 12] = [
    "ianuarie",
    "februarie",
    "martie",
    "aprilie",
    "mai",
    "iunie",
    "iulie",
    "august",
    "septembrie",
    "octombrie",
    "noiembrie",
    "decembrie",
];

/// „16 septembrie 2026”
pub fn fmt_date(d: NaiveDate) -> String {
    format!("{} {} {}", d.day(), LUNI[d.month0() as usize], d.year())
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
            if !item.documents.is_empty() {
                div { class: "docs",
                    for d in item.documents.iter() {
                        a { href: "{d.url}", target: "_blank", rel: "noopener", "{d.label}" }
                    }
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
