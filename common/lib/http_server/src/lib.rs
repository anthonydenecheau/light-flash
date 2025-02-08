use embedded_svc::{
    http::{Headers, Method},
    io::{Read, Write},
};
use esp_idf_svc::http::server::EspHttpServer;
use serde::Deserialize;
use log::info;

// Max payload length
const MAX_LEN: usize = 128;
// Need lots of stack to parse JSON 
const STACK_SIZE: usize = 10240;

static INDEX_HTML: &str = include_str!("./static/index.html");
static LOGO: &[u8] = include_bytes!("logo.png");

#[derive(Deserialize)]
struct WifiSettings<'a> {
    wifi_ssid: &'a str,
    wifi_psk: &'a str,
}

pub fn start_http_server() -> anyhow::Result<EspHttpServer<'static>> {

    info!("Create server...");
    let mut server = create_server()?;

    info!("Routes definition...");
    // Serve the HTML file
    server.fn_handler("/", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(INDEX_HTML.as_bytes())
            .map(|_| ())
    })?;

    // Serve the logo image
    server.fn_handler("/logo.png", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(LOGO)
            .map(|_| ())        
    })?;

    server.fn_handler::<anyhow::Error, _>("/connect", Method::Post, |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;

        if len > MAX_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }

        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let mut resp = req.into_ok_response()?;

        if let Ok(form) = serde_json::from_slice::<WifiSettings>(&buf) {
            info!("Credentials: {:?} {:?}", form.wifi_ssid, form.wifi_psk);
            write!(
                resp,
                "SSID {}- Password {}!",
                form.wifi_ssid, form.wifi_psk
            )?;
        } else {
            resp.write_all("JSON error".as_bytes())?;
        }

        Ok(())
    })?;

    // Main task no longer needed, free up some memory
    info!("HTTP server started");
    Ok(server)

}

fn create_server() -> anyhow::Result<EspHttpServer<'static>> {
    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };

    Ok(EspHttpServer::new(&server_configuration)?)
}