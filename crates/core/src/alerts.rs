//! Alertele pe cuvinte-cheie (FR-9, FR-10.4): după fiecare sincronizare, pentru fiecare utilizator
//! cu cuvinte-cheie, proiectele și certificatele de urbanism apărute de la ultima verificare
//! (`first_seen_at` după `alerts_checked_until`) se caută cu aceeași interogare FTS ca pe site;
//! potrivirile pleacă într-un singur email pe utilizator, apoi fereastra avansează. Dacă emailul
//! nu pleacă, fereastra nu avansează și se reîncearcă la sincronizarea următoare. Alertele nu costă
//! credite: creditul s-a plătit la adăugarea cuvântului.

use std::collections::HashSet;

use tracing::{info, warn};
use urban_shared::{Certificate, Item};

use crate::Result;
use crate::accounts::Keyword;
use crate::db::{Db, fts_query, now_iso};
use crate::notify::Mailer;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AlertStats {
    pub users: usize,
    pub emails: usize,
    pub items: usize,
    pub errors: usize,
}

/// Noutățile prinse de un cuvânt-cheie: proiecte de pe ordinea de zi și certificate de urbanism.
pub struct KeywordMatches {
    pub keyword: Keyword,
    pub items: Vec<Item>,
    pub certificates: Vec<Certificate>,
}

/// Potrivirile unui utilizator într-o fereastră, per cuvânt-cheie, fără dubluri.
pub struct Matches {
    pub keywords: Vec<KeywordMatches>,
}

impl Matches {
    pub fn total(&self) -> usize {
        self.total_items() + self.total_certificates()
    }

    pub fn total_items(&self) -> usize {
        self.keywords.iter().map(|k| k.items.len()).sum()
    }

    pub fn total_certificates(&self) -> usize {
        self.keywords.iter().map(|k| k.certificates.len()).sum()
    }
}

/// Proiectele și certificatele noi de după `since` care se potrivesc cuvintelor date; fiecare apare
/// o singură dată, la primul cuvânt care îl prinde.
pub fn find_matches(db: &Db, keywords: &[Keyword], since: &str) -> Result<Matches> {
    let mut seen_items: HashSet<i64> = HashSet::new();
    let mut seen_certs: HashSet<i64> = HashSet::new();
    let mut out = Vec::new();
    for k in keywords {
        let Some(fts) = fts_query(&k.normalized) else {
            continue;
        };
        let items: Vec<Item> = db
            .new_items_matching(&fts, since)?
            .into_iter()
            .filter(|i| seen_items.insert(i.id))
            .collect();
        let certificates: Vec<Certificate> = db
            .new_certificates_matching(&fts, since)?
            .into_iter()
            .filter(|c| seen_certs.insert(c.id))
            .collect();
        if !items.is_empty() || !certificates.is_empty() {
            out.push(KeywordMatches {
                keyword: k.clone(),
                items,
                certificates,
            });
        }
    }
    Ok(Matches { keywords: out })
}

/// Rulează alertele pentru toți utilizatorii cu cuvinte-cheie.
pub async fn run_keyword_alerts(db: &Db, mailer: &Mailer) -> Result<AlertStats> {
    db.sweep_expired()?;
    let mut stats = AlertStats::default();
    for user in db.users_with_keywords()? {
        stats.users += 1;
        let now = now_iso();
        let since = user.alerts_checked_until.clone();
        let keywords = db.keywords(user.id)?;
        let matches = find_matches(db, &keywords, &since)?;
        if matches.keywords.is_empty() {
            db.set_alerts_checked_until(user.id, &now)?;
            continue;
        }
        match mailer.send_keyword_digest(&user.email, &matches).await {
            Ok(()) => {
                let n = matches.total();
                db.log_alert(user.id, n, &now)?;
                db.set_alerts_checked_until(user.id, &now)?;
                stats.emails += 1;
                stats.items += n;
                info!(user = user.id, items = n, "alertă pe cuvinte-cheie trimisă");
            }
            Err(e) => {
                stats.errors += 1;
                warn!(user = user.id, error = %e, "alerta pe cuvinte-cheie a eșuat; reîncerc la următoarea sincronizare");
            }
        }
    }
    Ok(stats)
}
