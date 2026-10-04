//! Diagnostic de la LED embarquée : pilote en parallèle GPIO2 (ESP32-C3-DevKit-RUST-1) et
//! GPIO8 (ESP32-C3-DevKitM-1 / DevKitC-02) avec des couleurs pleines, pour distinguer une LED
//! morte d'une mauvaise broche. Observer la carte : rouge, vert, bleu, blanc, puis extinction,
//! en boucle, deux secondes par couleur.

use anyhow::Result;
use esp_idf_svc::hal::{delay::FreeRtos, peripherals::Peripherals};
use log::info;
use rgb_led::{RGB8, WS2812RMT};

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let p = Peripherals::take()?;
    let mut gpio2 = WS2812RMT::new(p.pins.gpio2, p.rmt.channel0)?;
    let mut gpio8 = WS2812RMT::new(p.pins.gpio8, p.rmt.channel1)?;
    info!("diagnostic LED : GPIO2 (canal RMT 0) et GPIO8 (canal RMT 1), couleurs pleines");

    let steps = [
        ("rouge", RGB8::new(255, 0, 0)),
        ("vert", RGB8::new(0, 255, 0)),
        ("bleu", RGB8::new(0, 0, 255)),
        ("blanc", RGB8::new(255, 255, 255)),
        ("éteint", RGB8::new(0, 0, 0)),
    ];
    loop {
        for (name, color) in steps {
            info!("{name} sur GPIO2 et GPIO8");
            gpio2.set_pixel(color)?;
            gpio8.set_pixel(color)?;
            FreeRtos::delay_ms(2000);
        }
    }
}
