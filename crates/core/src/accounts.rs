//! Conturi, sesiuni, cuvinte-cheie și credite (FR-9, ADR-0012). Modelul e cel din oracle.gsl, redus
//! la un singur utilizator per cont: creditele stau pe utilizator, ca două contoare modificate în
//! tranzacție, cu un jurnal append-only și chei de idempotență; jetoanele de autentificare și de
//! sesiune se păstrează doar ca hash SHA-256; ciclul anual se rotește leneș, la citire, fără
//! planificator.
//!
//! Un credit = un cuvânt-cheie ținut un an. Planul unic e `free`: [`FREE_CREDITS_PER_YEAR`] credite
//! pe an, deci atâtea cuvinte-cheie, cu alerte nelimitate pentru ele. Creditul se consumă la
//! adăugare și nu se recuperează la ștergere (altfel rotirea săptămânală a cuvintelor ar face din
//! două locuri oricâte); la reînnoirea ciclului, cuvintele ținute se reînnoiesc din creditele noi.

use chrono::{DateTime, Datelike, Duration, NaiveDate, SecondsFormat, Utc};
use rand::RngCore;
use rusqlite::{OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use urban_shared::text::tokens;

use crate::db::{Db, now_iso};
use crate::{Error, Result};

pub const FREE_CREDITS_PER_YEAR: i64 = 2;
pub const MAGIC_LINK_TTL_MINUTES: i64 = 15;
/// Cel mult un link de autentificare per adresă în acest interval.
pub const LOGIN_THROTTLE_SECONDS: i64 = 120;
pub const SESSION_TTL_DAYS: i64 = 30;
pub const KEYWORD_MIN_CHARS: usize = 3;
pub const KEYWORD_MAX_CHARS: usize = 60;
pub const KEYWORD_MAX_WORDS: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub plan: String,
    pub credits_available: i64,
    pub credits_used: i64,
    pub cycle_start_at: String,
    pub cycle_end_at: String,
    /// Proiectele cu `first_seen_at` până aici au fost deja evaluate pentru alerte.
    pub alerts_checked_until: String,
}

impl User {
    pub fn balance(&self) -> i64 {
        self.credits_available - self.credits_used
    }

