//! Sincronizarea periodică (FR-7.1): un singur task, rulările sunt strict secvențiale, deci nu pot
//! porni două în paralel. Intervalul vine din `URBAN_SYNC_HOURS`; `0` oprește task-ul.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tracing::{error, info, warn};
use urban_core::pdf::Chain;
use urban_core::scrape::Client;
use urban_core::sync::{self, SyncOptions};

use crate::state::AppState;

/// Pauza înainte de prima sincronizare, ca serverul să răspundă la /healthz imediat după pornire.
const INITIAL_DELAY: Duration = Duration::from_secs(15);

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
        loop {
            run_once(state, &client, &pdf).await;
            tokio::time::sleep(interval).await;
        }
    });
}

async fn run_once(state: &'static AppState, client: &Client, pdf: &Chain) {
    if state.sync_running.swap(true, Ordering::SeqCst) {
        warn!("o sincronizare este deja în curs; sar peste această rundă");
        return;
    }
    let started = Instant::now();
    match sync::run(&state.cfg, client, &state.db, pdf, SyncOptions::default()).await {
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
}
