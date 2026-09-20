//! Ordinea de zi: text extras din PDF → rânduri (FR-1.3) → potrivire cu cardurile (FR-1.4).
//!
//! Textul extras lipește uneori tokenii vecini: „10739800/24.08.2026Marian Ramona”.
//! Numărul de înregistrare are 6 cifre; numărul de ordine se validează ca precedentul + 1.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::scrape::Card;
use urban_shared::text::{normalize, tokens};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgendaRow {
    pub nr: Option<u32>,
    pub reg_number: Option<String>,
    pub reg_date: Option<String>,
    pub beneficiary: Option<String>,
    pub description: String,
    pub revenire: bool,
}

/// Început de rând: `nr reg/date rest` sau `nrreg/date rest`.
static ROW_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(\d+)(?:\s+(\d+))?\s*/\s*(\d{1,2}\.\d{1,2}\.\d{4})\s*(.*)$").expect("regex valid")
});

/// Cuvinte cu care începe, de regulă, denumirea lucrării. Ce e înainte e beneficiarul.
static DESC_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(studiu|sudiu|plan\s+urbanistic|p\.\s?u\.\s?[zd]\b|puz\b|pud\b|actualizare|reactualizare|aviz\b|elaborare|modificare|documenta[tțţ]ie|[îi]ntocmire|reglementare)",
    )
    .expect("regex valid")
});

static REVENIRE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\s,;(-]*revenire\s*(ctatu|c\.t\.a\.t\.u\.?)?[\s,;)-]*").expect("regex valid"));

const HEADER_MARKERS: [&str; 6] = [
    "romania",
    "primaria municipiului cluj napoca",
    "directia generala de urbanism",
    "serviciul strategii urbane",
    "calea motilor nr 3",
    "email strategiiurbane",
];

fn is_header_line(n: &str) -> bool {
    HEADER_MARKERS.iter().any(|m| n.starts_with(m))
}

fn is_footer_line(n: &str) -> bool {
    n.starts_with("primar")
        || n.contains("arhitect sef")
        || n.contains("digitally signed")
        || n.contains("semnat digital")
}

/// Un rând în curs de asamblare: antetul recunoscut de `ROW_START` plus liniile de continuare.
struct Block {
    nr: Option<u32>,
    reg_number: Option<String>,
    reg_date: Option<String>,
    parts: Vec<String>,
}

/// Rândurile tabelului din ordinea de zi.
pub fn parse_agenda(text: &str) -> Vec<AgendaRow> {
    let lines: Vec<&str> = text.lines().collect();
    // sărim antetul până la „Denumire lucrare”; dacă lipsește, pornim de la început
    let start = lines
        .iter()
        .position(|l| normalize(l).contains("denumire lucrare"))
        .map(|i| i + 1)
        .unwrap_or(0);

    let mut blocks: Vec<Block> = Vec::new();
    let mut expected_nr: u32 = 1;

    for line in &lines[start..] {
        let n = normalize(line);
        if n.is_empty() || is_header_line(&n) {
            continue;
        }
        if is_footer_line(&n) {
            break;
        }
        if let Some(c) = ROW_START.captures(line) {
            let (nr, reg) = split_nr_reg(&c[1], c.get(2).map(|m| m.as_str()), expected_nr);
            if let Some(nr) = nr {
                expected_nr = nr + 1;
            }
            let rest = c[4].trim().to_owned();
            let mut parts = Vec::new();
            if !rest.is_empty() {
                parts.push(rest);
            }
            blocks.push(Block {
                nr,
                reg_number: reg,
                reg_date: Some(c[3].to_owned()),
                parts,
            });
        } else if let Some(last) = blocks.last_mut() {
            last.parts.push(line.trim().to_owned());
        }
    }

    blocks
        .into_iter()
        .map(
            |Block {
                 nr,
                 reg_number,
                 reg_date,
                 parts,
             }| {
                let joined = parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
                let revenire = REVENIRE.is_match(&joined);
                let cleaned = REVENIRE.replace_all(&joined, " ").to_string();
                let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
                let (beneficiary, description) = split_beneficiary(&cleaned);
                AgendaRow {
                    nr,
                    reg_number,
                    reg_date,
                    beneficiary,
                    description,
                    revenire,
                }
            },
        )
        .collect()
}

