use chrono::NaiveDate;
use urban_core::db::{AgendaRowWrite, Db, ItemWrite, MeetingWrite, RunCounts};
use urban_core::shared::{Category, Document, SearchQuery};

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
        },
        None,
    )
    .unwrap();
    let run = db.status().unwrap().last_run.unwrap();
    assert!(run.ok);
    assert_eq!(run.meetings_seen, 20);
    assert_eq!(run.items_new, 7);
    assert!(run.finished_at.is_some());
}
