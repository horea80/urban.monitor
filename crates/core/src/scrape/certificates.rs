//! Lista certificatelor de urbanism emise → un rând per certificat (FR-10.1). Pagina e o arhivă
//! WordPress cu 24 de certificate, paginată prin `?sf_paged=N`, cele mai noi întâi; fiecare
//! `article` are titlul cu numărul, data emiterii, scopul și adresa lucrării. API-ul REST al
//! site-ului nu expune scopul și adresa, de aceea citim HTML-ul (ADR-0013).
//!
//! Pagina unui certificat (FR-10.6, ADR-0014) are în plus suprafața, codurile UTR, folosința
//! actuală, cartea funciară și numerele cadastrale, îngropate la începutul unor câmpuri de 8–30 mii
//! de caractere copiate din regulamentul PUG; `parse_certificate_detail` ia doar prefixele specifice.

use chrono::NaiveDate;
use scraper::{ElementRef, Html, Selector};

use super::{collapse_ws, resolve};
use urban_shared::text::{parse_ro_date, strip_diacritics};

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

/// Câmpurile specifice de pe pagina unui certificat (FR-10.6): doar partea cu informație, fără
/// textul de regulament din PUG.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CertificateDetail {
    /// „S = 5393 mp” → 5393; `None` când lipsește (la jumătate din PUZ-uri).
    pub surface_mp: Option<i64>,
    /// Codurile UTR din PUG („LC”, „ULC”, „RrM2”), fără dubluri, în ordinea apariției.
    pub utr: Vec<String>,
    /// „Folosință actuală: terenuri (arabil)” → „terenuri (arabil)”.
    pub land_use: Option<String>,
    /// Numărul cărții funciare; `None` când e „-”.
    pub cf: Option<String>,
    /// Numerele cadastrale, ca text: „314038, 314038-C1”; `None` când e „-, identificat prin plan…”.
    pub cadastral: Option<String>,
}

/// De aici începe textul copiat din regulamentul PUG, în „regimul tehnic” și „regimul economic”.
const BOILERPLATE_MARKERS: [&str; 3] = ["SECŢIUNEA", "SECȚIUNEA", "SECTIUNEA"];

pub fn parse_certificate_detail(html: &str) -> CertificateDetail {
    let doc = Html::parse_document(html);
    let root = doc.root_element();
    let sel = |s: &str| Selector::parse(s).expect("selector valid");
    let tehnic = text_of(&root, &sel(".field-regimtehnic")).unwrap_or_default();
    let economic = text_of(&root, &sel(".field-regimeconomic")).unwrap_or_default();
    let cf = text_of(&root, &sel(".field-nrfisacarte")).unwrap_or_default();
    let cadastral = text_of(&root, &sel(".field-nrcadastral")).unwrap_or_default();

    let tehnic_prefix = specific_prefix(&tehnic);
    let economic_prefix = specific_prefix(&economic);
    let mut utr = utr_codes_from_prefix(tehnic_prefix);
    for code in utr_codes_from_text(economic_prefix) {
        push_unique(&mut utr, code);
    }
    CertificateDetail {
        surface_mp: parse_surface(tehnic_prefix).or_else(|| parse_surface(economic_prefix)),
        utr,
        land_use: parse_land_use(economic_prefix),
        cf: after_label(&cf),
        cadastral: after_label(&cadastral),
    }
}

/// Partea dinaintea textului de regulament, sau tot textul (cel mult 600 de caractere) dacă
/// marcajul lipsește.
fn specific_prefix(s: &str) -> &str {
    let cut = BOILERPLATE_MARKERS
        .iter()
        .filter_map(|m| s.find(m))
        .min()
        .unwrap_or_else(|| s.char_indices().nth(600).map(|(i, _)| i).unwrap_or(s.len()));
    s[..cut].trim()
}

