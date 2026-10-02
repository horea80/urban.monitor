//! Tipuri de domeniu și tipurile de răspuns ale API-ului. Serializabile, fără logică de I/O.

use std::fmt;
use std::str::FromStr;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Tipul inițiativei de urbanism (FR-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Category {
    /// „Studiu de oportunitate pentru inițiere PUZ” = aviz de oportunitate
    AvizOportunitate,
    Puz,
    Pud,
    #[default]
    Altele,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::AvizOportunitate,
        Category::Puz,
        Category::Pud,
        Category::Altele,
    ];

    /// Forma stocată în baza de date și folosită în query string.
    pub fn as_str(self) -> &'static str {
        match self {
            Category::AvizOportunitate => "AVIZ_OPORTUNITATE",
            Category::Puz => "PUZ",
            Category::Pud => "PUD",
            Category::Altele => "ALTELE",
        }
    }

    /// Eticheta afișată utilizatorului.
    pub fn label(self) -> &'static str {
        match self {
            Category::AvizOportunitate => "Aviz de oportunitate PUZ",
            Category::Puz => "PUZ",
            Category::Pud => "PUD",
            Category::Altele => "Altele",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Category {
    type Err = String;

    /// Acceptă forma stocată și câteva aliasuri de tastat: `aviz`, `puz`, `pud`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "AVIZ_OPORTUNITATE" | "AVIZ" | "OPORTUNITATE" => Ok(Category::AvizOportunitate),
            "PUZ" => Ok(Category::Puz),
            "PUD" => Ok(Category::Pud),
            "ALTELE" => Ok(Category::Altele),
            other => Err(format!("categorie necunoscută: {other}")),
        }
    }
}

/// Tipul unui certificat de urbanism, derivat din scopul declarat (FR-10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CertificateKind {
    /// Doar informare: nu urmează nicio lucrare.
    Informare,
    /// Autorizarea unor lucrări: construire, desființare, reabilitare, intrare în legalitate.
    Construire,
    Puz,
    Pud,
    /// Operațiuni notariale sau cadastrale: dezmembrare, alipire.
    Operatiuni,
    #[default]
    Altele,
}

impl CertificateKind {
    pub const ALL: [CertificateKind; 6] = [
        CertificateKind::Informare,
        CertificateKind::Construire,
        CertificateKind::Puz,
        CertificateKind::Pud,
        CertificateKind::Operatiuni,
        CertificateKind::Altele,
    ];

    /// Forma stocată în baza de date și folosită în query string.
    pub fn as_str(self) -> &'static str {
        match self {
            CertificateKind::Informare => "INFORMARE",
            CertificateKind::Construire => "CONSTRUIRE",
            CertificateKind::Puz => "PUZ",
            CertificateKind::Pud => "PUD",
            CertificateKind::Operatiuni => "OPERATIUNI",
            CertificateKind::Altele => "ALTELE",
        }
    }

    /// Eticheta afișată utilizatorului.
    pub fn label(self) -> &'static str {
        match self {
            CertificateKind::Informare => "Informare",
            CertificateKind::Construire => "Autorizare lucrări",
            CertificateKind::Puz => "PUZ",
            CertificateKind::Pud => "PUD",
            CertificateKind::Operatiuni => "Operațiuni notariale / cadastrale",
            CertificateKind::Altele => "Altele",
        }
    }
}

impl fmt::Display for CertificateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CertificateKind {
    type Err = String;

    /// Acceptă forma stocată și câteva aliasuri de tastat: `info`, `constr`, `puz`, `pud`, `notarial`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "INFORMARE" | "INFO" => Ok(CertificateKind::Informare),
            "CONSTRUIRE" | "CONSTR" | "AUTORIZARE" => Ok(CertificateKind::Construire),
            "PUZ" => Ok(CertificateKind::Puz),
            "PUD" => Ok(CertificateKind::Pud),
            "OPERATIUNI" | "NOTARIAL" | "CADASTRAL" => Ok(CertificateKind::Operatiuni),
            "ALTELE" => Ok(CertificateKind::Altele),
            other => Err(format!("tip de certificat necunoscut: {other}")),
        }
    }
}

