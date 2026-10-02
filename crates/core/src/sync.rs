//! Sincronizare incrementală (FR-1.8, FR-1.9): lista → ședințele selectate → o scriere per ședință.
//! Toate descărcările unei ședințe se fac înainte de tranzacție; erorile per ședință nu opresc restul.
//! După ședințe urmează certificatele de urbanism (FR-10), pagină cu pagină, o scriere per pagină.

use chrono::{Datelike, Local};
use tracing::{error, info, warn};

use crate::agenda::{AgendaRow, match_rows, parse_agenda};
use crate::config::Config;
use crate::db::{
    AgendaRowWrite, CertificateDetailWrite, CertificateWrite, Db, ItemWrite, MeetingSummary, MeetingWrite, RunCounts,
    WriteReport,
};
use crate::error::{Error, Result};
use crate::pdf::PdfText;
use crate::scrape::{
    Card, CertificateDetail, CertificateRef, Client, MeetingRef, certificates_page_url, first_pdf,
    parse_certificate_detail, parse_certificates, parse_documents, parse_listing, parse_meeting,
};
use urban_shared::CertificateKind;
use urban_shared::text::{classify, classify_scop, parse_work_address, street_of};

/// Cheia din `meta`: anul până la care istoricul certificatelor a fost parcurs complet.
const CERT_BACKFILL_KEY: &str = "certificates_backfill_year";

/// Plasă de siguranță pentru parcurgerea istoricului: 2024–2026 înseamnă ~310 pagini.
const MAX_CERT_PAGES: u32 = 2000;

/// Tipurile pentru care se descarcă și pagina certificatului (FR-10.6, ADR-0014): cele care anunță o
/// documentație de urbanism, ~15% din certificate.
const DETAIL_KINDS: [CertificateKind; 2] = [CertificateKind::Puz, CertificateKind::Pud];

/// Pagini de certificat per rulare: ~2 s fiecare, deci cel mult ~10 minute; restul la rularea următoare.
const MAX_DETAILS_PER_RUN: usize = 300;

