mod common;

use common::*;
use urban_core::agenda::{AgendaRow, match_rows, parse_agenda};
use urban_core::pdf::{PdfExtract, PdfText};
use urban_core::scrape::{Card, parse_meeting};

fn rows(name: &str) -> Vec<AgendaRow> {
    let text = PdfExtract.extract(&fixture_bytes(name)).expect("text din PDF");
    parse_agenda(&text)
}

fn numbered(rows: &[AgendaRow]) -> Vec<u32> {
    rows.iter().filter_map(|r| r.nr).collect()
}

#[test]
fn agenda_2026_09_16_has_13_rows_with_glued_numbers() {
    let r = rows("agenda-2026-09-16.pdf");
    assert_eq!(r.len(), 13, "{r:#?}");
    assert_eq!(numbered(&r), (1..=13).collect::<Vec<_>>());

    assert_eq!(r[0].reg_number.as_deref(), Some("723388"));
    assert_eq!(r[0].reg_date.as_deref(), Some("14.08.2026"));
    assert_eq!(r[0].beneficiary.as_deref(), Some("Paun Adrian si alții"));
    assert!(r[0].description.contains("Borhanciului"));

    assert!(r[3].revenire, "rândul 4 e revenire CTATU");
    assert_eq!(r[3].beneficiary.as_deref(), Some("Marin Aurel"));
    assert!(r[3].description.contains("Ceahlău"));

    // lipite: „10739800/24.08.2026Marian Ramona Laura”
    assert_eq!(r[9].reg_number.as_deref(), Some("739800"));
    assert_eq!(r[9].beneficiary.as_deref(), Some("Marian Ramona Laura"));
    assert_eq!(r[10].reg_number.as_deref(), Some("746988"));
    assert_eq!(r[10].beneficiary.as_deref(), Some("Batiment Vert SRL"));
    assert!(r[10].description.contains("Brancusi"));
    assert_eq!(r[12].beneficiary.as_deref(), Some("Popa Cristian"));
}

#[test]
fn agenda_2026_01_14_has_17_rows() {
    let r = rows("agenda-2026-01-14.pdf");
    assert_eq!(r.len(), 17, "{r:#?}");
    assert_eq!(numbered(&r), (1..=17).collect::<Vec<_>>());
    assert_eq!(r[0].beneficiary.as_deref(), Some("Baile Someseni"));
    assert_eq!(r[1].reg_date.as_deref(), Some("2.12.2025"));
    assert_eq!(r[1].beneficiary.as_deref(), Some("Maglio Construct Investment S.R.L"));
    assert!(r[1].description.contains("Odobesti"));
    assert_eq!(
        r[7].beneficiary.as_deref(),
        Some("Patiu Sorin Vasile și Grecu Marius Gheorghe")
    );
    assert!(r[10].revenire);
    assert_eq!(r[10].beneficiary.as_deref(), Some("Salanta Marius Daniel"));
    assert!(r[10].description.contains("Doinei 95"));
    assert!(r[16].description.contains("Suceava"));
}

#[test]
fn agenda_2026_05_27_has_10_rows() {
    let r = rows("agenda-2026-05-27.pdf");
    assert_eq!(r.len(), 10, "{r:#?}");
    assert_eq!(r[0].beneficiary.as_deref(), Some("SC Lukakom Invest S.R.L"));
    assert!(r[0].description.contains("Tribunul Vladuțiu"));
    assert_eq!(r[2].beneficiary.as_deref(), Some("Hexagon Town S.R.L"));
    assert!(r[8].description.starts_with("P.U.D modificari interioare"));
    assert_eq!(r[8].beneficiary.as_deref(), Some("Anghel Dumitru Marcel"));
}

#[test]
fn rows_match_published_cards() {
    let page = parse_meeting(&fixture("meeting-2026-09-16.html"), MEETING_0916);
    let cards: Vec<Card> = page.projects().cloned().collect();
    let r = rows("agenda-2026-09-16.pdf");
    let m = match_rows(&r, &cards);
    let matched = m.iter().flatten().count();
    assert!(matched >= 12, "doar {matched} din 13 potrivite: {m:?}");

    let brancusi_row = r.iter().position(|x| x.description.contains("Brancusi")).unwrap();
    let card = &cards[m[brancusi_row].expect("rândul Brâncuși are card")];
    assert!(card.url.ends_with("/p-u-d-construire-imobil-mixt-118/"));

    let page = parse_meeting(&fixture("meeting-2026-01-14.html"), MEETING_0114);
    let cards: Vec<Card> = page.projects().cloned().collect();
    let r = rows("agenda-2026-01-14.pdf");
    let m = match_rows(&r, &cards);
    let matched = m.iter().flatten().count();
    assert!(matched >= 15, "doar {matched} din 17 potrivite: {m:?}");
    // Doinei 93 și 95 nu se încurcă între ele
    for (row, idx) in r.iter().zip(&m) {
        if let Some(i) = idx {
            if row.description.contains("Doinei 93") {
                assert!(cards[*i].address.as_deref().unwrap_or("").contains("93"));
            }
            if row.description.contains("Doinei 95") {
                assert!(cards[*i].address.as_deref().unwrap_or("").contains("95"));
            }
        }
    }
}

#[test]
fn scanned_pdf_yields_no_text() {
    let text = PdfExtract
        .extract(&fixture_bytes("conclusions-2026-09-16-scanned.pdf"))
        .expect("fără eroare");
    assert!(text.trim().is_empty());
    assert!(parse_agenda(&text).is_empty());
}