/// Un certificat de urbanism emis, așa cum apare în lista primăriei (FR-10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Certificate {
    pub id: i64,
    /// Pagina certificatului pe site-ul primăriei.
    pub url: String,
    /// Numărul din „Certificat de urbanism 1733/2026”.
    pub number: i64,
    pub year: i32,
    /// Data emiterii, așa cum o publică primăria.
    pub date: NaiveDate,
    /// Scopul declarat, textul brut: „INFORMARE”, „ELABORARE PLAN URBANISTIC ZONAL”…
    pub scop: String,
    pub kind: CertificateKind,
    /// Adresa lucrării, fără prefixul cu județul și municipiul; `None` când lipsește.
    pub address: Option<String>,
    pub street: Option<String>,
    /// Numărul stradal, ca text („28”, „107-109”, „31g”); `None` când lipsește sau e „FN”.
    pub street_no: Option<String>,
    /// Suprafața terenului, în mp, de pe pagina certificatului (FR-10.6); doar la PUZ și PUD.
    #[serde(default)]
    pub surface_mp: Option<i64>,
    /// Codurile UTR din PUG, separate prin virgulă: „LC, ULC, UIs”.
    #[serde(default)]
    pub utr: Option<String>,
    /// Folosința actuală a terenului: „terenuri: arabil, livadă, drum”.
    #[serde(default)]
    pub land_use: Option<String>,
    /// Numărul cărții funciare, când primăria îl publică (la PUZ de obicei nu).
    #[serde(default)]
    pub cf: Option<String>,
    /// Numerele cadastrale, ca text: „314038, 314038-C1”.
    #[serde(default)]
    pub cadastral: Option<String>,
}

impl Certificate {
    /// „Certificat de urbanism 1733/2026”
    pub fn title(&self) -> String {
        format!("Certificat de urbanism {}/{}", self.number, self.year)
    }
}

/// Parametrii căutării în certificate (FR-10.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CertificateQuery {
    /// Text liber; fiecare cuvânt se potrivește pe început de cuvânt, fără diacritice.
    #[serde(default)]
    pub q: String,
    /// Gol = toate tipurile.
    #[serde(default)]
    pub kinds: Vec<CertificateKind>,
    pub year: Option<i32>,
    #[serde(default = "SearchQuery::default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

impl Default for CertificateQuery {
    /// Fără text și filtre, cu limita implicită (nu 0, care ar însemna un singur rezultat).
    fn default() -> Self {
        CertificateQuery {
            q: String::new(),
            kinds: Vec::new(),
            year: None,
            limit: SearchQuery::default_limit(),
            offset: 0,
        }
    }
}

impl CertificateQuery {
    pub fn clamped_limit(&self) -> u32 {
        self.limit.clamp(1, SearchQuery::MAX_LIMIT)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CertificateResult {
    /// Numărul total de certificate care se potrivesc, indiferent de paginare.
    pub total: u32,
    pub items: Vec<Certificate>,
}

/// O ședință a comisiei.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Meeting {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub date: NaiveDate,
    /// „10:00”
    pub time: Option<String>,
    pub agenda_url: Option<String>,
    pub conclusions_pdf_url: Option<String>,
    pub announcement_url: Option<String>,
    pub item_count: i64,
    /// Ședința e programată în viitor: data ei e după ziua curentă (calculat la citire, pe server).
    #[serde(default)]
    pub upcoming: bool,
}

/// Un document atașat unui proiect (parte scrisă, parte desenată, adresă).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Document {
    pub label: String,
    pub url: String,
}

/// Un proiect discutat într-o ședință, așa cum îl întoarce căutarea și API-ul.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Item {
    pub id: i64,
    pub meeting_id: i64,
    pub meeting_date: NaiveDate,
    pub meeting_url: String,
    /// Pagina proiectului pe site-ul primăriei.
    pub url: String,
    pub title: String,
    pub address: Option<String>,
    pub street: Option<String>,
    pub category: Category,
    /// Data publicării cardului pe site, ISO 8601, așa cum apare în sursă.
    pub published_at: Option<String>,
    pub beneficiary: Option<String>,
    pub reg_number: Option<String>,
    pub reg_date: Option<String>,
    /// „revenire CTATU”: proiect rediscutat.
    pub revenire: bool,
    pub documents: Vec<Document>,
}

