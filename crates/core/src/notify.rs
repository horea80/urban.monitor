//! Emailuri prin Resend (ADR-0011): alerta la ședințe noi (FR-8), linkul de autentificare și
//! rezumatul pe cuvinte-cheie (FR-9). Randarea e în funcții pure, testabile; trimiterea e async
//! (reqwest), cu timeout, în task-ul care o cere; serverul web nu așteaptă după ea. Fără
//! `RESEND_API_KEY` nu există `Mailer`, iar apelanții decid ce fac (alertele se opresc, linkul de
//! autentificare se scrie în jurnal).

use std::time::Duration;

use chrono::Local;
use serde::Serialize;
use tracing::info;
use urban_shared::time::fmt_date_ro;
use urban_shared::{Item, Meeting};

use crate::alerts::Matches;
use crate::config::{AlertConfig, Config};
use crate::db::Db;
use crate::{Error, Result};

const RESEND_URL: &str = "https://api.resend.com/emails";
const TIMEOUT: Duration = Duration::from_secs(15);

pub struct Mailer {
    client: reqwest::Client,
    alert: AlertConfig,
    public_url: String,
}

#[derive(Serialize)]
struct Payload<'a> {
    from: &'a str,
    to: &'a [String],
    reply_to: &'a str,
    subject: &'a str,
    text: &'a str,
    html: &'a str,
}

impl Mailer {
    /// `Ok(None)` când alertele sunt oprite (fără `RESEND_API_KEY`).
    pub fn from_config(cfg: &Config) -> Result<Option<Mailer>> {
        let Some(alert) = cfg.alert.clone() else {
            return Ok(None);
        };
        let client = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(&cfg.user_agent)
            .build()
            .map_err(|e| Error::Http {
                url: RESEND_URL.to_owned(),
                source: e,
            })?;
        Ok(Some(Mailer {
            client,
            alert,
            public_url: cfg.public_url.clone(),
        }))
    }

    pub fn recipients(&self) -> &[String] {
        &self.alert.to
    }

    /// FR-8: ședințele încă nealertate cu data ≥ azi → un email către lista fixă; marcate abia după
    /// ce emailul a plecat.
    pub async fn alert_new_meetings(&self, db: &Db) -> Result<usize> {
        let today = Local::now().date_naive();
        let meetings = db.meetings_to_alert(today)?;
        if meetings.is_empty() {
            return Ok(0);
        }
        let (subject, text, html) = render(&self.public_url, &meetings);
        self.send(&self.alert.to, &subject, &text, &html).await?;
        let ids: Vec<i64> = meetings.iter().map(|m| m.id).collect();
        db.mark_alerted(&ids)?;
        info!(n = meetings.len(), to = ?self.alert.to, "alertă trimisă pentru ședințe noi");
        Ok(meetings.len())
    }

    /// Un email de probă, ca să verificăm cheia, expeditorul și destinatarii.
    pub async fn send_test(&self) -> Result<()> {
        let text = format!("Alertele Monitor Urban funcționează. Site: {}", self.public_url);
        let html = format!(
            "<p>Alertele Monitor Urban funcționează.</p><p><a href=\"{0}\">{0}</a></p>",
            esc(&self.public_url)
        );
        self.send(&self.alert.to, "Test: alertele Monitor Urban", &text, &html)
            .await
    }

    /// FR-9: linkul de autentificare, valabil câteva minute, către o singură adresă.
    pub async fn send_login_link(&self, email: &str, url: &str) -> Result<()> {
        let (subject, text, html) = render_login(url);
        self.send(&[email.to_owned()], &subject, &text, &html).await
    }

    /// FR-9: proiectele noi care se potrivesc cuvintelor-cheie ale unui utilizator.
    pub async fn send_keyword_digest(&self, email: &str, matches: &Matches) -> Result<()> {
        let (subject, text, html) = render_digest(&self.public_url, matches);
        self.send(&[email.to_owned()], &subject, &text, &html).await
    }

    async fn send(&self, to: &[String], subject: &str, text: &str, html: &str) -> Result<()> {
        let payload = Payload {
            from: &self.alert.from,
            to,
            reply_to: self.alert.to.first().map(String::as_str).unwrap_or_default(),
            subject,
            text,
            html,
        };
        let resp = self
            .client
            .post(RESEND_URL)
            .bearer_auth(&self.alert.resend_api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Http {
                url: RESEND_URL.to_owned(),
                source: e,
            })?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let body: String = body.chars().take(300).collect();
            return Err(Error::Other(format!("Resend a răspuns {status}: {body}")));
        }
        Ok(())
    }
}

