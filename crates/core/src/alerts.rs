//! Alertele pe cuvinte-cheie (FR-9): după fiecare sincronizare, pentru fiecare utilizator cu
//! cuvinte-cheie, proiectele apărute de la ultima verificare (`first_seen_at` după
//! `alerts_checked_until`) se caută cu aceeași interogare FTS ca pe site; potrivirile pleacă
//! într-un singur email pe utilizator, apoi fereastra avansează. Dacă emailul nu pleacă,
//! fereastra nu avansează și se reîncearcă la sincronizarea următoare. Alertele nu costă credite:
//! creditul s-a plătit la adăugarea cuvântului.

use std::collections::HashSet;

use tracing::{info, warn};
use urban_shared::Item;

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

/// Potrivirile unui utilizator într-o fereastră: per cuvânt-cheie, proiectele noi (fără dubluri).
pub struct Matches {
    pub keywords: Vec<(Keyword, Vec<Item>)>,
}

impl Matches {
    pub fn total(&self) -> usize {
        self.keywords.iter().map(|(_, items)| items.len()).sum()
    }
}

/// Proiectele noi de după `since` care se potrivesc cuvintelor date; un proiect apare o singură dată,
/// la primul cuvânt care îl prinde.
pub fn find_matches(db: &Db, keywords: &[Keyword], since: &str) -> Result<Matches> {
    let mut seen: HashSet<i64> = HashSet::new();
    let mut out = Vec::new();
    for k in keywords {
        let Some(fts) = fts_query(&k.normalized) else {
            continue;
        };
        let items: Vec<Item> = db
            .new_items_matching(&fts, since)?
            .into_iter()
            .filter(|i| seen.insert(i.id))
            .collect();
        if !items.is_empty() {
            out.push((k.clone(), items));
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
