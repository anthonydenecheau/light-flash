//! Tâche lumière : boucle de rendu à cadence fixe, seule à parler au driver LED.

use anyhow::Result;
use light_core::{power, Renderer, SharedState, RGB8};
use log::{error, info};
use rgb_led::WS2812RMT;
use std::{
    thread,
    time::{Duration, Instant},
};

/// Nombre de LED pilotées : 1 = LED embarquée ; 144 avec le ruban (voir HARDWARE.md).
pub const LED_COUNT: usize = 1;
/// Budget de courant d'une trame, en mA. 500 mA suffit à la LED embarquée alimentée par USB ;
/// avec le ruban, régler selon l'alimentation (HARDWARE.md §2).
pub const MAX_MILLIAMPS: u32 = 500;
/// Période de rendu : 20 ms, soit 50 images par seconde.
const FRAME: Duration = Duration::from_millis(20);
/// Pile du thread : rendu d'une trame + driver RMT.
const STACK_SIZE: usize = 8 * 1024;

pub fn spawn(led: WS2812RMT<'static>, shared: SharedState) -> Result<()> {
    thread::Builder::new()
        .name("light".into())
        .stack_size(STACK_SIZE)
        .spawn(move || run(led, shared))?;
    Ok(())
}

fn run(mut led: WS2812RMT<'static>, shared: SharedState) {
    let mut renderer = Renderer::default();
    let mut frame = vec![RGB8::default(); LED_COUNT];
    let mut last_sent: Option<Vec<RGB8>> = None;
    let mut last_tick = Instant::now();
    let mut over_budget = false;
    info!("tâche lumière démarrée : {LED_COUNT} LED, {MAX_MILLIAMPS} mA maximum");

    loop {
        let now = Instant::now();
        renderer.tick(now.duration_since(last_tick).as_millis() as u32);
        last_tick = now;

        // Copie de l'état : le verrou n'est jamais tenu pendant l'accès au driver.
        let state = *shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        renderer.render(&state, &mut frame);

        let estimated = power::estimate_ma(&frame);
        let sent = power::limit(&mut frame, MAX_MILLIAMPS);
        if sent < estimated && !over_budget {
            info!("trame ramenée de {estimated} à {sent} mA (budget {MAX_MILLIAMPS} mA)");
        }
        over_budget = sent < estimated;

        // N'écrire sur le ruban que si la trame change (couleur unie : une seule fois).
        if last_sent.as_deref() != Some(frame.as_slice()) {
            match led.set_pixels(&frame) {
                Ok(()) => last_sent = Some(frame.clone()),
                Err(e) => error!("envoi vers les LED : {e}"),
            }
        }
        thread::sleep(FRAME);
    }
}
