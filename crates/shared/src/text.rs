//! Normalizare de text și euristici de domeniu. Fără regex, ca să rămână mic la wasm.

use chrono::NaiveDate;
use unicode_normalization::UnicodeNormalization;

use crate::model::Category;

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

const STREET_PREFIXES: [&str; 22] = [
    "strada",
    "str",
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
                    let skip = p.len() + rest.chars().take_while(|c| *c == '.' || *c == ' ').count();
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
}
