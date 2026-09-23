//! SQLite: schemă versionată, scriere per ședință într-o tranzacție, căutare FTS5 (ADR-0003).
//! API sincron; din cod async se apelează prin `spawn_blocking`.

use std::collections::HashMap;
use std::path::Path;

use chrono::{Local, NaiveDate, SecondsFormat, Utc};
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params, params_from_iter};
use rusqlite_migration::{M, Migrations};

use crate::error::Result;
use urban_shared::text::{classify, normalize, tokens};
use urban_shared::{AgendaRowView, Category, Document, Item, Meeting, SearchQuery, SearchResult, Status, SyncRun};

const SCHEMA_V1: &str = r#"
CREATE TABLE meetings (
  id                  INTEGER PRIMARY KEY,
  url                 TEXT NOT NULL UNIQUE,
  title               TEXT NOT NULL,
  date                TEXT NOT NULL,
  time                TEXT,
  agenda_url          TEXT,
  agenda_text         TEXT,
  conclusions_pdf_url TEXT,
  announcement_url    TEXT,
  first_seen_at       TEXT NOT NULL,
  last_seen_at        TEXT NOT NULL
);
CREATE INDEX meetings_date ON meetings(date);

CREATE TABLE items (
  id                 INTEGER PRIMARY KEY,
  meeting_id         INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  url                TEXT NOT NULL UNIQUE,
  title              TEXT NOT NULL,
  address            TEXT,
  street             TEXT,
  category           TEXT NOT NULL,
  published_at       TEXT,
  beneficiary        TEXT,
  reg_number         TEXT,
  reg_date           TEXT,
  revenire           INTEGER NOT NULL DEFAULT 0,
  agenda_description TEXT,
  docs_fetched       INTEGER NOT NULL DEFAULT 0,
  first_seen_at      TEXT NOT NULL,
  last_seen_at       TEXT NOT NULL
);
CREATE INDEX items_meeting ON items(meeting_id);
CREATE INDEX items_category ON items(category);

CREATE TABLE documents (
  id      INTEGER PRIMARY KEY,
  item_id INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  label   TEXT NOT NULL,
  url     TEXT NOT NULL,
  UNIQUE(item_id, url)
);

CREATE TABLE agenda_rows (
  id          INTEGER PRIMARY KEY,
  meeting_id  INTEGER NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  nr          INTEGER,
  reg_number  TEXT,
  reg_date    TEXT,
  beneficiary TEXT,
  description TEXT NOT NULL,
  category    TEXT NOT NULL,
  revenire    INTEGER NOT NULL DEFAULT 0,
  item_id     INTEGER REFERENCES items(id) ON DELETE SET NULL
);
CREATE INDEX agenda_rows_meeting ON agenda_rows(meeting_id);

-- text deja normalizat în Rust (fără diacritice, litere mici); tokenizer-ul doar desparte pe spații
CREATE VIRTUAL TABLE items_fts  USING fts5(text, item_id UNINDEXED, tokenize='unicode61');
CREATE VIRTUAL TABLE agenda_fts USING fts5(text, row_id  UNINDEXED, tokenize='unicode61');

CREATE TABLE sync_runs (
  id               INTEGER PRIMARY KEY,
  started_at       TEXT NOT NULL,
  finished_at      TEXT,
  ok               INTEGER NOT NULL DEFAULT 0,
  meetings_seen    INTEGER NOT NULL DEFAULT 0,
  meetings_updated INTEGER NOT NULL DEFAULT 0,
  items_new        INTEGER NOT NULL DEFAULT 0,
  error            TEXT
);
"#;

// FR-8: când a plecat alerta pentru ședință (NULL = încă nu)
const SCHEMA_V2: &str = "ALTER TABLE meetings ADD COLUMN alerted_at TEXT;";

