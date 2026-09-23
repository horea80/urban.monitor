//! urban-shared: tipuri de domeniu și utilitare de text folosite atât pe server,
//! cât și în browser (compilat la wasm). Fără I/O, fără dependențe grele.

pub mod model;
pub mod text;
pub mod time;

pub use model::{
    AccountView, AgendaRowView, Category, Document, Item, KeywordView, Meeting, MeetingDetail, SearchQuery,
    SearchResult, Status, StreetCount, SyncRun,
};

pub const NAME: &str = "urban.monitor";
