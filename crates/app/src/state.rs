//! Starea procesului: configurația, baza de date și indicatorul „sync în curs”.
//! Se setează o singură dată la pornire și e citită de funcțiile server, de API și de job-ul de sync.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use urban_core::Config;
use urban_core::db::Db;

pub struct AppState {
    pub cfg: Config,
    pub db: Db,
    pub started_at: DateTime<Utc>,
    pub sync_running: AtomicBool,
}

static STATE: OnceLock<AppState> = OnceLock::new();

pub fn init(cfg: Config, db: Db) -> &'static AppState {
    let state = AppState {
        cfg,
        db,
        started_at: Utc::now(),
        sync_running: AtomicBool::new(false),
    };
    if STATE.set(state).is_err() {
        panic!("starea aplicației se inițializează o singură dată");
    }
    get()
}

pub fn get() -> &'static AppState {
    STATE.get().expect("starea aplicației nu a fost inițializată")
}

/// Un handle clonat al bazei, pentru `spawn_blocking`.
pub fn db() -> Db {
    get().db.clone()
}

pub fn sync_running() -> bool {
    get().sync_running.load(Ordering::SeqCst)
}