/// Subiectul, textul simplu și HTML-ul emailului pentru ședințele date (cel puțin una).
pub fn render(public_url: &str, meetings: &[Meeting]) -> (String, String, String) {
    let dates: Vec<String> = meetings.iter().map(|m| fmt_date_ro(m.date)).collect();
    let subject = if meetings.len() == 1 {
        format!("Ședință CTATU nouă: {}", dates[0])
    } else {
        format!("{} ședințe CTATU noi: {}", meetings.len(), dates.join(", "))
    };
    let mut text = String::from("Ședințe noi anunțate de Primăria Cluj-Napoca:\n\n");
    let mut html = String::from("<p>Ședințe noi anunțate de Primăria Cluj-Napoca:</p><ul>");
    for (m, date) in meetings.iter().zip(&dates) {
        let ora = m.time.as_deref().map(|t| format!(", ora {t}")).unwrap_or_default();
        let proiecte = if m.item_count > 0 {
            format!("{} proiecte pe ordinea de zi", m.item_count)
        } else {
            "ordinea de zi încă nepublicată".to_owned()
        };
        let link = format!("{public_url}/sedinte/{}", m.id);
        text.push_str(&format!(
            "• Ședința din {date}{ora} — {proiecte}\n  {link}\n  Pagina primăriei: {}\n\n",
            m.url
        ));
        html.push_str(&format!(
            "<li><a href=\"{}\">Ședința din {}</a>{} — {}<br><a href=\"{}\">Pagina primăriei</a></li>",
            esc(&link),
            esc(date),
            esc(&ora),
            esc(&proiecte),
            esc(&m.url)
        ));
    }
    text.push_str(&format!(
        "Trimis de Monitor Urban ({public_url}), care urmărește site-ul primăriei."
    ));
    html.push_str(&format!(
        "</ul><p style=\"color:#666\">Trimis de <a href=\"{0}\">Monitor Urban</a>, care urmărește site-ul primăriei.</p>",
        esc(public_url)
    ));
    (subject, text, html)
}

/// Emailul cu linkul de autentificare.
pub fn render_login(url: &str) -> (String, String, String) {
    let subject = "Autentificare Monitor Urban".to_owned();
    let text = format!(
        "Apasă linkul ca să intri în contul tău Monitor Urban (valabil 15 minute, o singură dată):\n\n{url}\n\n\
         Dacă nu ai cerut tu acest email, ignoră-l."
    );
    let html = format!(
        "<p>Apasă linkul ca să intri în contul tău Monitor Urban (valabil 15 minute, o singură dată):</p>\
         <p><a href=\"{0}\">Intră în cont</a></p><p style=\"color:#666\">{0}<br>Dacă nu ai cerut tu acest email, ignoră-l.</p>",
        esc(url)
    );
    (subject, text, html)
}

fn item_line(i: &Item) -> String {
    let unde = i
        .address
        .as_deref()
        .or(i.street.as_deref())
        .map(|s| format!(" — {s}"))
        .unwrap_or_default();
    format!(
        "{} ({}){unde}, ședința din {}",
        i.title,
        i.category.label(),
        fmt_date_ro(i.meeting_date)
    )
}