    pub fn cycle_end_date(&self) -> NaiveDate {
        parse_iso(&self.cycle_end_at).date_naive()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Keyword {
    pub id: i64,
    pub user_id: i64,
    /// Așa cum l-a scris utilizatorul, cu spațiile strânse.
    pub text: String,
    /// Forma normalizată (fără diacritice, litere mici), cea căutată în FTS.
    pub normalized: String,
}

/// 32 de octeți aleatori, hex: jetonul din link sau din cookie. În bază ajunge doar [`hash_token`].
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex(&bytes)
}

pub fn hash_token(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Adresa curățată (litere mici) sau `None` dacă nu arată ca un email.
pub fn normalize_email(s: &str) -> Option<String> {
    let e = s.trim().to_lowercase();
    let (local, domain) = e.split_once('@')?;
    let ok = !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !e.contains(char::is_whitespace)
        && e.len() <= 254;
    ok.then_some(e)
}

/// Cuvântul-cheie pentru afișare și forma normalizată pentru căutare; `Err` e motivul, gata de afișat.
pub fn validate_keyword(text: &str) -> std::result::Result<(String, String), String> {
    let display = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if display.chars().count() > KEYWORD_MAX_CHARS {
        return Err(format!("cel mult {KEYWORD_MAX_CHARS} de caractere"));
    }
    let toks = tokens(&display);
    if toks.is_empty() {
        return Err("scrie un cuvânt-cheie".to_owned());
    }
    if toks.len() > KEYWORD_MAX_WORDS {
        return Err(format!("cel mult {KEYWORD_MAX_WORDS} cuvinte"));
    }
    if toks.iter().any(|t| t.chars().count() < KEYWORD_MIN_CHARS) {
        return Err(format!("fiecare cuvânt are cel puțin {KEYWORD_MIN_CHARS} caractere"));
    }
    Ok((display, toks.join(" ")))
}

fn parse_iso(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn iso(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Aceeași zi și oră, anul următor (29 februarie → 28 februarie).
fn plus_one_year(t: DateTime<Utc>) -> DateTime<Utc> {
    t.with_year(t.year() + 1).unwrap_or_else(|| t + Duration::days(365))
}

fn row_to_user(r: &rusqlite::Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: r.get(0)?,
        email: r.get(1)?,
        plan: r.get(2)?,
        credits_available: r.get(3)?,
        credits_used: r.get(4)?,
        cycle_start_at: r.get(5)?,
        cycle_end_at: r.get(6)?,
        alerts_checked_until: r.get(7)?,
    })
}

const USER_SELECT: &str = "SELECT id, email, plan, credits_available, credits_used, cycle_start_at, cycle_end_at, \
     alerts_checked_until FROM users";

#[allow(clippy::too_many_arguments)] // un parametru per coloană din credit_ledger
fn ledger(
    tx: &Transaction<'_>,
    user_id: i64,
    direction: &str,
    amount: i64,
    reason: &str,
    key: &str,
    balance_after: i64,
    now: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO credit_ledger (user_id, direction, amount, reason, idempotency_key, balance_after, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![user_id, direction, amount, reason, key, balance_after, now],
    )?;
    Ok(())
}

impl Db {
    // ── autentificare ──────────────────────────────────────────────────────────────────────────

    /// Creează un link de autentificare și întoarce jetonul de pus în URL; `None` dacă adresa a
    /// primit deja unul în ultimele [`LOGIN_THROTTLE_SECONDS`].
    pub fn create_magic_link(&self, email: &str, consent: bool) -> Result<Option<String>> {
        let conn = self.conn()?;
        let now = Utc::now();
        let recent: i64 = conn.query_row(
            "SELECT COUNT(*) FROM magic_links WHERE email = ?1 AND created_at > ?2",
            params![email, iso(now - Duration::seconds(LOGIN_THROTTLE_SECONDS))],
            |r| r.get(0),
        )?;
        if recent > 0 {
            return Ok(None);
        }
        let token = new_token();
        conn.execute(
            "INSERT INTO magic_links (email, token_hash, consent, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                email,
                hash_token(&token),
                consent as i64,
                iso(now + Duration::minutes(MAGIC_LINK_TTL_MINUTES)),
                iso(now)
            ],
        )?;
        Ok(Some(token))
    }

    /// Consumă linkul: `(email, consent)` dacă e valid, nefolosit și neexpirat. Un jeton merge o singură dată.
    pub fn consume_magic_link(&self, token: &str) -> Result<Option<(String, bool)>> {
        let conn = self.conn()?;
        let now = now_iso();
        let row: Option<(i64, String, i64)> = conn
            .query_row(
                "SELECT id, email, consent FROM magic_links WHERE token_hash = ?1 AND used_at IS NULL AND expires_at > ?2",
                params![hash_token(token), now],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((id, email, consent)) = row else {
            return Ok(None);
        };
        conn.execute("UPDATE magic_links SET used_at = ?1 WHERE id = ?2", params![now, id])?;
        Ok(Some((email, consent != 0)))
    }

    /// Utilizatorul cu această adresă, creat la prima autentificare (cu grantul gratuit, într-o
    /// tranzacție). Un cont nou cere acordul bifat în formular.
    pub fn find_or_create_user(&self, email: &str, consent: bool) -> Result<User> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let now = Utc::now();
        let now_s = iso(now);
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM users WHERE email = ?1", params![email], |r| r.get(0))
            .optional()?;
        let id = match existing {
            Some(id) => {
                tx.execute("UPDATE users SET last_login_at = ?1 WHERE id = ?2", params![now_s, id])?;
                id
            }
            None => {
                if !consent {
                    return Err(Error::Other(
                        "un cont nou are nevoie de acordul pentru prelucrarea adresei de email".to_owned(),
                    ));
                }
                tx.execute(
                    "INSERT INTO users (email, plan, credits_available, credits_used, cycle_start_at, cycle_end_at, \
                     alerts_checked_until, consent_at, created_at, last_login_at) \
                     VALUES (?1, 'free', ?2, 0, ?3, ?4, ?3, ?3, ?3, ?3)",
                    params![email, FREE_CREDITS_PER_YEAR, now_s, iso(plus_one_year(now))],
                )?;
                let id = tx.last_insert_rowid();
                ledger(
                    &tx,
                    id,
                    "credit",
                    FREE_CREDITS_PER_YEAR,
                    "initial-free-grant",
                    &format!("initial-free-grant:{id}"),
                    FREE_CREDITS_PER_YEAR,
                    &now_s,
                )?;
                id
            }
        };
        tx.commit()?;
        self.user(id)?
            .ok_or_else(|| Error::Other("utilizatorul tocmai creat lipsește".to_owned()))
    }

    pub fn user(&self, id: i64) -> Result<Option<User>> {
        let conn = self.conn()?;
        let user = conn
            .query_row(&format!("{USER_SELECT} WHERE id = ?1"), params![id], row_to_user)
            .optional()?;
        drop(conn);
        user.map(|u| self.ensure_cycle(u)).transpose()
    }

    /// Ciclul anual, rotit leneș la prima citire de după `cycle_end_at`: creditele revin la
    /// [`FREE_CREDITS_PER_YEAR`], apoi cuvintele-cheie ținute se reînnoiesc din ele, câte un credit
    /// fiecare. Totul într-o tranzacție, cu rânduri idempotente în jurnal.
    fn ensure_cycle(&self, user: User) -> Result<User> {
        let now = Utc::now();
        let mut end = parse_iso(&user.cycle_end_at);
        if now < end {
            return Ok(user);
        }
        let mut start = end;
        while end <= now {
            start = end;
            end = plus_one_year(end);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let (start_s, end_s, now_s) = (iso(start), iso(end), iso(now));
        let held: Vec<i64> = {
            let mut st = tx.prepare("SELECT id FROM keywords WHERE user_id = ?1 ORDER BY id")?;
            st.query_map(params![user.id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        let used = (held.len() as i64).min(FREE_CREDITS_PER_YEAR);
        tx.execute(
            "UPDATE users SET credits_available = ?1, credits_used = ?2, cycle_start_at = ?3, cycle_end_at = ?4 \
             WHERE id = ?5 AND cycle_end_at = ?6",
            params![FREE_CREDITS_PER_YEAR, used, start_s, end_s, user.id, user.cycle_end_at],
        )?;
        ledger(
            &tx,
            user.id,
            "credit",
            FREE_CREDITS_PER_YEAR,
            "cycle-reset",
            &format!("cycle-reset:{}:{start_s}", user.id),
            FREE_CREDITS_PER_YEAR,
            &now_s,
        )?;
        let mut balance = FREE_CREDITS_PER_YEAR;
        for kw in held.iter().take(FREE_CREDITS_PER_YEAR as usize) {
            balance -= 1;
            ledger(
                &tx,
                user.id,
                "debit",
                1,
                "keyword-renewal",
                &format!("keyword-renewal:{}:{kw}:{start_s}", user.id),
                balance,
                &now_s,
            )?;
        }
        tx.commit()?;
        Ok(User {
            credits_available: FREE_CREDITS_PER_YEAR,
            credits_used: used,
            cycle_start_at: start_s,
            cycle_end_at: end_s,
            ..user
        })
    }

    pub fn create_session(&self, user_id: i64) -> Result<String> {
        let conn = self.conn()?;
        let token = new_token();
        let now = Utc::now();
        conn.execute(
            "INSERT INTO sessions (token_hash, user_id, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                hash_token(&token),
                user_id,
                iso(now + Duration::days(SESSION_TTL_DAYS)),
                iso(now)
            ],
        )?;
        Ok(token)
    }

    pub fn user_by_session(&self, token: &str) -> Result<Option<User>> {
        let conn = self.conn()?;
        let user_id: Option<i64> = conn
            .query_row(
                "SELECT user_id FROM sessions WHERE token_hash = ?1 AND expires_at > ?2",
                params![hash_token(token), now_iso()],
                |r| r.get(0),
            )
            .optional()?;
        drop(conn);
        match user_id {
            Some(id) => self.user(id),
            None => Ok(None),
        }
    }

    pub fn delete_session(&self, token: &str) -> Result<()> {
        self.conn()?
            .execute("DELETE FROM sessions WHERE token_hash = ?1", params![hash_token(token)])?;
        Ok(())
    }

    /// Sesiunile și linkurile expirate de peste o zi; rulat după fiecare sincronizare.
    pub fn sweep_expired(&self) -> Result<()> {
        let conn = self.conn()?;
        let cutoff = iso(Utc::now() - Duration::days(1));
        conn.execute("DELETE FROM sessions WHERE expires_at < ?1", params![cutoff])?;
        conn.execute("DELETE FROM magic_links WHERE expires_at < ?1", params![cutoff])?;
        Ok(())
    }

    // ── cuvinte-cheie ──────────────────────────────────────────────────────────────────────────

    pub fn keywords(&self, user_id: i64) -> Result<Vec<Keyword>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT id, user_id, text, normalized FROM keywords WHERE user_id = ?1 ORDER BY id")?;
        Ok(stmt
            .query_map(params![user_id], |r| {
                Ok(Keyword {
                    id: r.get(0)?,
                    user_id: r.get(1)?,
                    text: r.get(2)?,
                    normalized: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Adaugă un cuvânt-cheie și consumă un credit, în aceeași tranzacție. `Ok(Err(motiv))` când e
    /// refuzat: invalid, dublură sau fără credite.
    pub fn add_keyword(&self, user: &User, text: &str) -> Result<std::result::Result<Keyword, String>> {
        let (display, normalized) = match validate_keyword(text) {
            Ok(v) => v,
            Err(reason) => return Ok(Err(reason)),
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let dup: i64 = tx.query_row(
            "SELECT COUNT(*) FROM keywords WHERE user_id = ?1 AND normalized = ?2",
            params![user.id, normalized],
            |r| r.get(0),
        )?;
        if dup > 0 {
            return Ok(Err("ai deja acest cuvânt-cheie".to_owned()));
        }
        let (available, used): (i64, i64) = tx.query_row(
            "SELECT credits_available, credits_used FROM users WHERE id = ?1",
            params![user.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if available - used < 1 {
            return Ok(Err(format!(
                "nu mai ai credite anul acesta; se reînnoiesc pe {}",
                user.cycle_end_date().format("%d.%m.%Y")
            )));
        }
        let now = now_iso();
        tx.execute(
            "INSERT INTO keywords (user_id, text, normalized, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![user.id, display, normalized, now],
        )?;
        let id = tx.last_insert_rowid();
        // id-ul unui cuvânt șters se refolosește, iar rândul lui din registru rămâne: numărul de ordine
        // al debitului ține cheia unică
        let seq: i64 = tx.query_row(
            "SELECT COUNT(*) + 1 FROM credit_ledger WHERE user_id = ?1 AND reason = 'keyword'",
            params![user.id],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE users SET credits_used = credits_used + 1 WHERE id = ?1",
            params![user.id],
        )?;
        ledger(
            &tx,
            user.id,
            "debit",
            1,
            "keyword",
            &format!("keyword:{}:{id}:{seq}", user.id),
            available - used - 1,
            &now,
        )?;
        tx.commit()?;
        Ok(Ok(Keyword {
            id,
            user_id: user.id,
            text: display,
            normalized,
        }))
    }

    /// Șterge cuvântul; creditul rămâne consumat până la reînnoirea ciclului.
    pub fn delete_keyword(&self, user_id: i64, id: i64) -> Result<bool> {
        let n = self.conn()?.execute(
            "DELETE FROM keywords WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )?;
        Ok(n > 0)
    }

    /// Șterge contul și, prin cascadă, sesiunile, cuvintele, jurnalul și alertele lui.
    pub fn delete_user(&self, id: i64) -> Result<()> {
        self.conn()?.execute("DELETE FROM users WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ── alerte ─────────────────────────────────────────────────────────────────────────────────

    /// Utilizatorii cu cel puțin un cuvânt-cheie, cu ciclul la zi.
    pub fn users_with_keywords(&self) -> Result<Vec<User>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "{USER_SELECT} WHERE id IN (SELECT DISTINCT user_id FROM keywords) ORDER BY id"
        ))?;
        let users: Vec<User> = stmt.query_map([], row_to_user)?.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        drop(conn);
        users.into_iter().map(|u| self.ensure_cycle(u)).collect()
    }

    pub fn set_alerts_checked_until(&self, user_id: i64, until: &str) -> Result<()> {
        self.conn()?.execute(
            "UPDATE users SET alerts_checked_until = ?1 WHERE id = ?2",
            params![until, user_id],
        )?;
        Ok(())
    }

    pub fn log_alert(&self, user_id: i64, items: usize, window_until: &str) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO alert_log (user_id, kind, items, window_until, sent_at) VALUES (?1, 'digest', ?2, ?3, ?4)",
            params![user_id, items as i64, window_until, now_iso()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("t.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn email_si_cuvinte_validate() {
        assert_eq!(normalize_email("  Ana@Exemplu.RO "), Some("ana@exemplu.ro".to_owned()));
        assert_eq!(normalize_email("fara-arond"), None);
        assert_eq!(normalize_email("a@b"), None);
        assert_eq!(
            validate_keyword("  Viile   Dâmbul Rotund ").unwrap(),
            ("Viile Dâmbul Rotund".to_owned(), "viile dambul rotund".to_owned())
        );
        assert!(validate_keyword("nr").is_err());
        assert!(validate_keyword("a b c d e f").is_err());
    }

    #[test]
    fn link_o_singura_data_si_cont_cu_grant() {
        let (_d, db) = db();
        let token = db.create_magic_link("ana@exemplu.ro", true).unwrap().unwrap();
        assert!(
            db.create_magic_link("ana@exemplu.ro", true).unwrap().is_none(),
            "throttle"
        );
        assert_eq!(db.consume_magic_link("gresit").unwrap(), None);
        assert_eq!(
            db.consume_magic_link(&token).unwrap(),
            Some(("ana@exemplu.ro".to_owned(), true))
        );
        assert_eq!(db.consume_magic_link(&token).unwrap(), None, "al doilea click nu merge");

        let u = db.find_or_create_user("ana@exemplu.ro", true).unwrap();
        assert_eq!(u.balance(), FREE_CREDITS_PER_YEAR);
        let again = db.find_or_create_user("ana@exemplu.ro", false).unwrap();
        assert_eq!(again.id, u.id, "acelasi cont la a doua autentificare");
        assert!(
            db.find_or_create_user("nou@exemplu.ro", false).is_err(),
            "cont nou fara acord"
        );
    }

    #[test]
    fn un_credit_un_cuvant() {
        let (_d, db) = db();
        let u = db.find_or_create_user("ana@exemplu.ro", true).unwrap();
        let s = db.create_session(u.id).unwrap();
        assert_eq!(db.user_by_session(&s).unwrap().map(|x| x.id), Some(u.id));
        assert!(db.user_by_session("altul").unwrap().is_none());

        let k1 = db.add_keyword(&u, "Viile Dâmbul Rotund").unwrap().unwrap();
        assert!(
            db.add_keyword(&u, "viile dambul rotund").unwrap().is_err(),
            "dublura normalizata"
        );
        let u = db.user(u.id).unwrap().unwrap();
        assert_eq!(u.balance(), 1);
        db.add_keyword(&u, "Fabricii").unwrap().unwrap();
        let u = db.user(u.id).unwrap().unwrap();
        assert_eq!(u.balance(), 0);
        let refuz = db.add_keyword(&u, "Observatorului").unwrap().unwrap_err();
        assert!(refuz.contains("nu mai ai credite"), "{refuz}");
        assert_eq!(db.keywords(u.id).unwrap().len(), 2);
        assert_eq!(db.users_with_keywords().unwrap().len(), 1);

        assert!(db.delete_keyword(u.id, k1.id).unwrap());
        let u = db.user(u.id).unwrap().unwrap();
        assert_eq!(u.balance(), 0, "stergerea nu returneaza creditul");

        db.delete_session(&s).unwrap();
        assert!(db.user_by_session(&s).unwrap().is_none());
        db.delete_user(u.id).unwrap();
        assert!(db.keywords(u.id).unwrap().is_empty(), "cascada");
    }

    #[test]
    fn cuvant_nou_dupa_stergere() {
        // SQLite refolosește id-ul ultimului rând șters; cheia din registru nu trebuie să se ciocnească
        let (_d, db) = db();
        let u = db.find_or_create_user("ana@exemplu.ro", true).unwrap();
        let k1 = db.add_keyword(&u, "Fabricii").unwrap().unwrap();
        assert!(db.delete_keyword(u.id, k1.id).unwrap());
        let u = db.user(u.id).unwrap().unwrap();
        db.add_keyword(&u, "Observatorului").unwrap().unwrap();
        let u = db.user(u.id).unwrap().unwrap();
        assert_eq!(u.balance(), 0);
    }

    #[test]
    fn ciclul_anual_reinnoieste_cuvintele_tinute() {
        let (_d, db) = db();
        let u = db.find_or_create_user("ana@exemplu.ro", true).unwrap();
        let k1 = db.add_keyword(&u, "Fabricii").unwrap().unwrap();
        let u = db.user(u.id).unwrap().unwrap();
        db.add_keyword(&u, "Observatorului").unwrap().unwrap();
        db.delete_keyword(u.id, k1.id).unwrap();
        // mutăm sfârșitul ciclului în trecut, ca și cum ar fi trecut un an
        let past = iso(Utc::now() - Duration::days(1));
        db.conn()
            .unwrap()
            .execute("UPDATE users SET cycle_end_at = ?1 WHERE id = ?2", params![past, u.id])
            .unwrap();
        let fresh = db.user(u.id).unwrap().unwrap();
        assert_eq!(
            fresh.balance(),
            FREE_CREDITS_PER_YEAR - 1,
            "un cuvant tinut, un credit liber"
        );
        assert!(parse_iso(&fresh.cycle_end_at) > Utc::now());
        let (resets, renewals): (i64, i64) = db
            .conn()
            .unwrap()
            .query_row(
                "SELECT SUM(reason = 'cycle-reset'), SUM(reason = 'keyword-renewal') FROM credit_ledger WHERE user_id = ?1",
                params![u.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((resets, renewals), (1, 1));
    }
}