// FR-9: conturi, sesiuni (doar hash-uri), linkuri de autentificare, cuvinte-cheie, credite (ADR-0012)
const SCHEMA_V3: &str = r#"
CREATE TABLE users (
  id                   INTEGER PRIMARY KEY,
  email                TEXT NOT NULL UNIQUE,
  plan                 TEXT NOT NULL DEFAULT 'free',
  credits_available    INTEGER NOT NULL DEFAULT 0,
  credits_used         INTEGER NOT NULL DEFAULT 0,
  cycle_start_at       TEXT NOT NULL,
  cycle_end_at         TEXT NOT NULL,
  alerts_checked_until TEXT NOT NULL,
  consent_at           TEXT NOT NULL,
  created_at           TEXT NOT NULL,
  last_login_at        TEXT
);
CREATE TABLE sessions (
  token_hash  TEXT PRIMARY KEY,
  user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  expires_at  TEXT NOT NULL,
  created_at  TEXT NOT NULL
);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE TABLE magic_links (
  id          INTEGER PRIMARY KEY,
  email       TEXT NOT NULL,
  token_hash  TEXT NOT NULL UNIQUE,
  consent     INTEGER NOT NULL DEFAULT 0,
  expires_at  TEXT NOT NULL,
  used_at     TEXT,
  created_at  TEXT NOT NULL
);
CREATE INDEX magic_links_email ON magic_links(email, created_at);
CREATE TABLE keywords (
  id          INTEGER PRIMARY KEY,
  user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  text        TEXT NOT NULL,
  normalized  TEXT NOT NULL,
  created_at  TEXT NOT NULL,
  UNIQUE(user_id, normalized)
);
CREATE TABLE credit_ledger (
  id              INTEGER PRIMARY KEY,
  user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  direction       TEXT NOT NULL CHECK (direction IN ('credit','debit')),
  amount          INTEGER NOT NULL CHECK (amount >= 0),
  reason          TEXT NOT NULL,
  idempotency_key TEXT UNIQUE,
  balance_after   INTEGER NOT NULL,
  created_at      TEXT NOT NULL
);
CREATE INDEX credit_ledger_user ON credit_ledger(user_id);
CREATE TABLE alert_log (
  id           INTEGER PRIMARY KEY,
  user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  kind         TEXT NOT NULL CHECK (kind IN ('digest','teaser')),
  items        INTEGER NOT NULL,
  window_until TEXT NOT NULL,
  sent_at      TEXT NOT NULL
);
"#;

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(SCHEMA_V1), M::up(SCHEMA_V2), M::up(SCHEMA_V3)])
}

pub type Conn = PooledConnection<SqliteConnectionManager>;

#[derive(Clone)]
pub struct Db {
    pool: Pool<SqliteConnectionManager>,
}

/// Ce scriem pentru un proiect. `None` la câmpurile opționale înseamnă „păstrează ce există”.
#[derive(Debug, Clone, Default)]
pub struct ItemWrite {
    pub url: String,
    pub title: String,
    pub address: Option<String>,
    pub street: Option<String>,
    pub category: Category,
    pub published_at: Option<String>,
    pub beneficiary: Option<String>,
    pub reg_number: Option<String>,
    pub reg_date: Option<String>,
    pub revenire: bool,
    pub agenda_description: Option<String>,
    /// `None` = nu atingem documentele existente.
    pub documents: Option<Vec<Document>>,
}