/// Un rând din ordinea de zi care nu are un proiect publicat pe pagina ședinței (FR-3.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AgendaRowView {
    pub id: i64,
    pub meeting_id: i64,
    pub meeting_date: NaiveDate,
    pub meeting_url: String,
    /// Proiectul publicat de care a fost legat rândul, dacă potrivirea a reușit.
    pub item_id: Option<i64>,
    pub nr: Option<i64>,
    pub reg_number: Option<String>,
    pub reg_date: Option<String>,
    pub beneficiary: Option<String>,
    pub description: String,
    pub revenire: bool,
    pub category: Category,
}

/// Parametrii unei căutări (FR-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SearchQuery {
    /// Text liber; fiecare cuvânt se potrivește pe început de cuvânt, fără diacritice.
    #[serde(default)]
    pub q: String,
    /// Gol = toate categoriile.
    #[serde(default)]
    pub categories: Vec<Category>,
    pub year: Option<i32>,
    #[serde(default = "SearchQuery::default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

impl SearchQuery {
    pub const MAX_LIMIT: u32 = 200;

    pub fn default_limit() -> u32 {
        50
    }

    pub fn clamped_limit(&self) -> u32 {
        self.limit.clamp(1, Self::MAX_LIMIT)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SearchResult {
    /// Numărul total de proiecte care se potrivesc, indiferent de paginare.
    pub total: u32,
    pub items: Vec<Item>,
    /// Rânduri din ordinea de zi fără proiect publicat, care se potrivesc căutării.
    pub agenda_only: Vec<AgendaRowView>,
}

/// O ședință cu tot ce ține de ea (FR-4.2, API `/meetings/{id}`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MeetingDetail {
    pub meeting: Meeting,
    pub items: Vec<Item>,
    /// Toate rândurile din ordinea de zi, inclusiv cele legate de proiecte.
    pub agenda_rows: Vec<AgendaRowView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct StreetCount {
    pub street: String,
    pub count: i64,
}

/// O rulare a sincronizării (FR-7.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SyncRun {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub ok: bool,
    pub meetings_seen: i64,
    pub meetings_updated: i64,
    pub items_new: i64,
    /// Certificate de urbanism apărute în această rulare (FR-10).
    #[serde(default)]
    pub certificates_new: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Status {
    pub meetings: i64,
    pub items: i64,
    pub agenda_rows_unmatched: i64,
    /// Certificate de urbanism stocate (FR-10).
    #[serde(default)]
    pub certificates: i64,
    pub last_run: Option<SyncRun>,
}

/// Contul văzut din pagina /cont (FR-9): ce trebuie afișat, nimic din ce nu trebuie să ajungă în browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountView {
    pub email: String,
    pub plan: String,
    pub credits_balance: i64,
    pub credits_per_year: i64,
    pub cycle_end: NaiveDate,
    pub keywords: Vec<KeywordView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordView {
    pub id: i64,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_roundtrip() {
        for c in Category::ALL {
            assert_eq!(c.as_str().parse::<Category>().unwrap(), c);
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(json, format!("\"{}\"", c.as_str()));
            assert_eq!(serde_json::from_str::<Category>(&json).unwrap(), c);
        }
        assert_eq!("aviz".parse::<Category>().unwrap(), Category::AvizOportunitate);
        assert!("xyz".parse::<Category>().is_err());
    }

    #[test]
    fn certificate_kind_roundtrip() {
        for k in CertificateKind::ALL {
            assert_eq!(k.as_str().parse::<CertificateKind>().unwrap(), k);
            let json = serde_json::to_string(&k).unwrap();
            assert_eq!(json, format!("\"{}\"", k.as_str()));
            assert_eq!(serde_json::from_str::<CertificateKind>(&json).unwrap(), k);
        }
        assert_eq!(
            "notarial".parse::<CertificateKind>().unwrap(),
            CertificateKind::Operatiuni
        );
        assert!("xyz".parse::<CertificateKind>().is_err());
        let c = Certificate {
            number: 1733,
            year: 2026,
            ..Default::default()
        };
        assert_eq!(c.title(), "Certificat de urbanism 1733/2026");
    }

    #[test]
    fn search_query_defaults() {
        let q: SearchQuery = serde_json::from_str(r#"{"q":"brancusi"}"#).unwrap();
        assert_eq!(q.limit, 50);
        assert_eq!(q.offset, 0);
        assert!(q.categories.is_empty());
        let big = SearchQuery {
            limit: 10_000,
            ..Default::default()
        };
        assert_eq!(big.clamped_limit(), SearchQuery::MAX_LIMIT);
    }
}
