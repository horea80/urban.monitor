//! Starea căutării în query string (FR-4.1): `?q=brancusi&tip=PUZ,PUD&an=2026&de_la=50`.
//! Același tip e ruta paginii principale, e parsat din formularul clasic (fără wasm) și e
//! transformat în `SearchQuery` pentru bibliotecă.

use std::fmt;

use urban_shared::{Category, SearchQuery};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchParams {
    pub q: String,
    pub tip: Vec<Category>,
    pub an: Option<i32>,
    /// Decalajul paginii („de la”), multiplu de `PAGE`.
    pub offset: u32,
}

impl SearchParams {
    pub const PAGE: u32 = 50;

    /// Fără text și fără filtre: pagina arată cele mai recente proiecte.
    pub fn is_blank(&self) -> bool {
        self.q.trim().is_empty() && self.tip.is_empty() && self.an.is_none()
    }

    pub fn has_category(&self, c: Category) -> bool {
        self.tip.contains(&c)
    }

    pub fn with_offset(&self, offset: u32) -> Self {
        SearchParams { offset, ..self.clone() }
    }

    /// Căutarea unui text, fără filtre (linkul „vezi pe site” al unui cuvânt-cheie).
    pub fn for_query(q: &str) -> Self {
        SearchParams {
            q: q.to_owned(),
            ..Default::default()
        }
    }

    pub fn only_category(c: Category) -> Self {
        SearchParams {
            tip: vec![c],
            ..Default::default()
        }
    }

    pub fn to_query(&self) -> SearchQuery {
        SearchQuery {
            q: self.q.trim().to_owned(),
            categories: self.tip.clone(),
            year: self.an,
            limit: Self::PAGE,
            offset: self.offset,
        }
    }

    /// `PUZ,PUD` sau `puz` → categorii; ce nu se recunoaște e ignorat.
    pub fn parse_categories(s: &str) -> impl Iterator<Item = Category> + '_ {
        s.split(',').filter_map(|t| t.trim().parse().ok())
    }
}

impl From<&str> for SearchParams {
    /// Acceptă și `tip=PUZ&tip=PUD` (formularul clasic), și `tip=PUZ,PUD` (linkuri).
    fn from(qs: &str) -> Self {
        let mut p = SearchParams::default();
        for (k, v) in form_urlencoded::parse(qs.trim_start_matches('?').as_bytes()) {
            match &*k {
                "q" => p.q = v.trim().to_owned(),
                "tip" => p.tip.extend(Self::parse_categories(&v)),
                "an" => p.an = v.trim().parse().ok(),
                "de_la" | "offset" => p.offset = v.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
        // ordinea din `Category::ALL`, fără dubluri
        p.tip = Category::ALL.into_iter().filter(|c| p.tip.contains(c)).collect();
        p
    }
}

impl fmt::Display for SearchParams {
    /// Doar câmpurile ne-goale, ca linkurile să rămână scurte și partajabile.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        let q = self.q.trim();
        if !q.is_empty() {
            let encoded: String = form_urlencoded::byte_serialize(q.as_bytes()).collect();
            parts.push(format!("q={encoded}"));
        }
        if !self.tip.is_empty() {
            let tips: Vec<&str> = self.tip.iter().map(|c| c.as_str()).collect();
            parts.push(format!("tip={}", tips.join(",")));
        }
        if let Some(an) = self.an {
            parts.push(format!("an={an}"));
        }
        if self.offset > 0 {
            parts.push(format!("de_la={}", self.offset));
        }
        f.write_str(&parts.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_form_and_link_shapes() {
        let p = SearchParams::from("q=br%C3%A2ncu%C8%99i+107&tip=PUD&tip=PUZ&an=2026&de_la=50");
        assert_eq!(p.q, "brâncuși 107");
        assert_eq!(p.tip, vec![Category::Puz, Category::Pud]);
        assert_eq!(p.an, Some(2026));
        assert_eq!(p.offset, 50);

        let p = SearchParams::from("?tip=puz,pud,xyz");
        assert_eq!(p.tip, vec![Category::Puz, Category::Pud]);
        assert!(p.q.is_empty());
    }

    #[test]
    fn display_roundtrips_and_omits_empty_fields() {
        let p = SearchParams {
            q: "Câmpului 12".into(),
            tip: vec![Category::Pud],
            an: None,
            offset: 0,
        };
        let s = p.to_string();
        assert_eq!(s, "q=C%C3%A2mpului+12&tip=PUD");
        assert_eq!(SearchParams::from(s.as_str()), p);
        assert_eq!(SearchParams::default().to_string(), "");
    }
}

/// Mesajele paginii /cont, aduse înapoi de rutele contului prin query string (FR-9).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AccountParams {
    pub ok: String,
    pub eroare: String,
}

impl AccountParams {
    pub fn ok_message(&self) -> Option<&'static str> {
        match self.ok.as_str() {
            "trimis" => {
                Some("Dacă adresa e corectă, ai primit un email cu linkul de autentificare. E valabil 15 minute.")
            }
            "adaugat" => {
                Some("Cuvântul-cheie a fost adăugat; primești un email la fiecare proiect nou care se potrivește.")
            }
            "sters" => Some("Contul a fost șters."),
            _ => None,
        }
    }
}

impl From<&str> for AccountParams {
    fn from(qs: &str) -> Self {
        let mut p = AccountParams::default();
        for (k, v) in form_urlencoded::parse(qs.trim_start_matches('?').as_bytes()) {
            match &*k {
                "ok" => p.ok = v.trim().to_owned(),
                "eroare" => p.eroare = v.trim().chars().take(200).collect(),
                _ => {}
            }
        }
        p
    }
}

impl fmt::Display for AccountParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut qs = form_urlencoded::Serializer::new(String::new());
        if !self.ok.is_empty() {
            qs.append_pair("ok", &self.ok);
        }
        if !self.eroare.is_empty() {
            qs.append_pair("eroare", &self.eroare);
        }
        write!(f, "{}", qs.finish())
    }
}
