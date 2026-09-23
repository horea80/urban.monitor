//! Configurare din variabile de mediu, cu valori implicite (NFR-6).

use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    /// Directorul cu baza SQLite și snapshot-urile brute.
    pub data_dir: PathBuf,
    /// Prima ședință indexată: anul ≥ acesta.
    pub start_year: i32,
    /// Ședințele mai noi de atâtea zile se re-descarcă la fiecare sync.
    pub refresh_days: i64,
    /// Pauza minimă între două cereri către site-ul primăriei.
    pub request_delay: Duration,
    /// Intervalul task-ului de sync din server.
    pub sync_interval: Duration,
    pub user_agent: String,
    pub listing_url: String,
    /// Adresa publică a site-ului, pentru linkurile din alerte.
    pub public_url: String,
    /// Alerte pe email la ședințe noi (FR-8); `None` când lipsește `RESEND_API_KEY`.
    pub alert: Option<AlertConfig>,
}

/// Trimiterea alertelor prin Resend (ADR-0011).
#[derive(Clone)]
pub struct AlertConfig {
    pub resend_api_key: String,
    /// „Monitor Urban <noreply@hopartean.com>”; domeniul trebuie verificat în Resend.
    pub from: String,
    pub to: Vec<String>,
}

impl std::fmt::Debug for AlertConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlertConfig")
            .field("resend_api_key", &"***")
            .field("from", &self.from)
            .field("to", &self.to)
            .finish()
    }
}

impl Config {
    pub const LISTING_URL: &'static str = "https://primariaclujnapoca.ro/strategii-urbane/comisia-tehnica-de-amenajare-a-teritoriului-si-urbanism/sedinte-comisie/";

    pub fn from_env() -> Self {
        let sync_hours: f64 = env_parse("URBAN_SYNC_HOURS", 6.0);
        Config {
            data_dir: PathBuf::from(env_or("URBAN_DATA_DIR", "data")),
            start_year: env_parse("URBAN_START_YEAR", 2026),
            refresh_days: env_parse("URBAN_REFRESH_DAYS", 45),
            request_delay: Duration::from_millis(env_parse("URBAN_REQUEST_DELAY_MS", 1000)),
            sync_interval: Duration::from_secs_f64(sync_hours.max(0.0) * 3600.0),
            user_agent: env_or(
                "URBAN_USER_AGENT",
                "urban-monitor/0.1 (monitorizare publica a sedintelor CTATU Cluj-Napoca; uz personal)",
            ),
            listing_url: env_or("URBAN_LISTING_URL", Self::LISTING_URL),
            public_url: env_or("URBAN_PUBLIC_URL", "https://urbanism.hopartean.com")
                .trim_end_matches('/')
                .to_owned(),
            alert: std::env::var("RESEND_API_KEY")
                .ok()
                .map(|k| k.trim().to_owned())
                .filter(|k| !k.is_empty())
                .map(|key| AlertConfig {
                    resend_api_key: key,
                    from: env_or("ALERT_FROM", "Monitor Urban <noreply@hopartean.com>"),
                    to: env_or("ALERT_TO", "horea.hopartean@gmail.com")
                        .split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect(),
                }),
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("urban.db")
    }

    pub fn raw_dir(&self) -> PathBuf {
        self.data_dir.join("raw")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn env_parse<T: FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}
