//! Lista ședințelor, cele mai noi întâi.

use dioxus::prelude::*;

use super::Route;
use super::components::fmt_date;
use crate::server_fns;

#[component]
pub fn Meetings() -> Element {
    let list = use_server_future(server_fns::list_meetings)?;
    rsx! {
        h1 { "Ședințele comisiei" }
        match list() {
            Some(Ok(ms)) => rsx! {
                ul { class: "meetings",
                    for m in ms.iter() {
                        li { key: "{m.id}",
                            Link { to: Route::MeetingPage { id: m.id }, "Ședința din {fmt_date(m.date)}" }
                            if m.upcoming {
                                span { class: "tag upcoming", "programată" }
                            }
                            if let Some(t) = &m.time {
                                span { class: "muted", "ora {t}" }
                            }
                            span { class: "n", "{m.item_count} proiecte" }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! {
                p { class: "error", "Eroare: {e}" }
            },
            None => rsx! {
                p { class: "muted", "Se încarcă…" }
            },
        }
    }
}
