//! Pagina unui proiect (sau a concluziilor) → documente atașate (FR-1.6, FR-1.7).

use std::collections::HashSet;

use scraper::{Html, Selector};

use super::{collapse_ws, resolve};
use urban_shared::Document;

const FILES_HOST: &str = "files.primariaclujnapoca.ro";
const NOISE: [&str; 3] = ["anti-mita", "sigla", "placeholder"];
const IMAGE_EXT: [&str; 6] = [".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp"];

fn is_document(url: &str) -> bool {
    let lower = url.to_lowercase();
    if NOISE.iter().any(|n| lower.contains(n)) || IMAGE_EXT.iter().any(|e| lower.ends_with(e)) {
        return false;
    }
    lower.contains(FILES_HOST)
        || [".pdf", ".doc", ".docx", ".zip", ".dwg", ".xls", ".xlsx"]
            .iter()
            .any(|e| lower.ends_with(e))
}

/// Documentele din conținutul paginii. Caută întâi în `article .post-content`; dacă pagina
/// nu are acea structură, ia toate linkurile către serverul de fișiere al primăriei.
pub fn parse_documents(html: &str, page_url: &str) -> Vec<Document> {
    let doc = Html::parse_document(html);
    let base = url::Url::parse(page_url).ok();
    let in_content = Selector::parse("article .post-content a[href]").expect("selector valid");
    let any = Selector::parse("a[href]").expect("selector valid");

    let mut out = collect(&doc, &in_content, base.as_ref());
    if out.is_empty() {
        out = collect(&doc, &any, base.as_ref());
    }
    out
}

fn collect(doc: &Html, selector: &Selector, base: Option<&url::Url>) -> Vec<Document> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for a in doc.select(selector) {
        let Some(href) = a.value().attr("href") else { continue };
        let url = match base {
            Some(b) => match resolve(b, href) {
                Some(u) => u,
                None => continue,
            },
            None => href.to_owned(),
        };
        if !is_document(&url) || !seen.insert(url.clone()) {
            continue;
        }
        let mut label = collapse_ws(&a.text().collect::<String>());
        if label.is_empty() {
            label = url.rsplit('/').find(|s| !s.is_empty()).unwrap_or("document").to_owned();
        }
        out.push(Document { label, url });
    }
    out
}

/// Primul PDF din listă, de exemplu concluziile ședinței.
pub fn first_pdf(docs: &[Document]) -> Option<&str> {
    docs.iter()
        .find(|d| d.url.to_lowercase().ends_with(".pdf"))
        .map(|d| d.url.as_str())
}