/// „S = 5393 mp”, „S= 23 434 mp”, „S=394mp”, „S=1.234,5 mp” → partea întreagă, în mp.
fn parse_surface(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    for i in 0..b.len() {
        if !(b[i] == b'S' || b[i] == b's') || (i > 0 && b[i - 1].is_ascii_alphanumeric()) {
            continue;
        }
        let mut j = i + 1;
        while j < b.len() && b[j] == b' ' {
            j += 1;
        }
        if j >= b.len() || b[j] != b'=' {
            continue;
        }
        j += 1;
        while j < b.len() && b[j] == b' ' {
            j += 1;
        }
        let start = j;
        while j < b.len() && (b[j].is_ascii_digit() || matches!(b[j], b' ' | b'.' | b',')) {
            j += 1;
        }
        let mut k = j;
        while k < b.len() && b[k] == b' ' {
            k += 1;
        }
        // doar bytes ASCII între `start` și `k`, deci `k` e la limită de caracter
        let unit = s[k..].to_lowercase();
        if !(unit.starts_with("mp") || unit.starts_with("m2") || unit.starts_with("m²") || unit.starts_with("m.p")) {
            continue;
        }
        let int_part = s[start..j].split(',').next().unwrap_or("");
        let digits: String = int_part.chars().filter(char::is_ascii_digit).collect();
        if let Ok(v) = digits.parse::<i64>()
            && v > 0
        {
            return Some(v);
        }
    }
    None
}

/// Un token care arată a cod UTR: litere, cifre, `_`, între 2 și 8 caractere, începe cu majusculă,
/// nu e doar cifre și nu e o construcție („C1”).
fn looks_like_code(t: &str) -> bool {
    let len = t.chars().count();
    (2..=8).contains(&len)
        && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && t.starts_with(|c: char| c.is_ascii_uppercase())
        && !t.chars().all(|c| c.is_ascii_digit())
        && !(t.starts_with('C') && t[1..].chars().all(|c| c.is_ascii_digit()))
}

/// Codurile din prefixul „regimului tehnic”: ce rămâne după suprafață, ex. „S = 5393 mp EM” → „EM”.
fn utr_codes_from_prefix(prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    for t in prefix.split_whitespace() {
        let t = t.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if looks_like_code(t) && !t.eq_ignore_ascii_case("mp") {
            push_unique(&mut out, t.to_owned());
        }
    }
    out
}

/// Codurile din textul „regimului economic”: după „UTR=” sau „UTR ”, cele cu cifră sau `_`
/// („ULi_c”, „UM3”, „S_RIM”) și cele de forma „U” + majusculă + minuscule („UIs”, „UVa”, „ULc”).
fn utr_codes_from_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev_is_utr = false;
    for raw in text.split_whitespace() {
        let t = raw.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '=');
        let (explicit, code) = match t.strip_prefix("UTR=") {
            Some(rest) => (true, rest.trim_matches('=')),
            None => (prev_is_utr, t.trim_matches('=')),
        };
        prev_is_utr = t.eq_ignore_ascii_case("UTR") || t.eq_ignore_ascii_case("UTR=");
        if !looks_like_code(code) {
            continue;
        }
        let has_mark = code.chars().any(|c| c.is_ascii_digit() || c == '_');
        let u_mixed = code.len() >= 3
            && code.starts_with('U')
            && code[1..].starts_with(|c: char| c.is_ascii_uppercase())
            && code.chars().any(|c| c.is_ascii_lowercase());
        if explicit || has_mark || u_mixed {
            push_unique(&mut out, code.to_owned());
        }
    }
    out
}

fn push_unique(out: &mut Vec<String>, code: String) {
    if !out.iter().any(|c| c.eq_ignore_ascii_case(&code)) {
        out.push(code);
    }
}

/// Un caracter pe caracter: minuscule, fără diacritice, ca indicii să rămână aceiași.
fn fold_chars(s: &str) -> Vec<char> {
    s.chars()
        .map(|c| {
            let base = strip_diacritics(&c.to_string()).chars().next().unwrap_or(c);
            base.to_lowercase().next().unwrap_or(base)
        })
        .collect()
}

fn find_chars(hay: &[char], needle: &str, from: usize) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || hay.len() < n.len() || from > hay.len() - n.len() {
        return None;
    }
    (from..=hay.len() - n.len()).find(|&i| hay[i..i + n.len()] == n[..])
}

/// „Folosință actuală: terenuri (arabil). Destinația: …” → „terenuri (arabil)”.
fn parse_land_use(economic: &str) -> Option<String> {
    let orig: Vec<char> = economic.chars().collect();
    let folded = fold_chars(economic);
    let start = find_chars(&folded, "folosin", 0)?;
    let label_end = (start..orig.len().min(start + 30))
        .find(|&i| orig[i] == ':' || orig[i] == '-')
        .map(|i| i + 1)
        .unwrap_or(start);
    let end = find_chars(&folded, "destina", label_end).unwrap_or(orig.len());
    let text: String = orig[label_end..end].iter().collect();
    let text = text.trim().trim_end_matches(['.', ';', ',', '-', '–']).trim();
    (!text.is_empty()).then(|| text.chars().take(300).collect())
}

