//! urban-core: partea server-only a proiectului: scraper, PDF, agenda, SQLite, sync.
//! Parserele sunt functii pure testate pe fixture-urile din `tests/fixtures/`.

pub mod agenda;
pub mod config;
pub mod db;
pub mod error;
pub mod pdf;
pub mod scrape;
pub mod sync;

pub use config::Config;
pub use error::{Error, Result};
pub use urban_shared as shared;
