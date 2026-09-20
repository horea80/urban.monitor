//! Descarcare politicoasa si parsare a paginilor de pe site-ul primariei.

pub mod client;
pub mod listing;
pub mod meeting;
pub mod project;

pub use client::Client;
pub use listing::{parse_listing, MeetingRef};
pub use meeting::{parse_meeting, Card, CardKind, MeetingPage};
pub use project::{first_pdf, parse_documents};

/// Textul unui element, cu spatiile normalizate.
pub(crate) fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Rezolva un href relativ fata de pagina curenta.
pub(crate) fn resolve(base: &url::Url, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') || href.starts_with("mailto:") || href.starts_with("tel:") {
        return None;
    }
    base.join(href).ok().map(|u| u.to_string())
}