#[derive(Debug, Clone, Default)]
pub struct AgendaRowWrite {
    pub nr: Option<u32>,
    pub reg_number: Option<String>,
    pub reg_date: Option<String>,
    pub beneficiary: Option<String>,
    pub description: String,
    pub revenire: bool,
    /// Indexul în `MeetingWrite::items` al proiectului potrivit.
    pub item_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct MeetingWrite {
    pub url: String,
    pub title: String,
    pub date: NaiveDate,
    pub time: Option<String>,
    pub agenda_url: Option<String>,
    /// `None` = păstrăm textul existent.
    pub agenda_text: Option<String>,
    pub conclusions_pdf_url: Option<String>,
    pub announcement_url: Option<String>,
    pub items: Vec<ItemWrite>,
    /// `None` = păstrăm rândurile existente.
    pub agenda_rows: Option<Vec<AgendaRowWrite>>,
}

#[derive(Debug, Clone, Default)]
pub struct WriteReport {
    pub meeting_id: i64,
    pub items_new: usize,
    pub items_total: usize,
}

/// Ce știe sync-ul despre o ședință deja stocată.
#[derive(Debug, Clone)]
pub struct MeetingSummary {
    pub id: i64,
    pub date: NaiveDate,
    pub agenda_url: Option<String>,
    pub has_agenda_text: bool,
    pub conclusions_pdf_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ItemSummary {
    pub id: i64,
    pub docs_fetched: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RunCounts {
    pub meetings_seen: usize,
    pub meetings_updated: usize,
    pub items_new: usize,
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn parse_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap_or_default()
}

/// Text indexabil: câmpurile ne-goale, normalizate, separate de spațiu.
fn fts_text<'a>(parts: impl IntoIterator<Item = Option<&'a str>>) -> String {
    normalize(&parts.into_iter().flatten().collect::<Vec<_>>().join(" "))
}

/// Interogare FTS5 din text liber: fiecare cuvânt normalizat, potrivit pe prefix, legate cu AND.
pub fn fts_query(q: &str) -> Option<String> {
    let toks = tokens(q);
    if toks.is_empty() {
        None
    } else {
        Some(toks.iter().map(|t| format!("\"{t}\"*")).collect::<Vec<_>>().join(" "))
    }
}

const ITEM_SELECT: &str = "SELECT i.id, i.meeting_id, m.date, m.url, i.url, i.title, i.address, i.street, i.category, \
     i.published_at, i.beneficiary, i.reg_number, i.reg_date, i.revenire \
     FROM items i JOIN meetings m ON m.id = i.meeting_id";

fn row_to_item(r: &Row<'_>) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        meeting_id: r.get(1)?,
        meeting_date: parse_date(&r.get::<_, String>(2)?),
        meeting_url: r.get(3)?,
        url: r.get(4)?,
        title: r.get(5)?,
        address: r.get(6)?,
        street: r.get(7)?,
        category: r.get::<_, String>(8)?.parse().unwrap_or_default(),
        published_at: r.get(9)?,
        beneficiary: r.get(10)?,
        reg_number: r.get(11)?,
        reg_date: r.get(12)?,
        revenire: r.get::<_, i64>(13)? != 0,
        documents: Vec::new(),
    })
}

const MEETING_SELECT: &str = "SELECT m.id, m.url, m.title, m.date, m.time, m.agenda_url, m.conclusions_pdf_url, m.announcement_url, \
     (SELECT COUNT(*) FROM items i WHERE i.meeting_id = m.id) \
     FROM meetings m";

fn row_to_meeting(r: &Row<'_>) -> rusqlite::Result<Meeting> {
    let date = parse_date(&r.get::<_, String>(3)?);
    Ok(Meeting {
        id: r.get(0)?,
        url: r.get(1)?,
        title: r.get(2)?,
        date,
        time: r.get(4)?,
        agenda_url: r.get(5)?,
        conclusions_pdf_url: r.get(6)?,
        announcement_url: r.get(7)?,
        item_count: r.get(8)?,
        // decis pe server, nu în browser, ca SSR-ul și hidratarea să vadă aceeași valoare
        upcoming: date > Local::now().date_naive(),
    })
}

const AGENDA_SELECT: &str = "SELECT ar.id, ar.meeting_id, m.date, m.url, ar.item_id, ar.nr, ar.reg_number, ar.reg_date, \
     ar.beneficiary, ar.description, ar.revenire, ar.category \
     FROM agenda_rows ar JOIN meetings m ON m.id = ar.meeting_id";

