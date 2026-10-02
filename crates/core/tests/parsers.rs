mod common;

use chrono::{Datelike, NaiveDate};
use common::*;
use urban_core::scrape::{
    first_pdf, parse_certificate_detail, parse_certificates, parse_documents, parse_listing, parse_meeting,
};
use urban_core::shared::CertificateKind;
use urban_core::shared::text::{classify_scop, parse_work_address};

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

#[test]
fn certificates_page_lists_24_newest_first() {
    let url = "https://primariaclujnapoca.ro/urbanism/certificate-de-urbanism/documente-emise/";
    let page = parse_certificates(&fixture("certificates-page-1.html"), url);
    assert_eq!(page.certificates.len(), 24);
    assert!(page.has_next);

    let first = &page.certificates[0];
    assert_eq!(
        first.url,
        "https://primariaclujnapoca.ro/urbanism/certificate-de-urbanism/certificat-de-urbanism-1733-din-2026/"
    );
    assert_eq!((first.number, first.year), (1733, 2026));
    assert_eq!(first.date, ymd(2026, 10, 1).unwrap());
    assert_eq!(first.scop, "INFORMARE");
    assert_eq!(
        first.address.as_deref(),
        Some("judetul Cluj, municipiul Cluj-Napoca, CĂPITAN GRIGORE IGNAT, nr. 28")
    );
    let addr = parse_work_address(first.address.as_deref().unwrap());
    assert_eq!(addr.street.as_deref(), Some("CĂPITAN GRIGORE IGNAT"));
    assert_eq!(addr.street_no.as_deref(), Some("28"));

    // al doilea nu are adresă dincolo de județ și municipiu: strada lipsește
    let second = &page.certificates[1];
    assert_eq!((second.number, second.year), (1732, 2026));
    let addr = parse_work_address(second.address.as_deref().unwrap_or(""));
    assert_eq!(addr.street, None);

    // ordinea e descrescătoare după dată, numerele sunt din 2026, fără duplicate
    assert!(page.certificates.windows(2).all(|w| w[0].date >= w[1].date));
    assert!(
        page.certificates
            .iter()
            .all(|c| c.year == 2026 && c.date.year() == 2026)
    );
    let mut urls: Vec<&str> = page.certificates.iter().map(|c| c.url.as_str()).collect();
    urls.sort_unstable();
    urls.dedup();
    assert_eq!(urls.len(), 24);

    // 10 din 24 au stradă și număr; 6 doar stradă („nr. FN”, „nr. f.nr.”, „nr. FM” înseamnă fără număr);
    // 8 nu au nimic dincolo de județ și municipiu
    let parsed: Vec<_> = page
        .certificates
        .iter()
        .map(|c| parse_work_address(c.address.as_deref().unwrap_or("")))
        .collect();
    assert_eq!(
        parsed
            .iter()
            .filter(|a| a.street.is_some() && a.street_no.is_some())
            .count(),
        10
    );
    assert_eq!(
        parsed
            .iter()
            .filter(|a| a.street.is_some() && a.street_no.is_none())
            .count(),
        6
    );
    assert_eq!(parsed.iter().filter(|a| a.street.is_none()).count(), 8);
    let faget = parsed
        .iter()
        .find(|a| a.street.as_deref() == Some("FAGET"))
        .expect("Colonia Făget");
    assert_eq!(faget.street_no, None);

    // tipurile de pe prima pagină: informarea domină
    let kinds: Vec<CertificateKind> = page.certificates.iter().map(|c| classify_scop(&c.scop)).collect();
    assert_eq!(kinds.iter().filter(|k| **k == CertificateKind::Informare).count(), 15);
    assert_eq!(kinds.iter().filter(|k| **k == CertificateKind::Construire).count(), 4);
    assert_eq!(kinds.iter().filter(|k| **k == CertificateKind::Operatiuni).count(), 4);
    assert_eq!(kinds.iter().filter(|k| **k == CertificateKind::Altele).count(), 1);
}

#[test]
fn certificate_detail_pages_puz_and_pud() {
    // PUZ: fără CF și cadastral („identificat prin plan de încadrare în zonă”), dar cu suprafață, UTR-uri și folosință
    let d = parse_certificate_detail(&fixture("certificate-1685-2026-puz.html"));
    assert_eq!(d.surface_mp, Some(26677));
    // „Destinația: UTR=ULC, …; UTR=Lc, …, UTR=VE”; „Lc” e același cod cu „LC” din regimul tehnic, iar
    // subzonele din textul de regulament (UIs, UVa, UEt) nu sunt ale parcelei și nu trebuie să apară
    assert_eq!(d.utr, vec!["LC", "ULC", "VE"]);
    assert_eq!(d.land_use.as_deref(), Some("terenuri: arabil, livada, drum"));
    assert_eq!(d.cf, None);
    assert_eq!(d.cadastral, None);

    // PUD: parcela e identificată exact
    let d = parse_certificate_detail(&fixture("certificate-1647-2026-pud.html"));
    assert_eq!(d.surface_mp, Some(608));
    assert_eq!(d.utr, vec!["RRM1"]);
    assert_eq!(
        d.land_use.as_deref(),
        Some("teren (curți construcții), construcția C1 - Casă, construcția C2 - Cabinet medical P+E+M")
    );
    assert_eq!(d.cf.as_deref(), Some("299486"));
    assert_eq!(d.cadastral.as_deref(), Some("299486, 299486-C1, 299486-C2"));
}
