use anyhow::Result;
use core::convert::TryInto;
use esp_idf_svc::netif::{EspNetif, NetifConfiguration, NetifStack};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripheral,
    nvs::EspDefaultNvsPartition,
    wifi::{AccessPointConfiguration, AuthMethod, BlockingWifi, Configuration as WifiConfiguration, EspWifi, WifiDriver},
};

use esp_idf_svc::ipv4::{
    ClientConfiguration as IpClientConfiguration,
    Configuration as IpConfiguration, DHCPClientSettings,
};
use log::info;

const SSID: &str = "MyRustAP";
const PASSWORD: &str = "password123";

pub fn start_access_point(
    modem: impl peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: EspSystemEventLoop,    
) -> Result<Box<EspWifi<'static>>> {

    let nvs = EspDefaultNvsPartition::take()?;

    let wifi = WifiDriver::new(modem, sysloop.clone(), Some(nvs))?;
    let mut esp_wifi = configure_wifi(wifi)?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;
    
    wifi.start()?;
    info!("Wifi started");
    
    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    let _hostname = wifi.wifi().ap_netif().get_hostname()?;
    info!("Hostname: {:?}", _hostname);
    let ip_info = wifi.wifi().ap_netif().get_ip_info()?;
    info!("Access Point IP info: {:?}", ip_info);

    Ok(Box::new(esp_wifi))

}

fn configure_wifi(wifi: WifiDriver) -> anyhow::Result<EspWifi> {

    let mut wifi = EspWifi::wrap_all(
        wifi,
        // Note that setting a custom hostname can be used with any network adapter, not just Wifi
        // I.e. that would work with Eth as well, because DHCP is an L3 protocol
        EspNetif::new_with_conf(&NetifConfiguration {
            ip_configuration: Some(IpConfiguration::Client(IpClientConfiguration::DHCP(
                DHCPClientSettings {
                    hostname: Some("light".try_into().unwrap()),
                },
            ))),  
            ..NetifConfiguration::wifi_default_client()
        })?,
        EspNetif::new(NetifStack::Ap)?,        
    )?;

    let wifi_configuration = WifiConfiguration::AccessPoint(AccessPointConfiguration {
        ssid: SSID.try_into().unwrap(),
        password: PASSWORD.try_into().unwrap(),
        channel : 1,
        auth_method: AuthMethod::WPA2Personal,
        max_connections: 4,
        ..Default::default()
    });
 
    wifi.set_configuration(&wifi_configuration)?;

    Ok(wifi)
}