fn row_to_agenda(r: &Row<'_>) -> rusqlite::Result<AgendaRowView> {
    Ok(AgendaRowView {
        id: r.get(0)?,
        meeting_id: r.get(1)?,
        meeting_date: parse_date(&r.get::<_, String>(2)?),
        meeting_url: r.get(3)?,
        item_id: r.get(4)?,
        nr: r.get(5)?,
        reg_number: r.get(6)?,
        reg_date: r.get(7)?,
        beneficiary: r.get(8)?,
        description: r.get(9)?,
        revenire: r.get::<_, i64>(10)? != 0,
        category: r.get::<_, String>(11)?.parse().unwrap_or_default(),
    })
}

fn row_to_sync_run(r: &Row<'_>) -> rusqlite::Result<SyncRun> {
    Ok(SyncRun {
        id: r.get(0)?,
        started_at: r.get(1)?,
        finished_at: r.get(2)?,
        ok: r.get::<_, i64>(3)? != 0,
        meetings_seen: r.get(4)?,
        meetings_updated: r.get(5)?,
        items_new: r.get(6)?,
        error: r.get(7)?,
    })
}

/// Clauzele WHERE comune căutării în proiecte și în rândurile de agendă.
struct Filters {
    clauses: Vec<String>,
    args: Vec<Value>,
}

impl Filters {
    fn new() -> Self {
        Filters {
            clauses: vec!["1=1".to_owned()],
            args: Vec::new(),
        }
    }

    fn push_arg(&mut self, v: Value) -> usize {
        self.args.push(v);
        self.args.len()
    }

    fn fts(&mut self, id_col: &str, fts_table: &str, key_col: &str, query: Option<&str>) {
        if let Some(q) = query {
            let n = self.push_arg(Value::Text(q.to_owned()));
            self.clauses.push(format!(
                "{id_col} IN (SELECT {key_col} FROM {fts_table} WHERE {fts_table} MATCH ?{n})"
            ));
        }
    }

    fn categories(&mut self, col: &str, cats: &[Category]) {
        if cats.is_empty() {
            return;
        }
        let ph: Vec<String> = cats
            .iter()
            .map(|c| format!("?{}", self.push_arg(Value::Text(c.as_str().to_owned()))))
            .collect();
        self.clauses.push(format!("{col} IN ({})", ph.join(",")));
    }

    fn year(&mut self, col: &str, year: Option<i32>) {
        if let Some(y) = year {
            let a = self.push_arg(Value::Text(format!("{y:04}-01-01")));
            let b = self.push_arg(Value::Text(format!("{y:04}-12-31")));
            self.clauses.push(format!("{col} BETWEEN ?{a} AND ?{b}"));
        }
    }

