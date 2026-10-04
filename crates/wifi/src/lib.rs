//! Wi-Fi bloquant : station (STA) ou point d'accès (AP), sur un même driver réutilisable.

use anyhow::{anyhow, bail, Result};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{modem::Modem, peripheral::Peripheral},
    ipv4::IpInfo,
    nvs::EspDefaultNvsPartition,
    wifi::{
        AccessPointConfiguration, AuthMethod, BlockingWifi, ClientConfiguration, Configuration,
        EspWifi,
    },
};
use log::{info, warn};

/// Paramètres d'un point d'accès.
pub struct AccessPoint<'a> {
    pub ssid: &'a str,
    /// Vide = réseau ouvert ; sinon 8 caractères minimum (WPA2).
    pub password: &'a str,
    pub channel: u8,
    pub max_connections: u16,
}

impl Default for AccessPoint<'_> {
    fn default() -> Self {
        Self {
            ssid: "light-flash",
            password: "",
            channel: 1,
            max_connections: 4,
        }
    }
}

/// Crée le driver Wi-Fi sans le démarrer. Passer la partition NVS permet à ESP-IDF
/// de conserver la calibration RF entre deux démarrages.
pub fn new_wifi(
    modem: impl Peripheral<P = Modem> + 'static,
    sysloop: EspSystemEventLoop,
    nvs: Option<EspDefaultNvsPartition>,
) -> Result<EspWifi<'static>> {
    Ok(EspWifi::new(modem, sysloop, nvs)?)
}

/// Se connecte en station et attend un bail DHCP. esp-idf-svc borne la connexion à 15 s ;
/// un échec (réseau introuvable, mot de passe refusé, pas de DHCP) revient en `Err`.
pub fn connect_sta(
    esp_wifi: &mut EspWifi<'static>,
    sysloop: EspSystemEventLoop,
    ssid: &str,
    pass: &str,
) -> Result<IpInfo> {
    if ssid.is_empty() {
        bail!("SSID vide");
    }
    let auth_method = if pass.is_empty() {
        info!("mot de passe Wi-Fi vide : réseau ouvert");
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    };
    let mut wifi = BlockingWifi::wrap(esp_wifi, sysloop)?;
    if wifi.is_started()? {
        wifi.stop()?;
    }
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid
            .try_into()
            .map_err(|_| anyhow!("SSID trop long (32 octets maximum)"))?,
        password: pass
            .try_into()
            .map_err(|_| anyhow!("mot de passe Wi-Fi trop long (64 octets maximum)"))?,
        auth_method,
        ..Default::default()
    }))?;

    info!("Wi-Fi : connexion à « {ssid} »");
    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;
    let ip = wifi.wifi().sta_netif().get_ip_info()?;
    info!("Wi-Fi connecté, adresse {}", ip.ip);
    Ok(ip)
}

/// Démarre un point d'accès et attend que son interface soit prête
/// (adresse ESP-IDF par défaut : 192.168.71.1).
pub fn start_access_point(
    esp_wifi: &mut EspWifi<'static>,
    sysloop: EspSystemEventLoop,
    ap: &AccessPoint,
) -> Result<IpInfo> {
    let auth_method = if ap.password.is_empty() {
        warn!("point d'accès « {} » ouvert, sans mot de passe", ap.ssid);
        AuthMethod::None
    } else {
        if ap.password.len() < 8 {
            bail!("mot de passe du point d'accès trop court (8 caractères minimum)");
        }
        AuthMethod::WPA2Personal
    };
    let mut wifi = BlockingWifi::wrap(esp_wifi, sysloop)?;
    if wifi.is_started()? {
        wifi.stop()?;
    }
    wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
        ssid: ap
            .ssid
            .try_into()
            .map_err(|_| anyhow!("SSID du point d'accès trop long (32 octets maximum)"))?,
        password: ap
            .password
            .try_into()
            .map_err(|_| anyhow!("mot de passe du point d'accès trop long (64 octets maximum)"))?,
        auth_method,
        channel: ap.channel,
        max_connections: ap.max_connections,
        ..Default::default()
    }))?;
    wifi.start()?;
    wifi.wait_netif_up()?;
    let ip = wifi.wifi().ap_netif().get_ip_info()?;
    info!("point d'accès « {} » démarré, adresse {}", ap.ssid, ip.ip);
    Ok(ip)
}

/// Raccourci : crée le driver et se connecte en station (utilisé par `hardware-check`).
pub fn wifi(
    ssid: &str,
    pass: &str,
    modem: impl Peripheral<P = Modem> + 'static,
    sysloop: EspSystemEventLoop,
) -> Result<Box<EspWifi<'static>>> {
    let mut esp_wifi = new_wifi(modem, sysloop.clone(), None)?;
    connect_sta(&mut esp_wifi, sysloop, ssid, pass)?;
    Ok(Box::new(esp_wifi))
}