/// „Carte funciară: 314038” → „314038”; „Nr Cadastral: -, identificat prin plan…” → `None`.
fn after_label(raw: &str) -> Option<String> {
    let value = raw.split_once(':').map(|(_, v)| v).unwrap_or("").trim();
    if value.is_empty() || value.starts_with('-') {
        return None;
    }
    Some(value.chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfaces() {
        assert_eq!(parse_surface("S = 5393 mp EM"), Some(5393));
        assert_eq!(parse_surface("S= 23 434 mp TF"), Some(23434));
        assert_eq!(parse_surface("S=394mp RRM2"), Some(394));
        assert_eq!(parse_surface("S=1.234,50 mp"), Some(1234));
        assert_eq!(parse_surface("UTR=ULC, S_Va – scuar"), None);
        assert_eq!(parse_surface("VPR"), None);
        assert_eq!(parse_surface(""), None);
    }

    #[test]
    fn utr_codes() {
        assert_eq!(utr_codes_from_prefix("S = 5393 mp EM"), vec!["EM"]);
        assert_eq!(utr_codes_from_prefix("S=394mp RRM2"), vec!["RRM2"]);
        assert_eq!(utr_codes_from_prefix("VPR"), vec!["VPR"]);
        assert!(utr_codes_from_prefix("").is_empty());
        let t = "Folosință actuală: terenuri (arabil). Destinația: - parțial în ULi_c, zonă de urbanizare, \
                 - parțial în UM3, zonă mixtă, - parțial în VPr; UTR=ULC, Zonă; UTR=Lc, UTR Em; construcția C1 - Casă, \
                 S_RIM, UIs, UNELE, Imobil, Nu";
        assert_eq!(
            utr_codes_from_text(t),
            vec!["ULi_c", "UM3", "ULC", "Lc", "Em", "S_RIM", "UIs"]
        );
    }

    #[test]
    fn land_use_and_labels() {
        assert_eq!(
            parse_land_use("Folosință actuală: terenuri (arabil). Destinația: - parțial în ULi_c").as_deref(),
            Some("terenuri (arabil)")
        );
        assert_eq!(
            parse_land_use("Folosință actuală:teren (curți construcții) cu imprejmuire Destinația zonei: Lip")
                .as_deref(),
            Some("teren (curți construcții) cu imprejmuire")
        );
        assert_eq!(parse_land_use("Folosinţa actuală  - "), None);
        assert_eq!(parse_land_use("altceva"), None);
        assert_eq!(after_label("Carte funciară: 314038").as_deref(), Some("314038"));
        assert_eq!(
            after_label("Nr Cadastral: 314038, 314038-C1, 314038-C2").as_deref(),
            Some("314038, 314038-C1, 314038-C2")
        );
        assert_eq!(after_label("Nr Cadastral: -, identificat prin plan de încadrare"), None);
        assert_eq!(after_label("Carte funciară: -"), None);
        assert_eq!(after_label(""), None);
    }

    #[test]
    fn detail_from_minimal_page() {
        let html = r#"<main>
          <div class="field-nrfisacarte">Carte funciară: 299486</div>
          <div class="field-nrcadastral">Nr Cadastral: 299486, 299486-C1</div>
          <div class="field-regimeconomic">Folosință actuală: teren (curți construcții), construcția C1 - Casă; Destinația: RrM1, PARCELAR RIVERAN Încadrare…</div>
          <div class="field-regimtehnic">S=608 mp RRM1 SECŢIUNEA 3. CONDIŢII DE AMPLASARE … S = 999 mp</div>
        </main>"#;
        let d = parse_certificate_detail(html);
        assert_eq!(d.surface_mp, Some(608));
        assert_eq!(d.utr, vec!["RRM1"]);
        assert_eq!(
            d.land_use.as_deref(),
            Some("teren (curți construcții), construcția C1 - Casă")
        );
        assert_eq!(d.cf.as_deref(), Some("299486"));
        assert_eq!(d.cadastral.as_deref(), Some("299486, 299486-C1"));
        assert_eq!(parse_certificate_detail("<p>nimic</p>"), CertificateDetail::default());
    }

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
