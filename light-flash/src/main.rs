use wifi_ap::start_access_point;
use anyhow::{bail, Result};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::Peripherals;
use log::info;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    info!("Hello, world!");

    let _wifi = match start_access_point(
        "MyRustAP",
        "password123",
        peripherals.modem,
        sysloop,
    ) {
        Ok(inner) => inner,
        Err(err) => {
            bail!("Could not start Wi-Fi access point : {:?}", err)
        }
    };

    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        info!("Hello, world!");
    }
}
