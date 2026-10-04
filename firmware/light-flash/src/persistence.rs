//! Thread de persistance : enregistre l'état de la lampe en NVS après 2 s de calme,
//! seulement s'il a changé (voir `light_core::persist::SaveScheduler`).

use anyhow::Result;
use light_core::{LightState, SaveScheduler, SharedState};
use log::{error, info};
use std::{
    thread,
    time::{Duration, Instant},
};
use storage::SharedStorage;

/// Période d'observation de l'état.
const POLL: Duration = Duration::from_millis(250);
const STACK_SIZE: usize = 6 * 1024;

pub fn spawn(shared: SharedState, storage: SharedStorage, saved: Option<LightState>) -> Result<()> {
    thread::Builder::new()
        .name("persist".into())
        .stack_size(STACK_SIZE)
        .spawn(move || run(shared, storage, saved))?;
    Ok(())
}

fn run(shared: SharedState, storage: SharedStorage, saved: Option<LightState>) {
    let mut scheduler = SaveScheduler::new(saved, SaveScheduler::DEFAULT_QUIET_MS);
    let start = Instant::now();
    loop {
        thread::sleep(POLL);
        let state = *shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now_ms = start.elapsed().as_millis() as u64;
        if let Some(to_save) = scheduler.observe(state, now_ms) {
            match storage::lock(&storage).set_light_state(&to_save) {
                Ok(()) => info!("état enregistré en NVS : {to_save}"),
                Err(e) => {
                    error!("enregistrement de l'état en NVS : {e}");
                    scheduler.save_failed();
                }
            }
        }
    }
}
