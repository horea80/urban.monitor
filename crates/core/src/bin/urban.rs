//! CLI (FR-6): `urban sync [--full]`, `urban search <text> [--tip ...] [--an ...]`, `urban meetings`, `urban status`.
//! Serverul web este crate-ul `urban-app`.

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use urban_core::db::Db;
use urban_core::pdf::Chain;
use urban_core::scrape::Client;
use urban_core::shared::{Category, SearchQuery};
use urban_core::sync::{self, SyncOptions};
use urban_core::Config;

#[derive(Parser)]
#[command(name = "urban", version, about = "Ședințele CTATU Cluj-Napoca: sincronizare și căutare după stradă")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Descarcă ședințele noi sau recente de pe site-ul primăriei
    Sync {
        /// Re-descarcă tot, indiferent de ce există în bază
        #[arg(long)]
        full: bool,
    },
    /// Caută proiecte după stradă sau text liber
    Search {
        /// Cuvinte de căutat; fără diacritice merge la fel
        q: Vec<String>,
        /// Categorii: AVIZ_OPORTUNITATE, PUZ, PUD, ALTELE (sau aviz/puz/pud), separate prin virgulă
        #[arg(long = "tip", value_delimiter = ',')]
        tip: Vec<Category>,
        /// Doar ședințele din anul dat
        #[arg(long = "an")]
        an: Option<i32>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Ieșire JSON în loc de tabel
        #[arg(long)]
        json: bool,
    },
    /// Ședințele din bază, cele mai noi întâi
    Meetings,
    /// Contoare și ultima sincronizare
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let cfg = Config::from_env();
    let db = Db::open(&cfg.db_path())?;

    match cli.cmd {
        Cmd::Sync { full } => {
            let client = Client::new(&cfg)?;
            let pdf = Chain::default_chain();
            let report = sync::run(&cfg, &client, &db, &pdf, SyncOptions { full }).await?;
            println!(
                "ședințe în listă: {}, actualizate: {}, proiecte noi: {}, avertismente: {}, erori: {}",
                report.meetings_seen,
                report.meetings_updated,
                report.items_new,
                report.warnings.len(),
                report.errors.len()
            );
            for w in &report.warnings {
                println!("  avertisment: {w}");
            }
            for e in &report.errors {
                println!("  eroare: {e}");
            }
            if !report.errors.is_empty() {
                std::process::exit(1);
            }
        }
        Cmd::Search { q, tip, an, limit, json } => {
            let query = SearchQuery { q: q.join(" "), categories: tip, year: an, limit, offset: 0 };
            let result = db.search(&query)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
                return Ok(());
            }
            println!("{} proiecte găsite", result.total);
            for it in &result.items {
                println!(
                    "{}  {:<24}  {}\n{:>12}  {}{}\n{:>12}  {}",
                    it.meeting_date,
                    it.category.label(),
                    it.title,
                    "",
                    it.address.as_deref().unwrap_or("-"),
                    it.beneficiary.as_deref().map(|b| format!("  ·  {b}")).unwrap_or_default(),
                    "",
                    it.url
                );
            }
            if !result.agenda_only.is_empty() {
                println!("\n{} rânduri doar în ordinea de zi, fără proiect publicat:", result.agenda_only.len());
                for r in &result.agenda_only {
                    println!(
                        "{}  {:<24}  {}{}",
                        r.meeting_date,
                        r.category.label(),
                        r.description,
                        r.beneficiary.as_deref().map(|b| format!("  ·  {b}")).unwrap_or_default()
                    );
                }
            }
        }
        Cmd::Meetings => {
            for m in db.list_meetings()? {
                println!(
                    "{}  {:>5}  {:>3} proiecte  {}  {}",
                    m.date,
                    m.time.as_deref().unwrap_or("-"),
                    m.item_count,
                    if m.conclusions_pdf_url.is_some() { "concluzii" } else { "         " },
                    m.url
                );
            }
        }
        Cmd::Status => {
            let s = db.status()?;
            println!("ședințe: {}  proiecte: {}  rânduri de agendă nepotrivite: {}", s.meetings, s.items, s.agenda_rows_unmatched);
            match s.last_run {
                Some(r) => println!(
                    "ultima sincronizare: început {}  sfârșit {}  ok={}  ședințe {}/{}  proiecte noi {}{}",
                    r.started_at,
                    r.finished_at.as_deref().unwrap_or("în curs"),
                    r.ok,
                    r.meetings_updated,
                    r.meetings_seen,
                    r.items_new,
                    r.error.as_deref().map(|e| format!("\n  eroare: {e}")).unwrap_or_default()
                ),
                None => println!("nicio sincronizare încă; rulează `urban sync`"),
            }
        }
    }
    Ok(())
}
