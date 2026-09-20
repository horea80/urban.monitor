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
    pub const ALL: [Category; 4] = [Category::AvizOportunitate, Category::Puz, Category::Pud, Category::Altele];

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
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Status {
    pub meetings: i64,
    pub items: i64,
    pub agenda_rows_unmatched: i64,
    pub last_run: Option<SyncRun>,
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
    fn search_query_defaults() {
        let q: SearchQuery = serde_json::from_str(r#"{"q":"brancusi"}"#).unwrap();
        assert_eq!(q.limit, 50);
        assert_eq!(q.offset, 0);
        assert!(q.categories.is_empty());
        let big = SearchQuery { limit: 10_000, ..Default::default() };
        assert_eq!(big.clamped_limit(), SearchQuery::MAX_LIMIT);
    }
}
