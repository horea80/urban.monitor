mod common;

use chrono::{Datelike, NaiveDate};
use common::*;
use urban_core::scrape::{first_pdf, parse_documents, parse_listing, parse_meeting};

fn ymd(y: i32, m: u32, d: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(y, m, d)
}

#[test]
fn listing_lists_meetings_newest_first() {
    let refs = parse_listing(&fixture("listing.html"), LISTING_URL);
    assert!(refs.len() >= 60, "prea puține ședințe: {}", refs.len());
    let in_2026 = refs.iter().filter(|r| r.date.is_some_and(|d| d.year() == 2026)).count();
    assert_eq!(in_2026, 20);
    assert_eq!(refs[0].url, MEETING_0916);
    assert_eq!(refs[0].title, "Ședința din 16 septembrie 2026");
    assert_eq!(refs[0].date, ymd(2026, 9, 16));
    assert!(refs.iter().all(|r| r.date.is_some()), "toate ședințele au dată");
    // fără duplicate
    let mut urls: Vec<&str> = refs.iter().map(|r| r.url.as_str()).collect();
    urls.sort_unstable();
    urls.dedup();
    assert_eq!(urls.len(), refs.len());
}

#[test]
fn meeting_2026_09_16_header_and_cards() {
    let p = parse_meeting(&fixture("meeting-2026-09-16.html"), MEETING_0916);
    assert_eq!(p.title, "Ședința din 16 septembrie 2026");
    assert_eq!(p.date, ymd(2026, 9, 16));
    assert_eq!(p.time.as_deref(), Some("10:00"));
    assert_eq!(
        p.agenda_url.as_deref(),
        Some("https://files.primariaclujnapoca.ro/2026/09/10/Ordine-de-zi-sedinta-CTATU-16-septembrie-2026.pdf")
    );
    assert_eq!(p.cards.len(), 15);
    assert_eq!(p.projects().count(), 13);
    let c = p.conclusions().expect("card concluzii");
    assert_eq!(
        c.url,
        "https://primariaclujnapoca.ro/urbanism/proiecte-de-urbanism/concluziile-sedintei-41/"
    );
    let a = p.announcement().expect("card anunț");
    assert!(a.url.ends_with("/anunt-privind-desfasurarea-sedintei-115/"));

    let b = p
        .projects()
        .find(|c| c.url.ends_with("/p-u-d-construire-imobil-mixt-118/"))
        .expect("card Brâncuși");
    assert_eq!(b.title, "P.U.D construire imobil mixt");
    assert_eq!(b.address.as_deref(), Some("str C-tin Brancusi nr 107-109"));
    assert_eq!(b.published_at.as_deref(), Some("2026-09-10T14:43:59+03:00"));

    let v = p.projects().find(|c| c.title.contains("Vidrei")).expect("card Vidrei");
    assert_eq!(v.address.as_deref(), Some("str. Morii nr. 31g"));
}

#[test]
fn other_2026_meetings_parse() {
    let p = parse_meeting(&fixture("meeting-2026-01-14.html"), MEETING_0114);
    assert_eq!(p.date, ymd(2026, 1, 14));
    assert_eq!(p.time.as_deref(), Some("10:30"));
    assert_eq!(p.projects().count(), 17);
    assert_eq!(
        p.agenda_url.as_deref(),
        Some("https://files.primariaclujnapoca.ro/2026/01/09/Ordine-de-zi-CTATU-14-01-2026.pdf")
    );

    let p = parse_meeting(&fixture("meeting-2026-05-27.html"), MEETING_0527);
    assert_eq!(p.date, ymd(2026, 5, 27));
    assert_eq!(p.time.as_deref(), Some("12:00"));
    assert_eq!(p.projects().count(), 10);
    assert!(p.conclusions().is_some());
}

#[test]
fn project_pages_list_documents() {
    let url = "https://primariaclujnapoca.ro/urbanism/proiecte-de-urbanism/p-u-d-construire-imobil-mixt-118/";
    let docs = parse_documents(&fixture("project-pud-imobil-mixt-118.html"), url);
    let labels: Vec<&str> = docs.iter().map(|d| d.label.as_str()).collect();
    assert_eq!(labels, vec!["parte scrisă", "parte desenată", "adresa"]);
    assert_eq!(
        docs[0].url,
        "https://files.primariaclujnapoca.ro/2026/09/10/parte-scrisa.pdf"
    );
    assert!(
        docs.iter()
            .all(|d| d.url.starts_with("https://files.primariaclujnapoca.ro/"))
    );

    let url =
        "https://primariaclujnapoca.ro/urbanism/proiecte-de-urbanism/studiu-de-oportunitate-pentru-initiere-puz-38/";
    let docs = parse_documents(&fixture("project-studiu-oportunitate-puz-38.html"), url);
    let labels: Vec<&str> = docs.iter().map(|d| d.label.as_str()).collect();
    assert_eq!(labels, vec!["Parte scrisă", "Parte desenată"]);
}

#[test]
fn conclusions_page_has_scanned_pdf() {
    let url = "https://primariaclujnapoca.ro/urbanism/proiecte-de-urbanism/concluziile-sedintei-41/";
    let docs = parse_documents(&fixture("conclusions-page-41.html"), url);
    assert_eq!(
        first_pdf(&docs),
        Some("https://files.primariaclujnapoca.ro/2026/09/16/202609161352-1.pdf")
    );
}

#[test]
fn announcement_page_has_no_documents() {
    let url = "https://primariaclujnapoca.ro/urbanism/proiecte-de-urbanism/anunt-privind-desfasurarea-sedintei-115/";
    let docs = parse_documents(&fixture("announcement-page-115.html"), url);
    assert!(docs.iter().all(|d| !d.url.to_lowercase().contains("anti-mita")));
    assert!(first_pdf(&docs).is_none(), "{docs:?}");
}
