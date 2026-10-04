//! Firmware de l'applique : tâche lumière, persistance, réseau (station ou point d'accès de
//! secours), provisioning Improv sur BLE, mDNS et serveur HTTP de pilotage.

use anyhow::Result;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        gpio::{PinDriver, Pull},
        peripherals::Peripherals,
        reset,
    },
    log::EspLogger,
    mdns::EspMdns,
    nvs::EspDefaultNvsPartition,
    sys,
};
use http_server::{DebugHooks, HttpContext, PeerView, StatusView, SystemHooks};
use light_core::{naming, LightState, SharedIndication, SharedState};
use log::info;
use rgb_led::WS2812RMT;
use std::{
    sync::{atomic::Ordering, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use storage::{Storage, WifiCredentials};
use wifi::AccessPoint;

mod discovery;
mod light_task;
mod network;
mod persistence;
mod provisioning;

/// Identifiants de secours compilés depuis `cfg.toml` (section `[light-flash]`),
/// utilisés quand la NVS ne contient rien. Voir `cfg.toml.example`.
#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
    #[default("light-flash")]
    ap_password: &'static str,
}

const HARDWARE: &str = "ESP32-C3-DevKit-RUST-1";

fn main() -> Result<()> {
    sys::link_patches();
    EspLogger::initialize_default();
    info!("light-flash {}", env!("CARGO_PKG_VERSION"));
    let boot = Instant::now();

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let storage: storage::SharedStorage = Arc::new(Mutex::new(Storage::new(nvs.clone())?));

    // Nom de la lampe (NVS, sinon light-flash-xxxx d'après l'adresse MAC) et nom d'hôte dérivé,
    // figés pour la durée du démarrage : un changement de nom passe par un redémarrage.
    let default_name = naming::default_name(station_mac());
    let display_name: &'static str = Box::leak(
        storage::lock(&storage)
            .device_name()?
            .unwrap_or_else(|| default_name.clone())
            .into_boxed_str(),
    );
    let hostname: &'static str =
        Box::leak(naming::hostname_from(display_name, &default_name).into_boxed_str());
    info!("nom : « {display_name} », hôte : {hostname}");

    // État initial : le dernier enregistré, sinon allumée en blanc chaud.
    let saved = storage::lock(&storage).light_state()?;
    let initial = match saved {
        Some(state) => {
            info!("état restauré depuis la NVS : {state}");
            state
        }
        None => {
            let state = LightState {
                power: true,
                ..LightState::default()
            };
            info!("aucun état enregistré, état par défaut : {state}");
            state
        }
    };
    let shared: SharedState = Arc::new(Mutex::new(initial));
    // Indication système (provisioning, identification) qui remplace temporairement le rendu.
    let indication: SharedIndication = Arc::new(Mutex::new(None));

    // Tâche lumière : seule propriétaire du driver LED.
    let led = WS2812RMT::new(peripherals.pins.gpio2, peripherals.rmt.channel0)?;
    light_task::spawn(led, shared.clone(), indication.clone())?;
    // Persistance de l'état après 2 s de calme.
    persistence::spawn(shared.clone(), storage.clone(), saved)?;

    // Réseau : station si des identifiants existent (NVS, sinon cfg.toml), point d'accès sinon ;
    // le thread réseau gère la reconnexion et le repli.
    let esp_wifi = wifi::new_wifi(peripherals.modem, sysloop.clone(), Some(nvs), hostname)?;
    let credentials = match storage::lock(&storage).wifi_credentials()? {
        Some(c) => {
            info!("identifiants Wi-Fi : NVS (« {} »)", c.ssid);
            Some(c)
        }
        None if !CONFIG.wifi_ssid.is_empty() => {
            info!("identifiants Wi-Fi : cfg.toml (« {} »)", CONFIG.wifi_ssid);
            Some(WifiCredentials {
                ssid: CONFIG.wifi_ssid.to_owned(),
                psk: CONFIG.wifi_psk.to_owned(),
            })
        }
        None => {
            info!("aucun identifiant Wi-Fi : point d'accès de secours");
            None
        }
    };
    let access_point = AccessPoint {
        ssid: hostname,
        password: CONFIG.ap_password,
        ..Default::default()
    };
    let network = network::spawn(esp_wifi, sysloop, credentials, access_point)?;

    // mDNS : http://<hôte>.local/ et annonce du service HTTP (découverte par Home Assistant,
    // navigateurs Bonjour). Répond sur l'interface station comme sur le point d'accès.
    let mut mdns = EspMdns::take()?;
    mdns.set_hostname(hostname)?;
    mdns.set_instance_name(display_name)?;
    mdns.add_service(
        Some(display_name),
        "_http",
        "_tcp",
        80,
        &[("path", "/"), discovery::TXT_MARKER, ("name", display_name)],
    )?;
    info!("mDNS : http://{hostname}.local/");
    // Découverte des autres lampes (mode groupe de la page) ; le thread garde `mdns`.
    let peers = discovery::spawn(mdns, hostname)?;

    // Provisioning Improv sur BLE : bouton BOOT (GPIO9, pull-up externe) pour autoriser.
    let mut button = PinDriver::input(peripherals.pins.gpio9)?;
    button.set_pull(Pull::Up)?;
    let provisioning = provisioning::spawn(
        button,
        storage.clone(),
        network.clone(),
        indication,
        provisioning::Config {
            device_name: display_name,
            hardware: HARDWARE,
            version: env!("CARGO_PKG_VERSION"),
            require_authorization: true,
        },
    )?;

    // Serveur HTTP : page de pilotage, API JSON, réception des identifiants Wi-Fi, actions.
    // Démarré avant que le réseau soit prêt : le socket d'écoute survit aux reconnexions.
    let on_wifi_credentials = {
        let storage = storage.clone();
        Box::new(move |ssid: &str, psk: &str| -> Result<()> {
            let creds = WifiCredentials::new(ssid, psk)?;
            storage::lock(&storage).set_wifi_credentials(&creds)?;
            schedule_restart();
            Ok(())
        })
    };
    let status = {
        let network = network.clone();
        let ble_active = provisioning.ble_active_flag();
        Box::new(move || {
            let net = network.status();
            StatusView {
                name: display_name.to_owned(),
                hostname: hostname.to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                hardware: HARDWARE.to_owned(),
                uptime_s: boot.elapsed().as_secs(),
                // SAFETY: lecture d'un compteur ESP-IDF, sans argument ni effet de bord.
                heap_free: unsafe { sys::esp_get_free_heap_size() },
                wifi_mode: match net.mode {
                    network::NetMode::Connecting => "connecting",
                    network::NetMode::Station => "station",
                    network::NetMode::AccessPoint => "access-point",
                }
                .to_owned(),
                ssid: net.ssid,
                ip: net.ip.map(|ip| ip.to_string()),
                rssi: net.rssi,
                ble_active: ble_active.load(Ordering::Relaxed),
                led_count: light_task::LED_COUNT,
                max_ma: light_task::MAX_MILLIAMPS,
            }
        })
    };
    let system = SystemHooks {
        restart: Box::new(schedule_restart),
        forget_wifi: {
            let storage = storage.clone();
            Box::new(move || {
                storage::lock(&storage).clear_wifi_credentials()?;
                schedule_restart();
                Ok(())
            })
        },
        ble_visible: {
            let authorize = provisioning.authorize_flag();
            Box::new(move || authorize.store(true, Ordering::Relaxed))
        },
        set_name: {
            let storage = storage.clone();
            Box::new(move |name: &str| {
                let name = naming::validate_name(name).map_err(anyhow::Error::msg)?;
                storage::lock(&storage).set_device_name(&name)?;
                schedule_restart();
                Ok(())
            })
        },
    };
    let debug = cfg!(debug_assertions).then(|| {
        let disconnect = network.disconnect_flag();
        let authorize = provisioning.authorize_flag();
        let ble_off = provisioning.ble_off_flag();
        DebugHooks {
            wifi_disconnect: Box::new(move || disconnect.store(true, Ordering::Relaxed)),
            improv_authorize: Box::new(move || authorize.store(true, Ordering::Relaxed)),
            ble_off: Box::new(move || ble_off.store(true, Ordering::Relaxed)),
        }
    });
    let peers_view = Box::new(move || {
        peers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|p| PeerView {
                name: p.name.clone(),
                hostname: p.hostname.clone(),
                ip: p.ip.to_string(),
                port: p.port,
            })
            .collect()
    });
    let _server = http_server::start(HttpContext {
        light: shared,
        on_wifi_credentials,
        status,
        peers: peers_view,
        system,
        debug,
    })?;

    loop {
        thread::sleep(Duration::from_secs(60));
        // SAFETY: lecture d'un compteur ESP-IDF, sans argument ni effet de bord.
        info!("tas libre : {} octets", unsafe {
            sys::esp_get_free_heap_size()
        });
    }
}

/// Adresse MAC de la station Wi-Fi (gravée en usine), base du nom par défaut.
fn station_mac() -> [u8; 6] {
    let mut mac = [0u8; 6];
    // SAFETY: tampon de 6 octets attendu par ESP-IDF pour ce type d'adresse.
    unsafe { sys::esp_read_mac(mac.as_mut_ptr(), sys::esp_mac_type_t_ESP_MAC_WIFI_STA) };
    mac
}

/// Laisse le temps à la réponse HTTP de partir, puis redémarre (nouveau réseau, nouveau nom).
fn schedule_restart() {
    thread::spawn(|| {
        thread::sleep(Duration::from_secs(2));
        info!("redémarrage");
        reset::restart();
    });
}
