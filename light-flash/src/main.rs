use wifi_ap::start_access_point;
use http_server::start_http_server;
use anyhow::{bail, Result};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::Peripherals;
use log::info;
use std::sync::Arc;
use std::thread;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    info!("Hello, world!");

    info!("Warming up WIFI AP !");
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

    info!("Warming up HTTP SERVER !");
    let server = Arc::new(start_http_server()?);    
    // Create a thread to keep the server running
    let server_thread = {
        let _server = Arc::clone(&server);
        thread::spawn(move || loop {
            thread::park();
        })
    };

    // Keep the main function running
    server_thread.join().unwrap();

    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        info!("Still running!");
    }
}
