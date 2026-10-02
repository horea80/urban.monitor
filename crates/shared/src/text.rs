//! Normalizare de text și euristici de domeniu. Fără regex, ca să rămână mic la wasm.

use chrono::NaiveDate;
use unicode_normalization::UnicodeNormalization;

use crate::model::{Category, CertificateKind};

/// Elimină diacriticele: NFKD + eliminarea semnelor combinate.
/// Acoperă ă â î ș ț cu virgulă și ş ţ cu sedilă.
pub fn strip_diacritics(s: &str) -> String {
    s.nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

/// Litere mici, fără diacritice, doar `[a-z0-9]` separate de exact un spațiu.
/// Aceeași funcție se folosește la indexare și la căutare.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for c in strip_diacritics(s).chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(c);
        } else {
            pending_space = true;
        }
    }
    out
}

pub fn tokens(s: &str) -> Vec<String> {
    normalize(s)
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Clasificarea unui proiect după titlu (FR-2.2).
pub fn classify(title: &str) -> Category {
    let n = normalize(title);
    if n.contains("oportunitate") {
        return Category::AvizOportunitate;
    }
    let toks: Vec<&str> = n.split(' ').collect();
    if has_abbrev(&toks, "puz") || n.contains("plan urbanistic zonal") {
        return Category::Puz;
    }
    if has_abbrev(&toks, "pud") || n.contains("plan urbanistic de detaliu") {
        return Category::Pud;
    }
    Category::Altele
}

/// „puz” ca token, sau „p u z” ca trei tokeni consecutivi (din „P.U.Z”).
fn has_abbrev(toks: &[&str], abbrev: &str) -> bool {
    if toks.contains(&abbrev) {
        return true;
    }
    let letters: Vec<String> = abbrev.chars().map(|c| c.to_string()).collect();
    toks.windows(letters.len())
        .any(|w| w.iter().zip(&letters).all(|(a, b)| *a == b))
}

const RO_MONTHS: [&str; 12] = [
    "ianuarie",
    "februarie",
    "martie",
    "aprilie",
    "mai",
    "iunie",
    "iulie",
    "august",
    "septembrie",
    "octombrie",
    "noiembrie",
    "decembrie",
];

/// „Ședința din 16 septembrie 2026” sau „sedinta-din-16-septembrie-2026” → 2026-09-16.
pub fn parse_ro_date(text: &str) -> Option<NaiveDate> {
    let toks = tokens(text);
    for w in toks.windows(3) {
        let (Ok(day), Ok(year)) = (w[0].parse::<u32>(), w[2].parse::<i32>()) else {
            continue;
        };
        if w[0].len() > 2 || w[2].len() != 4 {
            continue;
        }
        let Some(month) = RO_MONTHS.iter().position(|m| *m == w[1]) else {
            continue;
        };
        if let Some(d) = NaiveDate::from_ymd_opt(year, month as u32 + 1, day) {
            return Some(d);
        }
    }
    None
}

/// „Orele: 10.00” / „ora 10:30” → „10:00” / „10:30”.
pub fn parse_time(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let hour: &String = &chars[start..i].iter().collect();
            if hour.len() <= 2
                && matches!(chars.get(i), Some('.') | Some(':') | Some(','))
                && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit())
                && chars.get(i + 2).is_some_and(|c| c.is_ascii_digit())
            {
                let h: u32 = hour.parse().ok()?;
                let m: String = chars[i + 1..i + 3].iter().collect();
                let mv: u32 = m.parse().ok()?;
                if h < 24 && mv < 60 {
                    return Some(format!("{h:02}:{m}"));
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

const STREET_PREFIXES: [&str; 25] = [
    "strada",
    "strazii",
    "străzii",
    "str",
    "p-ța",
    "calea",
    "bulevardul",
    "b-dul",
    "bdul",
    "bd",
    "blvd",
    "aleea",
    "piata",
    "piața",
    "p-ta",
    "drumul",
    "drum",
    "splaiul",
    "soseaua",
    "șoseaua",
    "zona",
    "colonia",
    "cartierul",
    "cartier",
    "intrarea",
];

/// Numele străzii din adresă, best-effort, cu majusculele originale:
/// „str C-tin Brancusi nr 107-109” → „C-tin Brancusi”; „zona Triajului – sud” → „Triajului”.
pub fn street_of(address: &str) -> Option<String> {
    let mut s = address.trim();
    // taie prefixele de tip stradă, eventual mai multe („zona str. Beclean”)
    for _ in 0..3 {
        let lower = s.to_lowercase();
        let mut cut = false;
        for p in STREET_PREFIXES {
            if let Some(rest) = lower.strip_prefix(p) {
                let boundary =
                    rest.is_empty() || rest.starts_with('.') || rest.starts_with(' ') || rest.starts_with(',');
                if boundary {
                    // în caractere, nu în bytes: „străzii” și „piața” au diacritice
                    let skip = p.chars().count() + rest.chars().take_while(|c| *c == '.' || *c == ' ').count();
                    s = &s[byte_index(s, skip)..];
                    cut = true;
                    break;
                }
            }
        }
        if !cut {
            break;
        }
    }
    // taie la număr, „nr”, virgulă sau liniuță separată de spații
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() || c == ',' {
            end = i;
            break;
        }
        let rest = &s[i..];
        if rest.starts_with(" nr") || rest.starts_with(" - ") || rest.starts_with(" – ") || rest.starts_with(" — ")
        {
            end = i;
            break;
        }
    }
    let street = s[..end].trim().trim_end_matches(['.', ',', '-', '–']).trim();
    if street.is_empty() {
        None
    } else {
        Some(street.to_owned())
    }
}

/// Tipul unui certificat de urbanism după scopul declarat (FR-10.2). Scopul e text liber, cu
/// variante și greșeli („INFOMARE”), deci regulile merg pe textul normalizat, în ordinea: PUZ/PUD
/// cerute explicit (nu „conform PUZ aprobat”), lucrări de autorizat, operațiuni notariale sau
/// cadastrale, informare.
pub fn classify_scop(scop: &str) -> CertificateKind {
    const WORKS: [&str; 19] = [
        "autoriz",
        "construir",
        "construct",
        "desfiintar",
        "demolar",
        "documentatie tehnic",
        "documentatiei tehnic",
        "documentatii tehnic",
        "dtac",
        "dtad",
        "intrare in legalitate",
        "reabilitar",
        "refatadiz",
        "extinder",
        "mansardar",
        "studiu de fezabilitate",
        "proiect tehnic",
        "amenajar",
        "modificari interioare",
    ];
    const OPS: [&str; 6] = [
        "notarial",
        "cadastral",
        "dezmembrar",
        "alipir",
        "circulatia imobil",
        "intabular",
    ];
    let n = normalize(scop);
    let toks: Vec<&str> = n.split(' ').filter(|t| !t.is_empty()).collect();
    if n.contains("plan urbanistic zonal") || leads_with_abbrev(&toks, "puz") {
        return CertificateKind::Puz;
    }
    if n.contains("plan urbanistic de detaliu") || leads_with_abbrev(&toks, "pud") {
        return CertificateKind::Pud;
    }
    if WORKS.iter().any(|w| n.contains(w)) {
        return CertificateKind::Construire;
    }
    if OPS.iter().any(|w| n.contains(w)) {
        return CertificateKind::Operatiuni;
    }
    if n.contains("informare") || n.contains("infomare") {
        return CertificateKind::Informare;
    }
    CertificateKind::Altele
}

/// Abrevierea („puz”, sau literele ei din „P.U.Z”) printre primii tokeni, fără un „conform” sau
/// „aprobat” înaintea ei: „ELABORARE PUZ…” da, „INFORMARE CONFORM PUZ APROBAT…” nu.
fn leads_with_abbrev(toks: &[&str], abbrev: &str) -> bool {
    const STOP: [&str; 5] = ["conform", "cf", "potrivit", "informare", "aprobat"];
    let letters: Vec<String> = abbrev.chars().map(|c| c.to_string()).collect();
    for (i, w) in toks.iter().take(6).enumerate() {
        if STOP.contains(w) {
            return false;
        }
        if *w == abbrev {
            return true;
        }
        let end = i + letters.len();
        if end <= toks.len() && toks[i..end].iter().zip(&letters).all(|(a, b)| *a == b) {
            return true;
        }
    }
    false
}

/// Adresa unei lucrări dintr-un certificat de urbanism, descompusă (FR-10.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkAddress {
    /// Fără prefixul administrativ: „CĂPITAN GRIGORE IGNAT, nr. 28”; `None` când nu rămâne nimic.
    pub address: Option<String>,
    pub street: Option<String>,
    /// „28”, „107-109”, „31g”; `None` când lipsește sau e „FN” / „f.nr.”.
    pub street_no: Option<String>,
}

/// „judetul Cluj, municipiul Cluj-Napoca, Str Traian Vuia, nr. 246” → adresa „Str Traian Vuia, nr. 246”,
/// strada „Traian Vuia”, numărul „246”. Segmentele cu județul și municipiul se elimină; fără ele,
/// adresa lipsește cu totul (cam o treime din certificate).
pub fn parse_work_address(raw: &str) -> WorkAddress {
    let segs: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty() && !is_admin_segment(s))
        .collect();
    if segs.is_empty() {
        return WorkAddress::default();
    }
    let address = segs.join(", ");
    let (street_part, street_no) = match find_number_marker(&address) {
        Some((start, after)) => {
            let number: String = address[after..]
                .trim_start_matches(['.', ':', ' '])
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches('.')
                .to_owned();
            (address[..start].trim_end_matches([' ', ',', '.']).to_owned(), number)
        }
        None => {
            // „str. Plevnei 134”: ultimul cuvânt al primului segment, dacă începe cu o cifră
            let first = segs[0];
            let last = first.rsplit(' ').next().unwrap_or("");
            let number = if last.starts_with(|c: char| c.is_ascii_digit()) && last != first {
                last.to_owned()
            } else {
                String::new()
            };
            (first.to_owned(), number)
        }
    };
    let n = normalize(&street_no);
    let no_number = n.is_empty()
        || matches!(
            n.as_str(),
            "fn" | "f n" | "fnr" | "f nr" | "fm" | "fara numar" | "fara nr"
        );
    WorkAddress {
        address: Some(address),
        street: street_of(&street_part),
        street_no: (!no_number).then_some(street_no),
    }
}