    fn sql(&self) -> String {
        self.clauses.join(" AND ")
    }
}

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let manager = SqliteConnectionManager::file(path).with_init(|c| {
            c.execute_batch(
                "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; \
                 PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;",
            )
        });
        let pool = Pool::builder().max_size(8).build(manager)?;
        let mut conn = pool.get()?;
        migrations().to_latest(&mut conn)?;
        Ok(Db { pool })
    }

    pub fn conn(&self) -> Result<Conn> {
        Ok(self.pool.get()?)
    }

    // ---- folosite de sync ----

    pub fn meeting_summary(&self, url: &str) -> Result<Option<MeetingSummary>> {
        let conn = self.conn()?;
        let r = conn
            .query_row(
                "SELECT id, date, agenda_url, COALESCE(length(agenda_text), 0) > 0, conclusions_pdf_url \
                 FROM meetings WHERE url = ?1",
                params![url],
                |r| {
                    Ok(MeetingSummary {
                        id: r.get(0)?,
                        date: parse_date(&r.get::<_, String>(1)?),
                        agenda_url: r.get(2)?,
                        has_agenda_text: r.get(3)?,
                        conclusions_pdf_url: r.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(r)
    }

    pub fn item_summaries(&self, urls: &[String]) -> Result<HashMap<String, ItemSummary>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, docs_fetched FROM items WHERE url = ?1")?;
        let mut out = HashMap::new();
        for u in urls {
            let s = stmt
                .query_row(params![u], |r| {
                    Ok(ItemSummary {
                        id: r.get(0)?,
                        docs_fetched: r.get(1)?,
                    })
                })
                .optional()?;
            if let Some(s) = s {
                out.insert(u.clone(), s);
            }
        }
        Ok(out)
    }

    /// Scrie o ședință cu tot ce ține de ea, într-o singură tranzacție. Idempotent pe URL-uri.
    pub fn write_meeting(&self, m: &MeetingWrite) -> Result<WriteReport> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let now = now_iso();

        let meeting_id: i64 = tx.query_row(
            "INSERT INTO meetings (url, title, date, time, agenda_url, agenda_text, conclusions_pdf_url, \
                                   announcement_url, first_seen_at, last_seen_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9) \
             ON CONFLICT(url) DO UPDATE SET \
               title = excluded.title, \
               date = excluded.date, \
               time = COALESCE(excluded.time, meetings.time), \
               agenda_url = COALESCE(excluded.agenda_url, meetings.agenda_url), \
               agenda_text = COALESCE(excluded.agenda_text, meetings.agenda_text), \
               conclusions_pdf_url = COALESCE(excluded.conclusions_pdf_url, meetings.conclusions_pdf_url), \
               announcement_url = COALESCE(excluded.announcement_url, meetings.announcement_url), \
               last_seen_at = excluded.last_seen_at \
             RETURNING id",
            params![
                m.url,
                m.title,
                m.date.to_string(),
                m.time,
                m.agenda_url,
                m.agenda_text,
                m.conclusions_pdf_url,
                m.announcement_url,
                now
            ],
            |r| r.get(0),
        )?;

        let mut item_ids = Vec::with_capacity(m.items.len());
        let mut items_new = 0;
        for it in &m.items {
            let existed = tx
                .query_row("SELECT 1 FROM items WHERE url = ?1", params![it.url], |_| Ok(()))
                .optional()?
                .is_some();
            let id: i64 = tx.query_row(
                "INSERT INTO items (meeting_id, url, title, address, street, category, published_at, beneficiary, \
                                    reg_number, reg_date, revenire, agenda_description, first_seen_at, last_seen_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13) \
                 ON CONFLICT(url) DO UPDATE SET \
                   meeting_id = excluded.meeting_id, \
                   title = excluded.title, \
                   address = COALESCE(excluded.address, items.address), \
                   street = COALESCE(excluded.street, items.street), \
                   category = excluded.category, \
                   published_at = COALESCE(excluded.published_at, items.published_at), \
                   beneficiary = COALESCE(excluded.beneficiary, items.beneficiary), \
                   reg_number = COALESCE(excluded.reg_number, items.reg_number), \
                   reg_date = COALESCE(excluded.reg_date, items.reg_date), \
                   revenire = MAX(excluded.revenire, items.revenire), \
                   agenda_description = COALESCE(excluded.agenda_description, items.agenda_description), \
                   last_seen_at = excluded.last_seen_at \
                 RETURNING id",
                params![
                    meeting_id,
                    it.url,
                    it.title,
                    it.address,
                    it.street,
                    it.category.as_str(),
                    it.published_at,
                    it.beneficiary,
                    it.reg_number,
                    it.reg_date,
                    it.revenire as i64,
                    it.agenda_description,
                    now
                ],
                |r| r.get(0),
            )?;
            if !existed {
                items_new += 1;
            }
            if let Some(docs) = &it.documents {
                tx.execute("DELETE FROM documents WHERE item_id = ?1", params![id])?;
                for d in docs {
                    tx.execute(
                        "INSERT OR IGNORE INTO documents (item_id, label, url) VALUES (?1, ?2, ?3)",
                        params![id, d.label, d.url],
                    )?;
                }
                tx.execute("UPDATE items SET docs_fetched = 1 WHERE id = ?1", params![id])?;
            }
            reindex_item(&tx, id)?;
            item_ids.push(id);
        }

        if let Some(rows) = &m.agenda_rows {
            tx.execute(
                "DELETE FROM agenda_fts WHERE row_id IN (SELECT id FROM agenda_rows WHERE meeting_id = ?1)",
                params![meeting_id],
            )?;
            tx.execute("DELETE FROM agenda_rows WHERE meeting_id = ?1", params![meeting_id])?;
            for r in rows {
                let item_id = r.item_index.and_then(|i| item_ids.get(i).copied());
                tx.execute(
                    "INSERT INTO agenda_rows (meeting_id, nr, reg_number, reg_date, beneficiary, description, \
                                              category, revenire, item_id) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        meeting_id,
                        r.nr.map(i64::from),
                        r.reg_number,
                        r.reg_date,
                        r.beneficiary,
                        r.description,
                        classify(&r.description).as_str(),
                        r.revenire as i64,
                        item_id
                    ],
                )?;
                let row_id = tx.last_insert_rowid();
                let text = fts_text([Some(r.description.as_str()), r.beneficiary.as_deref()]);
                tx.execute(
                    "INSERT INTO agenda_fts (row_id, text) VALUES (?1, ?2)",
                    params![row_id, text],
                )?;
            }
        }

        tx.commit()?;
        Ok(WriteReport {
            meeting_id,
            items_new,
            items_total: m.items.len(),
        })
    }

    pub fn start_sync_run(&self) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute("INSERT INTO sync_runs (started_at) VALUES (?1)", params![now_iso()])?;
        Ok(conn.last_insert_rowid())
    }

    /// Închide rulările rămase „în curs” după o oprire bruscă a procesului; întoarce câte au fost.
    pub fn close_stale_runs(&self) -> Result<usize> {
        let conn = self.conn()?;
        let n = conn.execute(
            "UPDATE sync_runs SET finished_at = ?1, ok = 0,                  error = COALESCE(error, 'întreruptă: procesul s-a oprit înainte de terminare')              WHERE finished_at IS NULL",
            params![now_iso()],
        )?;
        Ok(n)
    }

    pub fn finish_sync_run(&self, id: i64, ok: bool, counts: &RunCounts, error: Option<&str>) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE sync_runs SET finished_at = ?2, ok = ?3, meetings_seen = ?4, meetings_updated = ?5, \
                                  items_new = ?6, error = ?7 WHERE id = ?1",
            params![
                id,
                now_iso(),
                ok as i64,
                counts.meetings_seen as i64,
                counts.meetings_updated as i64,
                counts.items_new as i64,
                error
            ],
        )?;
        Ok(())
    }

    // ---- citire ----

    pub fn search(&self, q: &SearchQuery) -> Result<SearchResult> {
        let conn = self.conn()?;
        let fts = fts_query(&q.q);

        let mut f = Filters::new();
        f.fts("i.id", "items_fts", "item_id", fts.as_deref());
        f.categories("i.category", &q.categories);
        f.year("m.date", q.year);
        let where_sql = f.sql();

        let total: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM items i JOIN meetings m ON m.id = i.meeting_id WHERE {where_sql}"),
            params_from_iter(f.args.iter()),
            |r| r.get(0),
        )?;

        let sql = format!(
            "{ITEM_SELECT} WHERE {where_sql} ORDER BY m.date DESC, i.id ASC LIMIT {} OFFSET {}",
            q.clamped_limit(),
            q.offset
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut items: Vec<Item> = stmt
            .query_map(params_from_iter(f.args.iter()), row_to_item)?
            .collect::<rusqlite::Result<_>>()?;
        attach_documents(&conn, &mut items)?;

        let agenda_only = if fts.is_some() {
            let mut g = Filters::new();
            g.clauses.push("ar.item_id IS NULL".to_owned());
            g.fts("ar.id", "agenda_fts", "row_id", fts.as_deref());
            g.categories("ar.category", &q.categories);
            g.year("m.date", q.year);
            let sql = format!(
                "{AGENDA_SELECT} WHERE {} ORDER BY m.date DESC, ar.nr ASC LIMIT 50",
                g.sql()
            );
            let mut stmt = conn.prepare(&sql)?;
            stmt.query_map(params_from_iter(g.args.iter()), row_to_agenda)?
                .collect::<rusqlite::Result<_>>()?
        } else {
            Vec::new()
        };

        Ok(SearchResult {
            total: total as u32,
            items,
            agenda_only,
        })
    }

    /// Proiectele apărute după `since` (`first_seen_at`, ISO) care se potrivesc interogării FTS (FR-9).
    pub fn new_items_matching(&self, fts: &str, since: &str) -> Result<Vec<Item>> {
        let conn = self.conn()?;
        let sql = format!(
            "{ITEM_SELECT} WHERE i.first_seen_at > ?1 \n             AND i.id IN (SELECT item_id FROM items_fts WHERE items_fts MATCH ?2) \n             ORDER BY m.date DESC, i.id ASC LIMIT 200"
        );
        let mut stmt = conn.prepare(&sql)?;
        Ok(stmt
            .query_map(params![since, fts], row_to_item)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn list_meetings(&self) -> Result<Vec<Meeting>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("{MEETING_SELECT} ORDER BY m.date DESC"))?;
        Ok(stmt.query_map([], row_to_meeting)?.collect::<rusqlite::Result<_>>()?)
    }

    /// Ședințele cu data ≥ `from` pentru care nu s-a trimis încă alerta (FR-8), cele mai apropiate întâi.
    pub fn meetings_to_alert(&self, from: NaiveDate) -> Result<Vec<Meeting>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "{MEETING_SELECT} WHERE m.alerted_at IS NULL AND m.date >= ?1 ORDER BY m.date ASC, m.id ASC"
        ))?;
        Ok(stmt
            .query_map(params![from.to_string()], row_to_meeting)?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Marchează ședințele ca alertate, după ce emailul a plecat.
    pub fn mark_alerted(&self, ids: &[i64]) -> Result<()> {
        let conn = self.conn()?;
        let now = now_iso();
        for id in ids {
            conn.execute("UPDATE meetings SET alerted_at = ?1 WHERE id = ?2", params![now, id])?;
        }
        Ok(())
    }

    pub fn get_meeting(&self, id: i64) -> Result<Option<Meeting>> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                &format!("{MEETING_SELECT} WHERE m.id = ?1"),
                params![id],
                row_to_meeting,
            )
            .optional()?)
    }

    pub fn items_for_meeting(&self, meeting_id: i64) -> Result<Vec<Item>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("{ITEM_SELECT} WHERE i.meeting_id = ?1 ORDER BY i.id ASC"))?;
        let mut items: Vec<Item> = stmt
            .query_map(params![meeting_id], row_to_item)?
            .collect::<rusqlite::Result<_>>()?;
        attach_documents(&conn, &mut items)?;
        Ok(items)
    }

    pub fn agenda_rows_for_meeting(&self, meeting_id: i64) -> Result<Vec<AgendaRowView>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "{AGENDA_SELECT} WHERE ar.meeting_id = ?1 ORDER BY ar.nr ASC, ar.id ASC"
        ))?;
        Ok(stmt
            .query_map(params![meeting_id], row_to_agenda)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_item(&self, id: i64) -> Result<Option<Item>> {
        let conn = self.conn()?;
        let item = conn
            .query_row(&format!("{ITEM_SELECT} WHERE i.id = ?1"), params![id], row_to_item)
            .optional()?;
        match item {
            Some(mut it) => {
                let mut v = vec![std::mem::take(&mut it)];
                attach_documents(&conn, &mut v)?;
                Ok(v.pop())
            }
            None => Ok(None),
        }
    }

    /// Străzile distincte, cu numărul de proiecte, pentru autocomplete.
    pub fn streets(&self) -> Result<Vec<(String, i64)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT street, COUNT(*) FROM items WHERE street IS NOT NULL AND street <> '' \
             GROUP BY street ORDER BY street COLLATE NOCASE",
        )?;
        Ok(stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn status(&self) -> Result<Status> {
        let conn = self.conn()?;
        let (meetings, items, unmatched): (i64, i64, i64) = conn.query_row(
            "SELECT (SELECT COUNT(*) FROM meetings), (SELECT COUNT(*) FROM items), \
                    (SELECT COUNT(*) FROM agenda_rows WHERE item_id IS NULL)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let last_run = conn
            .query_row(
                "SELECT id, started_at, finished_at, ok, meetings_seen, meetings_updated, items_new, error \
                 FROM sync_runs ORDER BY id DESC LIMIT 1",
                [],
                row_to_sync_run,
            )
            .optional()?;
        Ok(Status {
            meetings,
            items,
            agenda_rows_unmatched: unmatched,
            last_run,
        })
    }
}

fn reindex_item(tx: &Transaction<'_>, id: i64) -> Result<()> {
    let text: String = tx.query_row(
        "SELECT title, address, street, beneficiary, agenda_description FROM items WHERE id = ?1",
        params![id],
        |r| {
            let parts: [Option<String>; 5] = [r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?];
            Ok(fts_text(parts.iter().map(|p| p.as_deref())))
        },
    )?;
    tx.execute("DELETE FROM items_fts WHERE item_id = ?1", params![id])?;
    tx.execute(
        "INSERT INTO items_fts (item_id, text) VALUES (?1, ?2)",
        params![id, text],
    )?;
    Ok(())
}

fn attach_documents(conn: &Connection, items: &mut [Item]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let ids: Vec<Value> = items.iter().map(|i| Value::Integer(i.id)).collect();
    let ph: Vec<String> = (1..=ids.len()).map(|n| format!("?{n}")).collect();
    let sql = format!(
        "SELECT item_id, label, url FROM documents WHERE item_id IN ({}) ORDER BY id",
        ph.join(",")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut by_item: HashMap<i64, Vec<Document>> = HashMap::new();
    let rows = stmt.query_map(params_from_iter(ids.iter()), |r| {
        Ok((
            r.get::<_, i64>(0)?,
            Document {
                label: r.get(1)?,
                url: r.get(2)?,
            },
        ))
    })?;
    for row in rows {
        let (item_id, doc) = row?;
        by_item.entry(item_id).or_default().push(doc);
    }
    for it in items.iter_mut() {
        if let Some(docs) = by_item.remove(&it.id) {
            it.documents = docs;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alertele_se_trimit_o_singura_data() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("t.db")).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        db.write_meeting(&MeetingWrite {
            url: "https://x/sedinta".into(),
            title: "S".into(),
            date,
            time: None,
            agenda_url: None,
            agenda_text: None,
            conclusions_pdf_url: None,
            announcement_url: None,
            items: vec![],
            agenda_rows: None,
        })
        .unwrap();
        let pending = db.meetings_to_alert(date).unwrap();
        assert_eq!(pending.len(), 1);
        // o ședință din trecut nu se alertează
        assert!(db.meetings_to_alert(date.succ_opt().unwrap()).unwrap().is_empty());
        db.mark_alerted(&[pending[0].id]).unwrap();
        assert!(db.meetings_to_alert(date).unwrap().is_empty());
    }

    #[test]
    fn fts_query_builds_prefix_terms() {
        assert_eq!(
            fts_query("Brâncuși nr. 107").as_deref(),
            Some("\"brancusi\"* \"nr\"* \"107\"*")
        );
        assert_eq!(fts_query("   "), None);
    }
}
