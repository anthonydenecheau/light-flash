//! Contre-épreuve : driver WS2812 d'origine de esp-rs/std-training (FixedLengthSignal, impulsions
//! recalculées à chaque envoi), copié tel quel, sans passer par `rgb_led`. Cycle rouge, vert,
//! bleu, blanc, éteint sur GPIO2 (ESP32-C3-DevKit-RUST-1), 2 s par couleur.

use anyhow::Result;
use core::time::Duration;
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::OutputPin,
    peripheral::Peripheral,
    peripherals::Peripherals,
    rmt::{config::TransmitConfig, FixedLengthSignal, PinState, Pulse, RmtChannel, TxRmtDriver},
};
use log::info;
use rgb::RGB8;

struct Original<'a> {
    tx: TxRmtDriver<'a>,
}

impl<'d> Original<'d> {
    fn new(
        led: impl Peripheral<P = impl OutputPin> + 'd,
        channel: impl Peripheral<P = impl RmtChannel> + 'd,
    ) -> Result<Self> {
        let config = TransmitConfig::new().clock_divider(2);
        let tx = TxRmtDriver::new(channel, led, &config)?;
        Ok(Self { tx })
    }

    fn set_pixel(&mut self, rgb: RGB8) -> Result<()> {
        let color: u32 = ((rgb.g as u32) << 16) | ((rgb.r as u32) << 8) | rgb.b as u32;
        let ticks_hz = self.tx.counter_clock()?;
        let ns = Duration::from_nanos;
        let t0h = Pulse::new_with_duration(ticks_hz, PinState::High, &ns(350))?;
        let t0l = Pulse::new_with_duration(ticks_hz, PinState::Low, &ns(800))?;
        let t1h = Pulse::new_with_duration(ticks_hz, PinState::High, &ns(700))?;
        let t1l = Pulse::new_with_duration(ticks_hz, PinState::Low, &ns(600))?;
        let mut signal = FixedLengthSignal::<24>::new();
        for i in (0..24).rev() {
            let p = 2_u32.pow(i);
            let bit = p & color != 0;
            let (high_pulse, low_pulse) = if bit { (t1h, t1l) } else { (t0h, t0l) };
            signal.set(23 - i as usize, &(high_pulse, low_pulse))?;
        }
        self.tx.start_blocking(&signal)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let p = Peripherals::take()?;
    let mut led = Original::new(p.pins.gpio2, p.rmt.channel0)?;
    info!(
        "driver d'origine std-training sur GPIO2, horloge RMT {} Hz",
        led.tx.counter_clock()?.0
    );
    let steps = [
        ("rouge", RGB8::new(255, 0, 0)),
        ("vert", RGB8::new(0, 255, 0)),
        ("bleu", RGB8::new(0, 0, 255)),
        ("blanc", RGB8::new(255, 255, 255)),
        ("éteint", RGB8::new(0, 0, 0)),
    ];
    loop {
        for (name, color) in steps {
            info!("{name}");
            led.set_pixel(color)?;
            FreeRtos::delay_ms(2000);
        }
    }
}
