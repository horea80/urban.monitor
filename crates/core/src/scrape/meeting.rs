//! Pagina unei ședințe → antet (dată, oră, PDF ordine de zi) și carduri (FR-1.2).

use chrono::NaiveDate;
use scraper::{ElementRef, Html, Selector};

use super::{collapse_ws, resolve};
use urban_shared::text::{normalize, parse_ro_date, parse_time};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// Un proiect de urbanism.
    Project,
    /// Cardul „Concluziile ședinței”; pagina lui conține PDF-ul cu concluziile.
    Conclusions,
    /// Cardul „Anunț privind desfășurarea ședinței”.
    Announcement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub kind: CardKind,
    pub url: String,
    pub title: String,
    pub address: Option<String>,
    /// ISO 8601 așa cum apare în `span.updated`.
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingPage {
    pub title: String,
    pub date: Option<NaiveDate>,
    pub time: Option<String>,
    pub agenda_url: Option<String>,
    pub cards: Vec<Card>,
}

impl MeetingPage {
    pub fn projects(&self) -> impl Iterator<Item = &Card> {
        self.cards.iter().filter(|c| c.kind == CardKind::Project)
    }

    pub fn conclusions(&self) -> Option<&Card> {
        self.cards.iter().find(|c| c.kind == CardKind::Conclusions)
    }

    pub fn announcement(&self) -> Option<&Card> {
        self.cards.iter().find(|c| c.kind == CardKind::Announcement)
    }
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("selector valid")
}

pub fn parse_meeting(html: &str, page_url: &str) -> MeetingPage {
    let doc = Html::parse_document(html);
    let base = url::Url::parse(page_url).ok();
    let resolve_href = |href: &str| -> Option<String> {
        match &base {
            Some(b) => resolve(b, href),
            None => Some(href.to_owned()),
        }
    };

    let title = doc
        .select(&sel("h1.entry-title"))
        .next()
        .map(|h| collapse_ws(&h.text().collect::<String>()))
        .unwrap_or_default();
    let date = parse_ro_date(&title).or_else(|| parse_ro_date(page_url));

    let time = doc
        .select(&sel("h3"))
        .map(|h| collapse_ws(&h.text().collect::<String>()))
        .find(|t| {
            let n = normalize(t);
            n.starts_with("orele") || n.starts_with("ora ")
        })
        .and_then(|t| parse_time(&t));

    let agenda_url = doc
        .select(&sel("a.sedinta-urbanism-content[href]"))
        .next()
        .or_else(|| {
            doc.select(&sel("a[href]"))
                .find(|a| normalize(&a.text().collect::<String>()) == "vezi document oficial")
        })
        .and_then(|a| a.value().attr("href"))
        .and_then(resolve_href);

    let cards = doc
        .select(&sel("article.proiect_urbanism"))
        .filter_map(|art| parse_card(art, &resolve_href))
        .collect();

    MeetingPage {
        title,
        date,
        time,
        agenda_url,
        cards,
    }
}

fn parse_card(art: ElementRef<'_>, resolve_href: &dyn Fn(&str) -> Option<String>) -> Option<Card> {
    let link = art.select(&sel("h2.entry-title a[href]")).next()?;
    let url = resolve_href(link.value().attr("href")?)?;
    let title = collapse_ws(&link.text().collect::<String>());
    if title.is_empty() {
        return None;
    }
    let address = art
        .select(&sel(".fusion-post-content-container"))
        .next()
        .map(|c| collapse_ws(&c.text().collect::<String>()))
        .filter(|s| !s.is_empty());
    let published_at = art
        .select(&sel("span.updated"))
        .next()
        .map(|s| collapse_ws(&s.text().collect::<String>()))
        .filter(|s| !s.is_empty());

    let n = normalize(&title);
    let kind = if n.starts_with("concluziile sedintei") {
        CardKind::Conclusions
    } else if n.contains("anunt privind desfasurarea") {
        CardKind::Announcement
    } else {
        CardKind::Project
    };

    Some(Card {
        kind,
        url,
        title,
        address,
        published_at,
    })
}
