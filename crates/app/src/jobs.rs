//! Sincronizarea periodică (FR-7.1): un singur task, rulările sunt strict secvențiale, deci nu pot
//! porni două în paralel. Intervalul vine din `URBAN_SYNC_HOURS`; `0` oprește task-ul. După fiecare
//! rulare pleacă alertele: ședințele noi către lista fixă (FR-8) și rezumatele pe cuvinte-cheie (FR-9).

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tracing::{error, info, warn};
use urban_core::notify::Mailer;
use urban_core::pdf::Chain;
use urban_core::scrape::Client;
use urban_core::sync::{self, SyncOptions};

use crate::state::AppState;

/// Pauza înainte de prima sincronizare, ca serverul să răspundă la /healthz imediat după pornire.
const INITIAL_DELAY: Duration = Duration::from_secs(15);

/// Marcaj în `URBAN_DATA_DIR`: dacă există la pornire, prima sincronizare e completă (`just resync full`).
const FULL_SYNC_MARKER: &str = "full-sync";

pub fn spawn(state: &'static AppState) {
    let interval = state.cfg.sync_interval;
    if interval.is_zero() {
        info!("URBAN_SYNC_HOURS=0: sincronizarea automată este oprită");
        return;
    }
    let client = match Client::new(&state.cfg) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "nu am putut crea clientul HTTP; sincronizarea automată este oprită");
            return;
        }
    };
    let pdf = Chain::default_chain();
    tokio::spawn(async move {
        info!(
            interval_h = interval.as_secs_f64() / 3600.0,
            "sincronizare periodică pornită"
        );
        tokio::time::sleep(INITIAL_DELAY).await;
        let mut full = take_full_sync_marker(state);
        loop {
            run_once(state, &client, &pdf, state.mailer.as_ref(), SyncOptions { full }).await;
            full = false;
            tokio::time::sleep(interval).await;
        }
    });
}

/// Șterge marcajul înainte de rulare, ca o sincronizare completă eșuată să nu se repete la fiecare pornire.
fn take_full_sync_marker(state: &AppState) -> bool {
    let path = state.cfg.data_dir.join(FULL_SYNC_MARKER);
    match std::fs::remove_file(&path) {
        Ok(()) => {
            info!(path = %path.display(), "marcaj găsit: prima sincronizare este completă");
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "nu am putut șterge marcajul; sincronizare obișnuită");
            false
        }
    }
}

async fn run_once(state: &'static AppState, client: &Client, pdf: &Chain, mailer: Option<&Mailer>, opts: SyncOptions) {
    if state.sync_running.swap(true, Ordering::SeqCst) {
        warn!("o sincronizare este deja în curs; sar peste această rundă");
        return;
    }
    let started = Instant::now();
    match sync::run(&state.cfg, client, &state.db, pdf, opts).await {
        Ok(rep) => info!(
            meetings_seen = rep.meetings_seen,
            meetings_updated = rep.meetings_updated,
            items_new = rep.items_new,
            warnings = rep.warnings.len(),
            errors = rep.errors.len(),
            secs = started.elapsed().as_secs(),
            "sincronizare terminată"
        ),
        Err(e) => error!(error = %e, secs = started.elapsed().as_secs(), "sincronizarea a eșuat"),
    }
    state.sync_running.store(false, Ordering::SeqCst);

    // FR-8: alertele pleacă după sincronizare; un eșec se reîncearcă la rularea următoare
    if let Some(m) = mailer {
        match m.alert_new_meetings(&state.db).await {
            Ok(0) => {}
            Ok(n) => info!(n, "alertă trimisă pentru ședințe noi"),
            Err(e) => error!(error = %e, "alerta pe email a eșuat; reîncerc la următoarea sincronizare"),
        }
        match urban_core::alerts::run_keyword_alerts(&state.db, m).await {
            Ok(st) if st.emails > 0 || st.errors > 0 => info!(
                users = st.users,
                emails = st.emails,
                items = st.items,
                errors = st.errors,
                "alerte pe cuvinte-cheie"
            ),
            Ok(_) => {}
            Err(e) => error!(error = %e, "alertele pe cuvinte-cheie au eșuat"),
        }
    }
}