/// Rezumatul pe cuvinte-cheie: proiectele noi, grupate pe cuvânt.
pub fn render_digest(public_url: &str, matches: &Matches) -> (String, String, String) {
    let total = matches.total();
    let words: Vec<&str> = matches.keywords.iter().map(|(k, _)| k.text.as_str()).collect();
    let subject = if total == 1 {
        format!("Proiect nou pentru „{}”", words[0])
    } else {
        format!(
            "{total} proiecte noi pentru {}",
            words.iter().map(|w| format!("„{w}”")).collect::<Vec<_>>().join(", ")
        )
    };
    let mut text = String::from("Proiecte noi pe ordinea de zi CTATU care se potrivesc cuvintelor tale cheie:\n\n");
    let mut html = String::from("<p>Proiecte noi pe ordinea de zi CTATU care se potrivesc cuvintelor tale cheie:</p>");
    for (k, items) in &matches.keywords {
        text.push_str(&format!("{}:\n", k.text));
        html.push_str(&format!("<h3>{}</h3><ul>", esc(&k.text)));
        for i in items {
            let meeting_link = format!("{public_url}/sedinte/{}", i.meeting_id);
            text.push_str(&format!(
                "• {}\n  {}\n  Proiectul pe site-ul primăriei: {}\n",
                item_line(i),
                meeting_link,
                i.url
            ));
            html.push_str(&format!(
                "<li>{}<br><a href=\"{}\">Ședința</a> · <a href=\"{}\">Proiectul pe site-ul primăriei</a></li>",
                esc(&item_line(i)),
                esc(&meeting_link),
                esc(&i.url)
            ));
        }
        text.push('\n');
        html.push_str("</ul>");
    }
    text.push_str(&format!(
        "Cuvintele-cheie le schimbi din contul tău: {public_url}/cont\nTrimis de Monitor Urban, care urmărește site-ul primăriei."
    ));
    html.push_str(&format!(
        "<p style=\"color:#666\">Cuvintele-cheie le schimbi din <a href=\"{0}/cont\">contul tău</a>.<br>Trimis de <a href=\"{0}\">Monitor Urban</a>, care urmărește site-ul primăriei.</p>",
        esc(public_url)
    ));
    (subject, text, html)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use urban_shared::Category;

    use super::*;
    use crate::accounts::Keyword;

    fn meeting(id: i64, day: u32, time: Option<&str>, items: i64) -> Meeting {
        Meeting {
            id,
            url: format!("https://primariaclujnapoca.ro/urbanism/sedinte-comisie/sedinta-{id}/"),
            title: String::new(),
            date: NaiveDate::from_ymd_opt(2026, 9, day).unwrap(),
            time: time.map(str::to_owned),
            agenda_url: None,
            conclusions_pdf_url: None,
            announcement_url: None,
            item_count: items,
            upcoming: true,
        }
    }

    #[test]
    fn o_sedinta() {
        let (subject, text, html) = render("https://urbanism.hopartean.com", &[meeting(7, 30, Some("12:00"), 11)]);
        assert_eq!(subject, "Ședință CTATU nouă: 30 septembrie 2026");
        assert!(text.contains("Ședința din 30 septembrie 2026, ora 12:00 — 11 proiecte pe ordinea de zi"));
        assert!(text.contains("https://urbanism.hopartean.com/sedinte/7"));
        assert!(html.contains("<a href=\"https://urbanism.hopartean.com/sedinte/7\">"));
    }

    #[test]
    fn mai_multe_sedinte_fara_ordine_de_zi() {
        let (subject, text, _) = render("https://x.ro", &[meeting(1, 28, None, 0), meeting(2, 30, None, 3)]);
        assert_eq!(subject, "2 ședințe CTATU noi: 28 septembrie 2026, 30 septembrie 2026");
        assert!(text.contains("Ședința din 28 septembrie 2026 — ordinea de zi încă nepublicată"));
    }

    #[test]
    fn login_si_rezumat() {
        let (_, text, html) = render_login("https://x.ro/cont/verifica?token=abc");
        assert!(text.contains("https://x.ro/cont/verifica?token=abc"));
        assert!(html.contains("href=\"https://x.ro/cont/verifica?token=abc\""));

        let item = Item {
            id: 5,
            meeting_id: 9,
            meeting_date: NaiveDate::from_ymd_opt(2026, 10, 7).unwrap(),
            meeting_url: "https://p/sedinta".into(),
            url: "https://p/proiect-5".into(),
            title: "PUZ str. Fabricii nr. 3".into(),
            address: Some("str. Fabricii nr. 3".into()),
            street: Some("Fabricii".into()),
            category: Category::Puz,
            published_at: None,
            beneficiary: None,
            reg_number: None,
            reg_date: None,
            revenire: false,
            documents: vec![],
        };
        let kw = Keyword {
            id: 1,
            user_id: 1,
            text: "Fabricii".into(),
            normalized: "fabricii".into(),
        };
        let m = Matches {
            keywords: vec![(kw, vec![item])],
        };
        let (subject, text, html) = render_digest("https://x.ro", &m);
        assert_eq!(subject, "Proiect nou pentru „Fabricii”");
        assert!(text.contains("PUZ str. Fabricii nr. 3 (PUZ) — str. Fabricii nr. 3, ședința din 7 octombrie 2026"));
        assert!(text.contains("https://x.ro/sedinte/9"));
        assert!(html.contains("<h3>Fabricii</h3>"));
    }
}
