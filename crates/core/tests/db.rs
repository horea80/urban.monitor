use chrono::NaiveDate;
use urban_core::db::{
    AgendaRowWrite, CertificateDetailWrite, CertificateWrite, Db, ItemWrite, MeetingWrite, RunCounts, fts_query,
};
use urban_core::shared::{Category, CertificateKind, CertificateQuery, Document, SearchQuery};

fn open() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    (dir, db)
}

fn meeting_0916() -> MeetingWrite {
    MeetingWrite {
        url: "https://x/sedinta-din-16-septembrie-2026/".into(),
        title: "Ședința din 16 septembrie 2026".into(),
        date: NaiveDate::from_ymd_opt(2026, 9, 16).unwrap(),
        time: Some("10:00".into()),
        agenda_url: Some("https://files/oz.pdf".into()),
        agenda_text: Some("text".into()),
        conclusions_pdf_url: None,
        announcement_url: None,
        items: vec![
            ItemWrite {
                url: "https://x/p-u-d-construire-imobil-mixt-118/".into(),
                title: "P.U.D construire imobil mixt".into(),
                address: Some("str C-tin Brancusi nr 107-109".into()),
                street: Some("C-tin Brancusi".into()),
                category: Category::Pud,
                published_at: Some("2026-09-10T14:43:59+03:00".into()),
                beneficiary: Some("Batiment Vert SRL".into()),
                reg_number: Some("746988".into()),
                reg_date: Some("27.08.2026".into()),
                revenire: false,
                agenda_description: Some("P.U.D construire imobil mixt str C-tin Brancusi nr 107-109".into()),
                documents: Some(vec![Document {
                    label: "parte scrisă".into(),
                    url: "https://files/ps.pdf".into(),
                }]),
            },
            ItemWrite {
                url: "https://x/studiu-7/".into(),
                title: "Studiu de oportunitate pentru inițiere elaborare P.U.Z".into(),
                address: Some("str. Câmpului nr. 333".into()),
                street: Some("Câmpului".into()),
                category: Category::AvizOportunitate,
                documents: Some(vec![]),
                ..Default::default()
            },
        ],
        agenda_rows: Some(vec![
            AgendaRowWrite {
                nr: Some(11),
                reg_number: Some("746988".into()),
                description: "P.U.D construire imobil mixt str C-tin Brancusi nr 107-109".into(),
                item_index: Some(0),
                ..Default::default()
            },
            AgendaRowWrite {
                nr: Some(12),
                beneficiary: Some("Popescu Ion".into()),
                description: "P.U.Z parcelare, str. Inexistentă nr. 1".into(),
                item_index: None,
                ..Default::default()
            },
        ]),
    }
}

