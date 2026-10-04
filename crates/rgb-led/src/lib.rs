//! Driver WS2812 (NeoPixel) sur le périphérique RMT, pour la LED embarquée ou un ruban.

use anyhow::Result;
use core::time::Duration;
use esp_idf_hal::{
    gpio::OutputPin,
    peripheral::Peripheral,
    rmt::{config::TransmitConfig, PinState, Pulse, RmtChannel, TxRmtDriver, VariableLengthSignal},
};

pub use rgb::RGB8;

/// Timings WS2812B (fiche technique) : T0H 0,35 µs, T0L 0,8 µs, T1H 0,7 µs, T1L 0,6 µs.
const T0H_NS: u64 = 350;
const T0L_NS: u64 = 800;
const T1H_NS: u64 = 700;
const T1L_NS: u64 = 600;

pub struct WS2812RMT<'d> {
    tx: TxRmtDriver<'d>,
    /// Impulsions d'un bit à 0 et d'un bit à 1, calculées une fois pour toutes.
    zero: [Pulse; 2],
    one: [Pulse; 2],
}

impl<'d> WS2812RMT<'d> {
    /// Rust ESP Board : gpio2 ; ESP32-C3-DevKitC-02 : gpio8.
    pub fn new(
        led: impl Peripheral<P = impl OutputPin> + 'd,
        channel: impl Peripheral<P = impl RmtChannel> + 'd,
    ) -> Result<Self> {
        let config = TransmitConfig::new().clock_divider(2);
        let tx = TxRmtDriver::new(channel, led, &config)?;
        let ticks_hz = tx.counter_clock()?;
        let pulse =
            |state, ns| Pulse::new_with_duration(ticks_hz, state, &Duration::from_nanos(ns));
        let zero = [
            pulse(PinState::High, T0H_NS)?,
            pulse(PinState::Low, T0L_NS)?,
        ];
        let one = [
            pulse(PinState::High, T1H_NS)?,
            pulse(PinState::Low, T1L_NS)?,
        ];
        Ok(Self { tx, zero, one })
    }

    pub fn set_pixel(&mut self, rgb: RGB8) -> Result<()> {
        self.set_pixels(&[rgb])
    }

    /// Envoie une trame complète ; `pixels[0]` est la LED la plus proche de la carte.
    /// Bloque le temps de la transmission (≈ 30 µs par LED).
    pub fn set_pixels(&mut self, pixels: &[RGB8]) -> Result<()> {
        let mut signal = VariableLengthSignal::with_capacity(pixels.len() * 24);
        for px in pixels {
            // Ordre de transmission WS2812 : vert, rouge, bleu, bit de poids fort en premier.
            let grb = ((px.g as u32) << 16) | ((px.r as u32) << 8) | px.b as u32;
            for i in (0..24).rev() {
                let bit = if grb & (1 << i) != 0 {
                    &self.one
                } else {
                    &self.zero
                };
                signal.push(bit)?;
            }
        }
        self.tx.start_blocking(&signal)?;
        Ok(())
    }
}