/// „judetul Cluj”, „municipiul Cluj-Napoca”, „Cluj-Napoca”.
fn is_admin_segment(s: &str) -> bool {
    let n = normalize(s);
    [
        "judetul",
        "judet",
        "jud",
        "municipiul",
        "mun",
        "comuna",
        "orasul",
        "localitatea",
    ]
    .iter()
    .any(|p| n == *p || n.starts_with(&format!("{p} ")))
        || n == "cluj napoca"
        || n == "cluj"
}

/// Poziția markerului „nr” (început și indexul de după el), ca token: la început, după spațiu sau
/// virgulă, urmat de punct, spațiu, cifră sau sfârșit. „Henri” nu se potrivește.
fn find_number_marker(s: &str) -> Option<(usize, usize)> {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    for (k, &(i, c)) in chars.iter().enumerate() {
        if !matches!(c, 'n' | 'N') {
            continue;
        }
        let Some(&(_, c2)) = chars.get(k + 1) else { break };
        if !matches!(c2, 'r' | 'R') {
            continue;
        }
        let before_ok = k == 0 || matches!(chars[k - 1].1, ' ' | ',' | '.');
        let after = chars.get(k + 2).map(|&(j, _)| j).unwrap_or(s.len());
        let after_ok = chars
            .get(k + 2)
            .is_none_or(|&(_, c3)| matches!(c3, '.' | ' ' | ':' | ',') || c3.is_ascii_digit());
        if before_ok && after_ok {
            return Some((i, after));
        }
    }
    None
}

