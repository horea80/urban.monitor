//! Pagina cu lista ședințelor → referințe către paginile ședințelor (FR-1.1).

use std::collections::HashSet;

use chrono::NaiveDate;
use scraper::{Html, Selector};

use super::{collapse_ws, resolve};
use urban_shared::text::parse_ro_date;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetingRef {
    pub url: String,
    pub title: String,
    pub date: Option<NaiveDate>,
}

const MEETING_PATH_MARKER: &str = "/urbanism/sedinte-comisie/sedinta-din-";

/// Toate linkurile către ședințe, în ordinea din pagină, fără duplicate.
pub fn parse_listing(html: &str, base_url: &str) -> Vec<MeetingRef> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("a[href]").expect("selector valid");
    let base = url::Url::parse(base_url).ok();
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for a in doc.select(&sel) {
        let Some(href) = a.value().attr("href") else { continue };
        if !href.contains(MEETING_PATH_MARKER) {
            continue;
        }
        let url = match &base {
            Some(b) => match resolve(b, href) {
                Some(u) => u,
                None => continue,
            },
            None => href.to_owned(),
        };
        if !seen.insert(url.clone()) {
            continue;
        }
        let title = collapse_ws(&a.text().collect::<String>());
        let date = parse_ro_date(&title).or_else(|| parse_ro_date(&url));
        out.push(MeetingRef { url, title, date });
    }
    out
}