/// `nr` și `reg` din grupurile regex-ului; când sunt lipite, desparte după regula celor 6 cifre.
fn split_nr_reg(first: &str, second: Option<&str>, expected: u32) -> (Option<u32>, Option<String>) {
    if let Some(reg) = second {
        return (first.parse().ok(), Some(reg.to_owned()));
    }
    let blob = first;
    let exp = expected.to_string();
    if blob.len() > exp.len() && blob.starts_with(&exp) && blob.len() - exp.len() >= 4 {
        return (Some(expected), Some(blob[exp.len()..].to_owned()));
    }
    if blob.len() >= 7 {
        let cut = blob.len() - 6;
        return (blob[..cut].parse().ok(), Some(blob[cut..].to_owned()));
    }
    if blob.len() >= 4 {
        return (None, Some(blob.to_owned()));
    }
    (blob.parse().ok(), None)
}

fn split_beneficiary(text: &str) -> (Option<String>, String) {
    let trim_chars: &[char] = &[' ', ',', ';', '-', '–', '.', ':'];
    match DESC_START.find(text) {
        Some(m) if m.start() > 0 => {
            let b = text[..m.start()].trim_matches(trim_chars).to_owned();
            let d = text[m.start()..].trim().to_owned();
            (if b.is_empty() { None } else { Some(b) }, d)
        }
        _ => (None, text.trim().to_owned()),
    }
}

const STOPWORDS: [&str; 78] = [
    "str",
    "strada",
    "nr",
    "de",
    "si",
    "pentru",
    "cu",
    "in",
    "la",
    "a",
    "pe",
    "din",
    "zona",
    "calea",
    "sud",
    "nord",
    "est",
    "vest",
    "latura",
    "estica",
    "vestica",
    "nordica",
    "sudica",
    "initiere",
    "elaborare",
    "elaboare",
    "studiu",
    "sudiu",
    "oportunitate",
    "puz",
    "pud",
    "p",
    "u",
    "z",
    "d",
    "construire",
    "imobil",
    "imobile",
    "locuinta",
    "locuinte",
    "unifamiliala",
    "unifamiliale",
    "semicolectiva",
    "colectiva",
    "colective",
    "mixt",
    "mixte",
    "functiuni",
    "ansamblu",
    "dezvoltare",
    "parcelare",
    "regim",
    "redus",
    "inaltime",
    "aviz",
    "actualizare",
    "urbanizare",
    "restructurare",
    "urbana",
    "amenajari",
    "exterioare",
    "desfiintare",
    "extindere",
    "etajare",
    "modificari",
    "interioare",
    "amenajare",
    "edicul",
    "parcare",
    "teren",
    "sc",
    "srl",
    "s",
    "r",
    "l",
    "activitati",
    "economice",
    "caracter",
];

fn is_stopword(t: &str) -> bool {
    STOPWORDS.contains(&t)
}

fn is_numeric(t: &str) -> bool {
    t.chars().all(|c| c.is_ascii_digit())
}

/// Ponderea tokenilor unui card: adresa cântărește mai mult decât titlul.
fn card_weights(card: &Card) -> HashMap<String, u32> {
    let mut w: HashMap<String, u32> = HashMap::new();
    for t in tokens(&card.title) {
        let weight = if t.len() <= 1 || is_stopword(&t) || is_numeric(&t) {
            0
        } else {
            1
        };
        w.entry(t).and_modify(|x| *x = (*x).max(weight)).or_insert(weight);
    }
    if let Some(addr) = &card.address {
        for t in tokens(addr) {
            let weight = if t.len() <= 1 || is_stopword(&t) {
                0
            } else if is_numeric(&t) || t.len() <= 3 {
                1
            } else {
                3
            };
            w.entry(t).and_modify(|x| *x = (*x).max(weight)).or_insert(weight);
        }
    }
    w
}

const MATCH_THRESHOLD: u32 = 3;

