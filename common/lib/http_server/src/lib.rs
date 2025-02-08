use core::str;
use embedded_svc::{
    http::{Headers, Method}, 
    io::{Read, Write},
};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::http::server::{Configuration,EspHttpServer};
use anyhow::{bail, Result};
use wifi::wifi;
use serde::{Deserialize, Serialize};

// Max payload length
const MAX_LEN: usize = 128;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
struct UserSettings {
    wifi_ssid: &'static str,
    wifi_psk: &'static str,
}

fn main() -> Result<()> {

    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    let mut server = EspHttpServer::new(&Configuration::default())?;

    server.fn_handler("/", Method::Get, |request| {
        let html = include_str!("./static/index.html");
        let mut response = request.into_ok_response()?;
        response.write_all(html.as_bytes());
        Ok(())
    })?;

    server.fn_handler("/style.css", Method::Get, |request| {
        let css = include_str!("./static/style.css");
        let mut response = request.into_ok_response()?;
        response.write_all(css.as_bytes());
        Ok(())
    })?;

    server.fn_handler("/connect", Method::Post, move |mut request| {
        let len = request.content_len().unwrap_or(0) as usize;
        if len > MAX_LEN {
            request.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }
        let mut buf = vec![0; len];
        request.read_exact(&mut buf)?;
        let mut resp = request.into_ok_response()?;
        if let Ok(form) = serde_json::from_slice::<UserSettings>(&buf) {
            let ssid = form.wifi_ssid;
            let password = form.wifi_psk;

            println!("Connecting to SSID: {}, Password: {}", ssid, password);

            // Connect to the Wi-Fi network
            let _wifi = match wifi(
                ssid,
                password,
                peripherals.modem,
                sysloop,
            ) {
                Ok(inner) => inner,
                Err(err) => {
                    bail!("Could not connect to Wi-Fi network: {:?}", err)
                }
            };

        }

        let mut resp = request.into_ok_response()?;
        resp.write_all(b"Connecting...")?;
        resp.flush()?;
        Ok(())
    })?;

    Ok(())    

}
