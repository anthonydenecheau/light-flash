use anyhow::{bail, Result};
use core::convert::TryInto;
use std::net::Ipv4Addr;
use std::str::FromStr;
use esp_idf_sys as _;
use esp_idf_svc::netif::{EspNetif, NetifConfiguration, NetifStack};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripheral,
    wifi::{AccessPointConfiguration, AuthMethod, BlockingWifi, Configuration as WifiConfiguration, EspWifi, WifiDriver},
};
use esp_idf_svc::ipv4::{
    ClientConfiguration as IpClientConfiguration, ClientSettings as IpClientSettings,
    Configuration as IpConfiguration, Mask, Subnet,
};
use log::info;

// Expects IPv4 address
const DEVICE_IP: &str = "192.168.4.1";
// Expects IPv4 address
const GATEWAY_IP: &str = "192.168.4.1";
// Expects a number between 0 and 32, defaults to 24
const GATEWAY_NETMASK: Option<&str> = option_env!("GATEWAY_NETMASK");

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
    let wifi = WifiDriver::new(modem, sysloop.clone(), None)?;
    let mut esp_wifi = configure_wifi(wifi)?;

    let mut wifi = BlockingWifi::wrap(&mut esp_wifi, sysloop)?;
    
    info!("Starting wifi...");
    wifi.start()?;

    let ip_info = wifi.wifi().ap_netif().get_ip_info()?;
    info!("Access Point IP info: {:?}", ip_info);

    Ok(Box::new(esp_wifi))

}

fn configure_wifi(wifi: WifiDriver) -> anyhow::Result<EspWifi> {
    let netmask = GATEWAY_NETMASK.unwrap_or("24");
    let netmask = u8::from_str(netmask)?;
    let gateway_addr = Ipv4Addr::from_str(GATEWAY_IP)?;
    let static_ip = Ipv4Addr::from_str(DEVICE_IP)?;

    let mut wifi = EspWifi::wrap_all(
        wifi,
        EspNetif::new_with_conf(&NetifConfiguration {
            ip_configuration: Some(IpConfiguration::Client(IpClientConfiguration::Fixed(
                IpClientSettings {
                    ip: static_ip,
                    subnet: Subnet {
                        gateway: gateway_addr,
                        mask: Mask(netmask),
                    },
                    // Can also be set to Ipv4Addrs if you need DNS
                    dns: None,
                    secondary_dns: None,
                },
            ))),
            ..NetifConfiguration::wifi_default_client()
        })?,
        EspNetif::new(NetifStack::Ap)?,        
    )?;

    info!("Setting configuration...");
    let wifi_configuration = WifiConfiguration::AccessPoint(AccessPointConfiguration {
        ssid: "MyRustAP"
            .try_into()
            .expect("Could not parse the given SSID into WiFi config"),
        password: "password123"
            .try_into()
            .expect("Could not parse the given password into WiFi config"),
        channel : 1,
        auth_method: AuthMethod::WPA2Personal,
        max_connections: 4,
        ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;

    Ok(wifi)
}