/// Pentru fiecare rând, indexul cardului potrivit, unu-la-unu, cel mai bun scor întâi.
pub fn match_rows(rows: &[AgendaRow], cards: &[Card]) -> Vec<Option<usize>> {
    let weights: Vec<HashMap<String, u32>> = cards.iter().map(card_weights).collect();
    let row_tokens: Vec<HashSet<String>> = rows
        .iter()
        .map(|r| {
            let mut s: HashSet<String> = tokens(&r.description).into_iter().collect();
            if let Some(b) = &r.beneficiary {
                s.extend(tokens(b));
            }
            s
        })
        .collect();

    let mut candidates: Vec<(u32, usize, usize)> = Vec::new();
    for (ri, rt) in row_tokens.iter().enumerate() {
        for (ci, w) in weights.iter().enumerate() {
            let score: u32 = rt.iter().filter_map(|t| w.get(t)).sum();
            if score >= MATCH_THRESHOLD {
                candidates.push((score, ri, ci));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let mut assigned = vec![None; rows.len()];
    let mut used_cards = HashSet::new();
    for (_, ri, ci) in candidates {
        if assigned[ri].is_none() && !used_cards.contains(&ci) {
            assigned[ri] = Some(ci);
            used_cards.insert(ci);
        }
    }
    assigned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_glued_numbers() {
        assert_eq!(split_nr_reg("1", Some("723388"), 1), (Some(1), Some("723388".into())));
        assert_eq!(split_nr_reg("10739800", None, 10), (Some(10), Some("739800".into())));
        assert_eq!(split_nr_reg("13749074", None, 13), (Some(13), Some("749074".into())));
        // fără așteptare potrivită, cade pe regula celor 6 cifre
        assert_eq!(split_nr_reg("2731330", None, 9), (Some(2), Some("731330".into())));
        // număr de înregistrare scurt, din ianuarie, cu numărul de ordine așteptat
        assert_eq!(split_nr_reg("35535", None, 3), (Some(3), Some("5535".into())));
    }

    #[test]
    fn parses_rows_from_text() {
        let text = "Nr. Nr. inregistrare Beneficiar Denumire lucrare\n\
                    1 723388/14.08.2026Paun Adrian si alții\nStudiu de oportunitate pentru inițiere P.U.Z,\nstr. Borhanciului – est\n\
                    \n4 731261/19.08.2026\nrevenire CTATUMarin Aurel\nP.U.D construire locuinta unifamiliala,\n str. Ceahlău nr. 60b\n\
                    7 736505/21.08.2026S.C. Scala Sapte S.R.L P.U.Z de restructurare urbana, str. Bobâlnei 62-64\n\
                    PRIMAR, ARHITECT ȘEF,\nEMIL BOC\n";
        let rows = parse_agenda(text);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].nr, Some(1));
        assert_eq!(rows[0].reg_number.as_deref(), Some("723388"));
        assert_eq!(rows[0].reg_date.as_deref(), Some("14.08.2026"));
        assert_eq!(rows[0].beneficiary.as_deref(), Some("Paun Adrian si alții"));
        assert!(rows[0].description.starts_with("Studiu de oportunitate"));
        assert!(rows[0].description.contains("Borhanciului"));
        assert!(rows[1].revenire);
        assert_eq!(rows[1].beneficiary.as_deref(), Some("Marin Aurel"));
        assert!(rows[1].description.starts_with("P.U.D construire"));
        assert_eq!(rows[2].beneficiary.as_deref(), Some("S.C. Scala Sapte S.R.L"));
        assert!(rows[2].description.contains("Bobâlnei 62-64"));
    }

    #[test]
    fn matches_rows_to_cards_by_address() {
        use crate::scrape::CardKind;
        let card = |title: &str, addr: &str| Card {
            kind: CardKind::Project,
            url: format!("u/{}", normalize(addr).replace(' ', "-")),
            title: title.into(),
            address: Some(addr.into()),
            published_at: None,
        };
        let cards = vec![
            card("P.U.D construire locuință semicolectivă", "str. Doinei nr. 95"),
            card("P.U.D construire locuință semicolectivă", "str. Doinei nr. 93"),
            card(
                "Studiu de oportunitate pentru inițiere PUZ",
                "str. Giordano Bruno nr. 41",
            ),
            card("P.U.Z construire locuintă unifamilială", "str. Trifoiului nr. 41"),
        ];
        let row = |d: &str| AgendaRow {
            description: d.into(),
            ..Default::default()
        };
        let rows = vec![
            row("PUD construire locuinta semicolectiva, str.Doinei 93"),
            row("Studiu de oportunitate pentru initiere elaborare P.U.Z str,. Giordano Bruno nr. 41"),
            row("P.U.Z construire locuinta unifamiliala str. Trifoiului nr. 41"),
            row("PUD construire locuinta semicolectiva, str.Doinei 95"),
            row("Ceva complet diferit, str. Inexistenta nr. 1"),
        ];
        let m = match_rows(&rows, &cards);
        assert_eq!(m, vec![Some(1), Some(2), Some(3), Some(0), None]);
    }
}
