//! Lista certificatelor de urbanism emise → un rând per certificat (FR-10.1). Pagina e o arhivă
//! WordPress cu 24 de certificate, paginată prin `?sf_paged=N`, cele mai noi întâi; fiecare
//! `article` are titlul cu numărul, data emiterii, scopul și adresa lucrării. API-ul REST al
//! site-ului nu expune scopul și adresa, de aceea citim HTML-ul (ADR-0013).

use chrono::NaiveDate;
use scraper::{ElementRef, Html, Selector};

use super::{collapse_ws, resolve};
use urban_shared::text::parse_ro_date;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateRef {
    pub url: String,
    pub number: i64,
    pub year: i32,
    /// Data emiterii.
    pub date: NaiveDate,
    /// Scopul brut, cu spațiile strânse; gol când lipsește.
    pub scop: String,
    /// Adresa brută, cu prefixul administrativ; `None` când lipsește.
    pub address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CertificatesPage {
    pub certificates: Vec<CertificateRef>,
    /// Există linkul „Next”: mai sunt pagini după aceasta.
    pub has_next: bool,
}

/// URL-ul paginii `page` (de la 1) a listei: prima e lista însăși, următoarele au `?sf_paged=N`.
pub fn page_url(listing_url: &str, page: u32) -> String {
    if page <= 1 {
        listing_url.to_owned()
    } else {
        format!("{listing_url}?sf_paged={page}")
    }
}

/// Certificatele de pe o pagină a listei, în ordinea din pagină, și dacă mai urmează pagini.
pub fn parse_certificates(html: &str, base_url: &str) -> CertificatesPage {
    let doc = Html::parse_document(html);
    let sel_article = Selector::parse("article.type-certificat_urbanism").expect("selector valid");
    let sel_link = Selector::parse("h2.entry-title a[href]").expect("selector valid");
    let sel_updated = Selector::parse("span.updated").expect("selector valid");
    let sel_date_label = Selector::parse("div.date-label").expect("selector valid");
    let sel_scop = Selector::parse(".field-scop").expect("selector valid");
    let sel_addr = Selector::parse(".field-adresa_lucrare").expect("selector valid");
    let sel_next = Selector::parse("a.pagination-next").expect("selector valid");
    let base = url::Url::parse(base_url).ok();

    let mut certificates = Vec::new();
    for art in doc.select(&sel_article) {
        let Some(a) = art.select(&sel_link).next() else {
            continue;
        };
        let Some(href) = a.value().attr("href") else { continue };
        let url = match &base {
            Some(b) => match resolve(b, href) {
                Some(u) => u,
                None => continue,
            },
            None => href.to_owned(),
        };
        let title = collapse_ws(&a.text().collect::<String>());
        let Some((number, year)) = number_and_year(&title).or_else(|| number_and_year_from_slug(&url)) else {
            continue;
        };
        // `date-label` e data emiterii (ordinea listei); `span.updated` e ultima modificare, uneori mai târzie
        let date = text_of(&art, &sel_date_label)
            .and_then(|t| parse_ro_date(&t))
            .or_else(|| text_of(&art, &sel_updated).and_then(|t| iso_date(&t)));
        let Some(date) = date else { continue };
        certificates.push(CertificateRef {
            url,
            number,
            year,
            date,
            scop: text_of(&art, &sel_scop).unwrap_or_default(),
            address: text_of(&art, &sel_addr),
        });
    }
    CertificatesPage {
        certificates,
        has_next: doc.select(&sel_next).next().is_some(),
    }
}

fn text_of(el: &ElementRef<'_>, sel: &Selector) -> Option<String> {
    let t = collapse_ws(&el.select(sel).next()?.text().collect::<String>());
    (!t.is_empty()).then_some(t)
}

/// „Certificat de urbanism 1733/2026” → (1733, 2026)
fn number_and_year(title: &str) -> Option<(i64, i32)> {
    let (a, b) = title.rsplit_once('/')?;
    let number: i64 = a.trim_end().rsplit(' ').next()?.parse().ok()?;
    let year: i32 = leading_digits(b.trim())?;
    Some((number, year))
}

/// „…/certificat-de-urbanism-1733-din-2026/” → (1733, 2026)
fn number_and_year_from_slug(url: &str) -> Option<(i64, i32)> {
    let slug = url.trim_end_matches('/').rsplit('/').next()?;
    let (a, b) = slug.rsplit_once("-din-")?;
    let number: i64 = a.rsplit('-').next()?.parse().ok()?;
    let year: i32 = leading_digits(b)?;
    Some((number, year))
}

fn leading_digits<T: std::str::FromStr>(s: &str) -> Option<T> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// „2026-10-01T00:00:00+03:00” → 2026-10-01
fn iso_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim().get(..10)?, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_from_title_and_slug() {
        assert_eq!(number_and_year("Certificat de urbanism 1733/2026"), Some((1733, 2026)));
        assert_eq!(
            number_and_year("Certificat de urbanism 12/2024 (rectificat)"),
            Some((12, 2024))
        );
        assert_eq!(number_and_year("Certificat de urbanism"), None);
        assert_eq!(
            number_and_year_from_slug(
                "https://primariaclujnapoca.ro/urbanism/certificate-de-urbanism/certificat-de-urbanism-1733-din-2026/"
            ),
            Some((1733, 2026))
        );
        assert_eq!(number_and_year_from_slug("https://x/altceva/"), None);
        assert_eq!(
            iso_date("2026-10-01T00:00:00+03:00"),
            NaiveDate::from_ymd_opt(2026, 10, 1)
        );
        assert_eq!(iso_date("azi"), None);
    }

    #[test]
    fn page_urls() {
        let l = "https://x/documente-emise/";
        assert_eq!(page_url(l, 0), l);
        assert_eq!(page_url(l, 1), l);
        assert_eq!(page_url(l, 7), "https://x/documente-emise/?sf_paged=7");
    }

    #[test]
    fn minimal_article() {
        let html = r#"<main>
          <article class="post certificat_urbanism type-certificat_urbanism">
            <div class="date-label">30 septembrie 2026</div>
            <span class="updated rich-snippet-hidden">2026-10-02T00:00:09+03:00</span>
            <h2 class="entry-title"><a href="/urbanism/certificate-de-urbanism/certificat-de-urbanism-1732-din-2026/">Certificat de urbanism 1732/2026</a></h2>
            <div class="field-scop">  informare </div>
            <div class="field-adresa_lucrare"></div>
          </article>
          <div class="pagination"><span class="current">1</span></div></main>"#;
        let p = parse_certificates(
            html,
            "https://primariaclujnapoca.ro/urbanism/certificate-de-urbanism/documente-emise/",
        );
        assert_eq!(p.certificates.len(), 1);
        let c = &p.certificates[0];
        assert_eq!(
            c.url,
            "https://primariaclujnapoca.ro/urbanism/certificate-de-urbanism/certificat-de-urbanism-1732-din-2026/"
        );
        assert_eq!((c.number, c.year), (1732, 2026));
        assert_eq!(c.date, NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
        assert_eq!(c.scop, "informare");
        assert_eq!(c.address, None);
        assert!(!p.has_next);
    }
}