/// 26677 → „26 677”, cu spațiu ca separator de mii.
pub fn fmt_thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

/// Indexul în bytes al celui de-al `n`-lea caracter.
fn byte_index(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_diacritics_and_punctuation() {
        assert_eq!(normalize("str. Câmpului nr. 333"), "str campului nr 333");
        assert_eq!(normalize("Brâncuși / Moților"), "brancusi motilor");
        assert_eq!(normalize("Ş ţ ş Ţ"), "s t s t");
        assert_eq!(normalize("  P.U.Z  "), "p u z");
    }

    #[test]
    fn classify_titles() {
        assert_eq!(
            classify("Studiu de oportunitate pentru inițiere P.U.Z"),
            Category::AvizOportunitate
        );
        assert_eq!(
            classify("Studiu de oportunitate pentru initiere PUZ (Actualizare aviz de oportunitate nr. 130/2021)"),
            Category::AvizOportunitate
        );
        assert_eq!(classify("P.U.Z de restructurare urbană"), Category::Puz);
        assert_eq!(
            classify("PUZ parcelare și construire locuințe cu regim redus de înălțime"),
            Category::Puz
        );
        assert_eq!(classify("P.U.D construire imobil mixt"), Category::Pud);
        assert_eq!(
            classify("PUD desființare parcare și construire imobil mixt, amenajări exterioare"),
            Category::Pud
        );
        assert_eq!(classify("Concluziile ședinței"), Category::Altele);
        assert_eq!(classify("ANUNȚ PRIVIND DESFĂȘURAREA ȘEDINȚEI"), Category::Altele);
    }

    #[test]
    fn dates_and_times() {
        assert_eq!(
            parse_ro_date("Ședința din 16 septembrie 2026"),
            NaiveDate::from_ymd_opt(2026, 9, 16)
        );
        assert_eq!(
            parse_ro_date("sedinta-din-2-aprilie-2026"),
            NaiveDate::from_ymd_opt(2026, 4, 2)
        );
        assert_eq!(
            parse_ro_date("https://x/sedinta-din-15-decembrie-2025/"),
            NaiveDate::from_ymd_opt(2025, 12, 15)
        );
        assert_eq!(parse_ro_date("fără dată"), None);
        assert_eq!(parse_time("Orele: 10.00"), Some("10:00".into()));
        assert_eq!(parse_time("ora 10:30 va avea loc"), Some("10:30".into()));
        assert_eq!(parse_time("Orele: 9,30"), Some("09:30".into()));
        assert_eq!(parse_time("nimic"), None);
    }

    #[test]
    fn streets() {
        assert_eq!(
            street_of("str C-tin Brancusi nr 107-109").as_deref(),
            Some("C-tin Brancusi")
        );
        assert_eq!(street_of("str. Ceahlău nr. 60b").as_deref(), Some("Ceahlău"));
        assert_eq!(street_of("zona Triajului – sud").as_deref(), Some("Triajului"));
        assert_eq!(street_of("Calea Baciului nr. 1-3").as_deref(), Some("Baciului"));
        assert_eq!(street_of("str.Bărc II nr. 71").as_deref(), Some("Bărc II"));
        assert_eq!(street_of("str. Plevnei 134").as_deref(), Some("Plevnei"));
        assert_eq!(street_of("zona str. Beclean").as_deref(), Some("Beclean"));
        assert_eq!(street_of("Baile Someseni").as_deref(), Some("Baile Someseni"));
        assert_eq!(street_of("str Frunzisului - nord").as_deref(), Some("Frunzisului"));
        assert_eq!(
            street_of("str. Vidrei nr 6-8, str. Morii nr. 31g").as_deref(),
            Some("Vidrei")
        );
        assert_eq!(street_of(""), None);
    }

    #[test]
    fn classify_scopes_from_real_certificates() {
        use CertificateKind::*;
        assert_eq!(classify_scop("INFORMARE"), Informare);
        assert_eq!(classify_scop("informare"), Informare);
        assert_eq!(classify_scop("INFOMARE"), Informare);
        assert_eq!(classify_scop("ELABORARE PLAN URBANISTIC ZONAL"), Puz);
        assert_eq!(classify_scop(" ELABORARE P.U.Z. - PARCELARE"), Puz);
        assert_eq!(
            classify_scop("ELABORARE PLAN URBANISTIC DE DETALIU ȘI DOCUMENTAȚIE TEHNICĂ PENTRU AUTORIZARE"),
            Pud
        );
        assert_eq!(
            classify_scop(
                "ELABORARE DOCUMENTATIE TEHNICA PENTRU AUTORIZAREA EXECUTARII LUCRARILOR DE CONSTRUIRE IMOBIL \
                 DE LOCUINTE COLECTIVE MICI, IMPREJMUIRE, AMENAJARI EXTERIOARE, OPERATIUNI NOTARIALE SI INFORMARE"
            ),
            Construire
        );
        assert_eq!(
            classify_scop(
                "ELABORARE DOCUMENTAȚIE TEHNICĂ PENTRU INTRARE IN LEGALITATE CONSTRUIRE LOCUINȚĂ UNIFAMILIALĂ \
                 CONFORM P.U.Z. VALEA FÂNAȚELOR APROBAT CU H.C.L. NR. 519 DIN 15.12.2009"
            ),
            Construire
        );
        assert_eq!(
            classify_scop("ELABORARE DOCUMENTAłIE TEHNICĂ PENTRU AUTORIZAREA"),
            Construire
        );
        assert_eq!(
            classify_scop("ELABORARE STUDIU DE FEZABILITATE SI DOCUMENTATIE"),
            Construire
        );
        assert_eq!(classify_scop("AMENAJARE SPAȚIU VERDE"), Construire);
        assert_eq!(classify_scop("OPERAȚIUNI CADASTRALE"), Operatiuni);
        assert_eq!(classify_scop("OPERATIUNI NOTARIALE ALIPIRE PARCELE"), Operatiuni);
        assert_eq!(
            classify_scop("OPERAȚIUNI NOTARIALE - DEZMEMBRARE CONFORM PUD APROBAT"),
            Operatiuni
        );
        assert_eq!(classify_scop("INFORMARE CONFORM PUZ APROBAT CU HCL 5/2020"), Informare);
        assert_eq!(
            classify_scop("DEZVOLTAREA UNEI CAPACITĂȚI NOI DE PRODUCERE A ENERGIEI ELECTRICE"),
            Altele
        );
        assert_eq!(classify_scop(""), Altele);
    }

    #[test]
    fn work_addresses() {
        let a = parse_work_address("judetul Cluj, municipiul Cluj-Napoca, CĂPITAN GRIGORE IGNAT, nr. 28");
        assert_eq!(a.address.as_deref(), Some("CĂPITAN GRIGORE IGNAT, nr. 28"));
        assert_eq!(a.street.as_deref(), Some("CĂPITAN GRIGORE IGNAT"));
        assert_eq!(a.street_no.as_deref(), Some("28"));

        assert_eq!(
            parse_work_address("judetul Cluj, municipiul Cluj-Napoca"),
            WorkAddress::default()
        );
        assert_eq!(parse_work_address("  "), WorkAddress::default());

        let a = parse_work_address("judetul Cluj, municipiul Cluj-Napoca, Zona străzii Valea Chintăului");
        assert_eq!(a.street.as_deref(), Some("Valea Chintăului"));
        assert_eq!(a.street_no, None);

        let a = parse_work_address("judetul Cluj, municipiul Cluj-Napoca, Strada Mihai Românu, nr. FN");
        assert_eq!(a.street.as_deref(), Some("Mihai Românu"));
        assert_eq!(a.street_no, None);

        let a = parse_work_address("judetul Cluj, municipiul Cluj-Napoca, Zona str. Frunzisului, nr. f.nr.");
        assert_eq!(a.street.as_deref(), Some("Frunzisului"));
        assert_eq!(a.street_no, None);

        let a = parse_work_address("judetul CLUJ, municipiul CLUJ-NAPOCA, Str P-ța Ștefan cel Mare, nr. 20");
        assert_eq!(a.street.as_deref(), Some("Ștefan cel Mare"));
        assert_eq!(a.street_no.as_deref(), Some("20"));

        let a = parse_work_address("judetul Cluj, municipiul Cluj-Napoca, ZoNA COLONIA FAGET, nr. FM");
        assert_eq!(a.street.as_deref(), Some("FAGET"));
        assert_eq!(a.street_no, None);

        let a = parse_work_address("str. Morii nr. 31g");
        assert_eq!(a.street.as_deref(), Some("Morii"));
        assert_eq!(a.street_no.as_deref(), Some("31g"));

        let a = parse_work_address("str. Plevnei 134");
        assert_eq!(a.street.as_deref(), Some("Plevnei"));
        assert_eq!(a.street_no.as_deref(), Some("134"));

        let a = parse_work_address("judetul Cluj, municipiul Cluj-Napoca, Str Henri Barbusse, nr. 44-46");
        assert_eq!(a.street.as_deref(), Some("Henri Barbusse"));
        assert_eq!(a.street_no.as_deref(), Some("44-46"));
    }

    #[test]
    fn thousands() {
        assert_eq!(fmt_thousands(0), "0");
        assert_eq!(fmt_thousands(777), "777");
        assert_eq!(fmt_thousands(5393), "5 393");
        assert_eq!(fmt_thousands(26677), "26 677");
        assert_eq!(fmt_thousands(1234567), "1 234 567");
    }
}
