use anyhow::{bail, Result};
use esp_idf_sys as _;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripheral,
    wifi::{AccessPointConfiguration, AuthMethod, BlockingWifi, Configuration, EspWifi},
};
use log::info;

pub fn start_access_point(
    ssid: &str,
    pass: &str,
    modem: impl peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,    
) -> Result<Box<EspWifi<'static>>> {

    if ssid.is_empty() {
        bail!("Missing WiFi name")
    }
    if pass.is_empty() {
        info!("Wifi password is empty");
    }
    let mut esp_wifi = EspWifi::new(modem, sysloop.clone(), None)?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
        ssid: ssid
            .try_into()
            .expect("Could not parse the given SSID into WiFi config"),
        password: pass
            .try_into()
            .expect("Could not parse the given password into WiFi config"),
        channel : 1,
        auth_method: AuthMethod::WPA2Personal,
        max_connections: 4,
        ..Default::default()
    }))?;

    info!("Starting wifi...");
    wifi.start()?;

    let ip_info = wifi.wifi().ap_netif().get_ip_info()?;
    info!("Access Point IP info: {:?}", ip_info);

    Ok(Box::new(esp_wifi))

}