#[derive(Debug, Clone, Copy, Default)]
pub struct SyncOptions {
    /// Re-descarcă tot, indiferent de ce există deja.
    pub full: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SyncReport {
    pub meetings_seen: usize,
    pub meetings_updated: usize,
    pub items_new: usize,
    /// Certificate de urbanism citite din paginile parcurse (FR-10).
    pub certificates_seen: usize,
    pub certificates_new: usize,
    /// Pagini de certificat descărcate pentru PUZ și PUD (FR-10.6).
    pub certificate_details: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl SyncReport {
    fn counts(&self) -> RunCounts {
        RunCounts {
            meetings_seen: self.meetings_seen,
            meetings_updated: self.meetings_updated,
            items_new: self.items_new,
            certificates_new: self.certificates_new,
        }
    }
}

/// Rulează o sincronizare completă și o înregistrează în `sync_runs`.
pub async fn run(cfg: &Config, client: &Client, db: &Db, pdf: &dyn PdfText, opts: SyncOptions) -> Result<SyncReport> {
    let run_id = db.start_sync_run()?;
    let result = run_inner(cfg, client, db, pdf, opts).await;
    match &result {
        Ok(rep) => {
            let error = if rep.errors.is_empty() {
                None
            } else {
                Some(rep.errors.join("\n"))
            };
            db.finish_sync_run(run_id, rep.errors.is_empty(), &rep.counts(), error.as_deref())?;
        }
        Err(e) => db.finish_sync_run(run_id, false, &RunCounts::default(), Some(&e.to_string()))?,
    }
    result
}

async fn run_inner(cfg: &Config, client: &Client, db: &Db, pdf: &dyn PdfText, opts: SyncOptions) -> Result<SyncReport> {
    let html = client.get_html(&cfg.listing_url).await?;
    let listing = parse_listing(&html, &cfg.listing_url);
    if listing.is_empty() {
        return Err(Error::parse(
            &cfg.listing_url,
            "nicio ședință găsită în listă; s-a schimbat structura paginii?",
        ));
    }

    let today = Local::now().date_naive();
    let mut selected: Vec<MeetingRef> = listing
        .into_iter()
        .filter(|m| m.date.is_some_and(|d| d.year() >= cfg.start_year))
        .collect();
    selected.sort_by_key(|m| m.date);

    let mut report = SyncReport {
        meetings_seen: selected.len(),
        ..Default::default()
    };
    info!(total = selected.len(), start_year = cfg.start_year, "ședințe în listă");

    for mref in &selected {
        let Some(date) = mref.date else { continue };
        let existing = db.meeting_summary(&mref.url)?;
        let recent = (today - date).num_days() <= cfg.refresh_days;
        if !(opts.full || existing.is_none() || recent) {
            continue;
        }
        match process_meeting(client, db, pdf, opts, mref, existing.as_ref()).await {
            Ok((rep, warnings)) => {
                report.meetings_updated += 1;
                report.items_new += rep.items_new;
                info!(url = %mref.url, items = rep.items_total, new = rep.items_new, "ședință sincronizată");
                for w in &warnings {
                    warn!("{w}");
                }
                report.warnings.extend(warnings);
            }
            Err(e) => {
                error!(url = %mref.url, error = %e, "ședința a eșuat");
                report.errors.push(format!("{}: {e}", mref.url));
            }
        }
    }

    // certificatele de urbanism (FR-10): o eroare aici nu anulează ce s-a scris pentru ședințe
    if let Err(e) = sync_certificates(cfg, client, db, opts, &mut report).await {
        error!(error = %e, "certificatele de urbanism au eșuat");
        report.errors.push(format!("certificate de urbanism: {e}"));
    }
    // paginile certificatelor PUZ și PUD (FR-10.6), pas separat și reluabil
    if let Err(e) = sync_certificate_details(client, db, &mut report).await {
        error!(error = %e, "paginile certificatelor au eșuat");
        report.errors.push(format!("pagini de certificat: {e}"));
    }
    Ok(report)
}

/// FR-10.6: pentru certificatele PUZ și PUD fără pagina descărcată, cele mai recente întâi, ia
/// suprafața, UTR-urile, folosința actuală, CF și cadastralul. O pagină fără câmpurile așteptate se
/// marchează totuși descărcată, ca să nu se reîncerce la nesfârșit; o descărcare eșuată se reia.
async fn sync_certificate_details(client: &Client, db: &Db, report: &mut SyncReport) -> Result<()> {
    let pending = db.certificates_needing_details(&DETAIL_KINDS, MAX_DETAILS_PER_RUN)?;
    if pending.is_empty() {
        return Ok(());
    }
    info!(n = pending.len(), "certificate: descarc paginile PUZ și PUD");
    for (id, url) in pending {
        match client.get_html(&url).await {
            Ok(html) => {
                let d = parse_certificate_detail(&html);
                if d == CertificateDetail::default() {
                    report
                        .warnings
                        .push(format!("{url}: pagina certificatului nu are câmpurile așteptate"));
                }
                db.write_certificate_details(
                    id,
                    &CertificateDetailWrite {
                        surface_mp: d.surface_mp,
                        utr: (!d.utr.is_empty()).then(|| d.utr.join(", ")),
                        land_use: d.land_use,
                        cf: d.cf,
                        cadastral: d.cadastral,
                    },
                )?;
                report.certificate_details += 1;
            }
            Err(e) => report.warnings.push(format!("{url}: pagina certificatului: {e}")),
        }
    }
    Ok(())
}

/// FR-10.1: lista certificatelor, pagină cu pagină, cele mai noi întâi. La prima rulare, după `full`
/// sau după coborârea anului de start parcurge tot istoricul până la `cert_start_year` și notează
/// asta în `meta`; altfel se oprește la prima pagină fără certificate noi, de obicei a doua.
async fn sync_certificates(
    cfg: &Config,
    client: &Client,
    db: &Db,
    opts: SyncOptions,
    report: &mut SyncReport,
) -> Result<()> {
    let start_year = cfg.cert_start_year;
    let backfilled = db
        .meta_get(CERT_BACKFILL_KEY)?
        .and_then(|v| v.parse::<i32>().ok())
        .is_some_and(|y| y <= start_year);
    let walk_all = opts.full || !backfilled;
    if walk_all {
        info!(start_year, "certificate de urbanism: parcurg tot istoricul");
    }

    let mut page = 1u32;
    loop {
        let url = certificates_page_url(&cfg.cert_listing_url, page);
        let html = client.get_html(&url).await?;
        let parsed = parse_certificates(&html, &url);
        if parsed.certificates.is_empty() {
            if page == 1 {
                return Err(Error::parse(
                    &url,
                    "niciun certificat găsit în listă; s-a schimbat structura paginii?",
                ));
            }
            report
                .warnings
                .push(format!("{url}: pagină fără certificate; mă opresc aici"));
            break;
        }
        let reached_start = parsed.certificates.iter().any(|c| c.date.year() < start_year);
        let writes: Vec<CertificateWrite> = parsed
            .certificates
            .iter()
            .filter(|c| c.date.year() >= start_year)
            .map(certificate_write)
            .collect();
        let new = db.write_certificates(&writes)?;
        report.certificates_seen += writes.len();
        report.certificates_new += new;
        info!(page, seen = writes.len(), new, "pagină de certificate sincronizată");

        if reached_start || !parsed.has_next || (!walk_all && new == 0) {
            break;
        }
        page += 1;
        if page > MAX_CERT_PAGES {
            report
                .warnings
                .push(format!("certificate: m-am oprit după {MAX_CERT_PAGES} pagini"));
            break;
        }
    }
    if walk_all {
        db.meta_set(CERT_BACKFILL_KEY, &start_year.to_string())?;
    }
    Ok(())
}

fn certificate_write(c: &CertificateRef) -> CertificateWrite {
    let addr = c.address.as_deref().map(parse_work_address).unwrap_or_default();
    CertificateWrite {
        url: c.url.clone(),
        number: c.number,
        year: c.year,
        date: c.date,
        scop: c.scop.clone(),
        kind: classify_scop(&c.scop),
        address: addr.address,
        street: addr.street,
        street_no: addr.street_no,
    }
}

async fn process_meeting(
    client: &Client,
    db: &Db,
    pdf: &dyn PdfText,
    opts: SyncOptions,
    mref: &MeetingRef,
    existing: Option<&MeetingSummary>,
) -> Result<(WriteReport, Vec<String>)> {
    let mut warnings = Vec::new();

    let html = client.get_html(&mref.url).await?;
    let page = parse_meeting(&html, &mref.url);
    let date = page
        .date
        .or(mref.date)
        .ok_or_else(|| Error::parse(&mref.url, "nu am putut determina data ședinței"))?;
    let projects: Vec<Card> = page.projects().cloned().collect();
    if projects.is_empty() {
        warnings.push(format!(
            "{}: 0 proiecte pe pagina ședinței; s-a schimbat structura?",
            mref.url
        ));
    }

    // ordinea de zi (FR-1.3)
    let mut agenda_text: Option<String> = None;
    let mut agenda_rows: Option<Vec<AgendaRow>> = None;
    if let Some(agenda_url) = &page.agenda_url {
        let have = existing.is_some_and(|e| e.has_agenda_text && e.agenda_url.as_deref() == Some(agenda_url.as_str()));
        if opts.full || !have {
            match client.get_pdf(agenda_url).await {
                Ok(bytes) => match pdf.extract(&bytes) {
                    Ok(text) => {
                        let rows = parse_agenda(&text);
                        if rows.is_empty() {
                            warnings.push(format!("{agenda_url}: niciun rând recunoscut în ordinea de zi"));
                        }
                        agenda_rows = Some(rows);
                        agenda_text = Some(text);
                    }
                    Err(e) => warnings.push(format!("{agenda_url}: text PDF indisponibil: {e}")),
                },
                Err(e) => warnings.push(format!("{agenda_url}: descărcare eșuată: {e}")),
            }
        } else if let Some(text) = existing.map(|e| db.agenda_text(e.id)).transpose()?.flatten() {
            // ordinea de zi apare adesea înaintea proiectelor: repotrivim rândurile cu cardurile de acum
            agenda_rows = Some(parse_agenda(&text));
        }
    } else {
        warnings.push(format!("{}: fără link către ordinea de zi", mref.url));
    }

    // concluziile ședinței (FR-1.7)
    let mut conclusions_pdf_url: Option<String> = None;
    if let Some(card) = page.conclusions() {
        let known = existing.and_then(|e| e.conclusions_pdf_url.clone());
        if opts.full || known.is_none() {
            match client.get_html(&card.url).await {
                Ok(h) => {
                    let docs = parse_documents(&h, &card.url);
                    conclusions_pdf_url = first_pdf(&docs).map(str::to_owned);
                    if conclusions_pdf_url.is_none() {
                        warnings.push(format!("{}: pagina concluziilor nu are PDF", card.url));
                    }
                }
                Err(e) => warnings.push(format!("{}: pagina concluziilor: {e}", card.url)),
            }
        }
    }
    let announcement_url = page.announcement().map(|c| c.url.clone());

    // proiectele și documentele lor (FR-1.2, FR-1.6)
    let urls: Vec<String> = projects.iter().map(|c| c.url.clone()).collect();
    let known_items = db.item_summaries(&urls)?;
    let mut items: Vec<ItemWrite> = Vec::with_capacity(projects.len());
    for card in &projects {
        let need_docs = opts.full || known_items.get(&card.url).is_none_or(|s| !s.docs_fetched);
        let documents = if need_docs {
            match client.get_html(&card.url).await {
                Ok(h) => Some(parse_documents(&h, &card.url)),
                Err(e) => {
                    warnings.push(format!("{}: pagina proiectului: {e}", card.url));
                    None
                }
            }
        } else {
            None
        };
        items.push(ItemWrite {
            url: card.url.clone(),
            title: card.title.clone(),
            address: card.address.clone(),
            street: card.address.as_deref().and_then(street_of),
            category: classify(&card.title),
            published_at: card.published_at.clone(),
            documents,
            ..Default::default()
        });
    }

    // potrivirea rândurilor cu proiectele (FR-1.4, FR-1.5)
    let rows_write = agenda_rows.map(|rows| {
        let matched = match_rows(&rows, &projects);
        let unmatched = matched.iter().filter(|m| m.is_none()).count();
        if unmatched > 0 {
            warnings.push(format!(
                "{}: {unmatched} rânduri din ordinea de zi fără proiect publicat",
                mref.url
            ));
        }
        rows.into_iter()
            .zip(matched)
            .map(|(r, idx)| {
                if let Some(i) = idx {
                    let it = &mut items[i];
                    it.beneficiary = r.beneficiary.clone();
                    it.reg_number = r.reg_number.clone();
                    it.reg_date = r.reg_date.clone();
                    it.revenire = r.revenire;
                    it.agenda_description = Some(r.description.clone());
                }
                AgendaRowWrite {
                    nr: r.nr,
                    reg_number: r.reg_number,
                    reg_date: r.reg_date,
                    beneficiary: r.beneficiary,
                    description: r.description,
                    revenire: r.revenire,
                    item_index: idx,
                }
            })
            .collect::<Vec<_>>()
    });

    let mw = MeetingWrite {
        url: mref.url.clone(),
        title: if page.title.is_empty() {
            mref.title.clone()
        } else {
            page.title.clone()
        },
        date,
        time: page.time.clone(),
        agenda_url: page.agenda_url.clone(),
        agenda_text,
        conclusions_pdf_url,
        announcement_url,
        items,
        agenda_rows: rows_write,
    };
    let report = db.write_meeting(&mw)?;
    Ok((report, warnings))
}
