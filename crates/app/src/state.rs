//! Starea procesului: configurația, baza de date și indicatorul „sync în curs”.
//! Se setează o singură dată la pornire și e citită de funcțiile server, de API și de job-ul de sync.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use tracing::{error, info};
use urban_core::Config;
use urban_core::db::Db;
use urban_core::notify::Mailer;

pub struct AppState {
    pub cfg: Config,
    pub db: Db,
    /// `None` fără `RESEND_API_KEY`: alertele și linkurile de autentificare nu pleacă pe email.
    pub mailer: Option<Mailer>,
    pub started_at: DateTime<Utc>,
    pub sync_running: AtomicBool,
}

static STATE: OnceLock<AppState> = OnceLock::new();

pub fn init(cfg: Config, db: Db) -> &'static AppState {
    let mailer = match Mailer::from_config(&cfg) {
        Ok(Some(m)) => {
            info!(to = ?m.recipients(), "email activ: alerte (FR-8, FR-9) și autentificare");
            Some(m)
        }
        Ok(None) => {
            info!("RESEND_API_KEY lipsește: emailurile nu pleacă (linkurile de autentificare ajung în jurnal)");
            None
        }
        Err(e) => {
            error!(error = %e, "nu am putut porni trimiterea de email");
            None
        }
    };
    let state = AppState {
        cfg,
        db,
        mailer,
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