#[test]
fn write_then_search() {
    let (_dir, db) = open();
    let rep = db.write_meeting(&meeting_0916()).unwrap();
    assert_eq!(rep.items_new, 2);
    assert_eq!(rep.items_total, 2);

    // prefix, fără diacritice, cu majuscule
    let r = db
        .search(&SearchQuery {
            q: "BRANCUS".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);
    assert_eq!(r.items[0].title, "P.U.D construire imobil mixt");
    assert_eq!(r.items[0].beneficiary.as_deref(), Some("Batiment Vert SRL"));
    assert_eq!(r.items[0].documents.len(), 1);
    assert_eq!(r.items[0].meeting_date, NaiveDate::from_ymd_opt(2026, 9, 16).unwrap());

    // diacritice în sens invers: căutăm fără, textul are
    let r = db
        .search(&SearchQuery {
            q: "campului".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);
    assert_eq!(r.items[0].category, Category::AvizOportunitate);

    // beneficiarul e indexat
    let r = db
        .search(&SearchQuery {
            q: "batiment vert".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);

    // fără text: toate, filtrate pe categorie și an
    let r = db.search(&SearchQuery::default()).unwrap();
    assert_eq!(r.total, 2);
    assert!(r.agenda_only.is_empty());
    let r = db
        .search(&SearchQuery {
            categories: vec![Category::Pud],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);
    let r = db
        .search(&SearchQuery {
            year: Some(2025),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 0);

    // rând doar în ordinea de zi
    let r = db
        .search(&SearchQuery {
            q: "inexistenta".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 0);
    assert_eq!(r.agenda_only.len(), 1);
    assert_eq!(r.agenda_only[0].category, Category::Puz);
    assert_eq!(r.agenda_only[0].beneficiary.as_deref(), Some("Popescu Ion"));

    // paginare
    let r = db
        .search(&SearchQuery {
            limit: 1,
            offset: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 2);
    assert_eq!(r.items.len(), 1);
}

#[test]
fn rewrite_is_idempotent_and_keeps_known_values() {
    let (_dir, db) = open();
    db.write_meeting(&meeting_0916()).unwrap();

    // a doua trecere: fără agendă și fără documente (None = păstrează)
    let mut again = meeting_0916();
    again.agenda_text = None;
    again.agenda_rows = None;
    for it in &mut again.items {
        it.documents = None;
        it.beneficiary = None;
    }
    let rep = db.write_meeting(&again).unwrap();
    assert_eq!(rep.items_new, 0);

    let s = db.status().unwrap();
    assert_eq!(s.meetings, 1);
    assert_eq!(s.items, 2);
    assert_eq!(s.agenda_rows_unmatched, 1);

    let r = db
        .search(&SearchQuery {
            q: "brancusi".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        r.items[0].beneficiary.as_deref(),
        Some("Batiment Vert SRL"),
        "beneficiarul cunoscut se păstrează"
    );
    assert_eq!(r.items[0].documents.len(), 1, "documentele se păstrează");

    let meetings = db.list_meetings().unwrap();
    assert_eq!(meetings.len(), 1);
    assert_eq!(meetings[0].item_count, 2);
    let m = db.get_meeting(meetings[0].id).unwrap().unwrap();
    assert_eq!(m.time.as_deref(), Some("10:00"));
    assert_eq!(db.items_for_meeting(m.id).unwrap().len(), 2);
    assert_eq!(db.agenda_rows_for_meeting(m.id).unwrap().len(), 2);
    let streets = db.streets().unwrap();
    assert_eq!(streets.len(), 2);
}

#[test]
fn sync_runs_are_recorded() {
    let (_dir, db) = open();
    assert!(db.status().unwrap().last_run.is_none());
    let id = db.start_sync_run().unwrap();
    db.finish_sync_run(
        id,
        true,
        &RunCounts {
            meetings_seen: 20,
            meetings_updated: 3,
            items_new: 7,
            certificates_new: 2,
        },
        None,
    )
    .unwrap();
    let run = db.status().unwrap().last_run.unwrap();
    assert!(run.ok);
    assert_eq!(run.meetings_seen, 20);
    assert_eq!(run.items_new, 7);
    assert_eq!(run.certificates_new, 2);
    assert!(run.finished_at.is_some());
}

#[allow(clippy::too_many_arguments)]
fn cert(
    url: &str,
    number: i64,
    year: i32,
    (y, m, d): (i32, u32, u32),
    scop: &str,
    kind: CertificateKind,
    address: Option<&str>,
    street: Option<&str>,
    street_no: Option<&str>,
) -> CertificateWrite {
    CertificateWrite {
        url: url.into(),
        number,
        year,
        date: NaiveDate::from_ymd_opt(y, m, d).unwrap(),
        scop: scop.into(),
        kind,
        address: address.map(str::to_owned),
        street: street.map(str::to_owned),
        street_no: street_no.map(str::to_owned),
    }
}

#[test]
fn certificates_write_search_and_alert_window() {
    let (_dir, db) = open();
    let certs = vec![
        cert(
            "https://x/cu-1733",
            1733,
            2026,
            (2026, 10, 1),
            "INFORMARE",
            CertificateKind::Informare,
            Some("CĂPITAN GRIGORE IGNAT, nr. 28"),
            Some("CĂPITAN GRIGORE IGNAT"),
            Some("28"),
        ),
        cert(
            "https://x/cu-1731",
            1731,
            2026,
            (2026, 9, 30),
            "ELABORARE PLAN URBANISTIC ZONAL",
            CertificateKind::Puz,
            Some("Str Traian Vuia, nr. 246"),
            Some("Traian Vuia"),
            Some("246"),
        ),
        cert(
            "https://x/cu-12",
            12,
            2024,
            (2024, 1, 5),
            "OPERAȚIUNI CADASTRALE",
            CertificateKind::Operatiuni,
            None,
            None,
            None,
        ),
    ];
    assert_eq!(db.write_certificates(&certs).unwrap(), 3);
    assert_eq!(
        db.write_certificates(&certs).unwrap(),
        0,
        "a doua scriere nu aduce nimic nou"
    );
    assert_eq!(db.status().unwrap().certificates, 3);

    // fără text: toate, cele mai recente întâi
    let r = db.search_certificates(&CertificateQuery::default()).unwrap();
    assert_eq!(r.total, 3);
    assert_eq!(r.items[0].number, 1733);
    assert_eq!(r.items[0].title(), "Certificat de urbanism 1733/2026");
    assert_eq!(r.items[0].street_no.as_deref(), Some("28"));
    assert_eq!(r.items[2].kind, CertificateKind::Operatiuni);

    // prefix, fără diacritice: „capitan” găsește „CĂPITAN”
    let r = db
        .search_certificates(&CertificateQuery {
            q: "capitan ignat".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);

    // numărul certificatului e indexat
    let r = db
        .search_certificates(&CertificateQuery {
            q: "1731".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);
    assert_eq!(r.items[0].kind, CertificateKind::Puz);

    // filtre pe tip și an, paginare
    let r = db
        .search_certificates(&CertificateQuery {
            kinds: vec![CertificateKind::Operatiuni],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);
    let r = db
        .search_certificates(&CertificateQuery {
            year: Some(2026),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 2);
    let r = db
        .search_certificates(&CertificateQuery {
            limit: 1,
            offset: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 3);
    assert_eq!(r.items.len(), 1);
    assert_eq!(r.items[0].number, 1731);

    // alerte: văzut după `since` și emis cu cel mult 14 zile înaintea lui `since`
    let fts = fts_query("traian vuia").unwrap();
    assert_eq!(
        db.new_certificates_matching(&fts, "2026-09-25T00:00:00Z")
            .unwrap()
            .len(),
        1
    );
    assert!(
        db.new_certificates_matching(&fts, "2999-01-01T00:00:00Z")
            .unwrap()
            .is_empty()
    );
    // certificat vechi descărcat acum (istoric): în afara ferestrei, nu alertează
    let fts = fts_query("cadastrale").unwrap();
    assert!(
        db.new_certificates_matching(&fts, "2026-09-25T00:00:00Z")
            .unwrap()
            .is_empty()
    );

    // starea internă cu chei
    assert_eq!(db.meta_get("certificates_backfill_year").unwrap(), None);
    db.meta_set("certificates_backfill_year", "2024").unwrap();
    db.meta_set("certificates_backfill_year", "2023").unwrap();
    assert_eq!(
        db.meta_get("certificates_backfill_year").unwrap().as_deref(),
        Some("2023")
    );
}

#[test]
fn certificate_details_are_fetched_once_and_searchable() {
    let (_dir, db) = open();
    let certs = vec![
        cert(
            "https://x/cu-1733",
            1733,
            2026,
            (2026, 10, 1),
            "INFORMARE",
            CertificateKind::Informare,
            None,
            None,
            None,
        ),
        cert(
            "https://x/cu-1685",
            1685,
            2026,
            (2026, 9, 14),
            "ELABORARE PLAN URBANISTIC ZONAL",
            CertificateKind::Puz,
            Some("Str Doinaș, nr. FN"),
            Some("Doinaș"),
            None,
        ),
        cert(
            "https://x/cu-1647",
            1647,
            2026,
            (2026, 9, 3),
            "ELABORARE PLAN URBANISTIC DE DETALIU",
            CertificateKind::Pud,
            Some("Str Decebal, nr. 96"),
            Some("Decebal"),
            Some("96"),
        ),
    ];
    db.write_certificates(&certs).unwrap();

    // doar PUZ și PUD, cele mai recente întâi, cu plafon
    let kinds = [CertificateKind::Puz, CertificateKind::Pud];
    let pending = db.certificates_needing_details(&kinds, 10).unwrap();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[0].1, "https://x/cu-1685");
    assert_eq!(db.certificates_needing_details(&kinds, 1).unwrap().len(), 1);
    assert!(db.certificates_needing_details(&[], 10).unwrap().is_empty());

    db.write_certificate_details(
        pending[0].0,
        &CertificateDetailWrite {
            surface_mp: Some(26677),
            utr: Some("LC, ULC, UIs".into()),
            land_use: Some("terenuri: arabil, livada, drum".into()),
            cf: None,
            cadastral: None,
        },
    )
    .unwrap();
    // o pagină fără câmpuri se marchează totuși descărcată
    db.write_certificate_details(pending[1].0, &CertificateDetailWrite::default())
        .unwrap();
    assert!(db.certificates_needing_details(&kinds, 10).unwrap().is_empty());

    // câmpurile se citesc și sunt indexate: căutăm după UTR și după folosință
    let r = db
        .search_certificates(&CertificateQuery {
            q: "uis".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);
    assert_eq!(r.items[0].number, 1685);
    assert_eq!(r.items[0].surface_mp, Some(26677));
    assert_eq!(r.items[0].utr.as_deref(), Some("LC, ULC, UIs"));
    let r = db
        .search_certificates(&CertificateQuery {
            q: "livada".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.total, 1);

    // re-citirea listei nu șterge detaliile
    db.write_certificates(&certs).unwrap();
    let r = db
        .search_certificates(&CertificateQuery {
            q: "1685".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.items[0].land_use.as_deref(), Some("terenuri: arabil, livada, drum"));
    assert!(db.certificates_needing_details(&kinds, 10).unwrap().is_empty());
}
