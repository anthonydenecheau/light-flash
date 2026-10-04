//! Firmware de l'applique : tâche lumière, Wi-Fi (station ou point d'accès de secours)
//! et serveur HTTP de pilotage.

use anyhow::Result;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{peripherals::Peripherals, reset},
    log::EspLogger,
    nvs::EspDefaultNvsPartition,
    sys,
};
use http_server::DebugHooks;
use light_core::{LightState, SharedState};
use log::info;
use rgb_led::WS2812RMT;
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use storage::{Storage, WifiCredentials};
use wifi::AccessPoint;

mod light_task;
mod network;
mod persistence;

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

/// Nom du point d'accès de secours.
const AP_SSID: &str = "light-flash";

fn main() -> Result<()> {
    sys::link_patches();
    EspLogger::initialize_default();
    info!("light-flash {}", env!("CARGO_PKG_VERSION"));

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let storage: storage::SharedStorage = Arc::new(Mutex::new(Storage::new(nvs.clone())?));

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

    // Tâche lumière : seule propriétaire du driver LED.
    let led = WS2812RMT::new(peripherals.pins.gpio2, peripherals.rmt.channel0)?;
    light_task::spawn(led, shared.clone())?;
    // Persistance de l'état après 2 s de calme.
    persistence::spawn(shared.clone(), storage.clone(), saved)?;

    // Réseau : station si des identifiants existent (NVS, sinon cfg.toml), point d'accès sinon ;
    // le thread réseau gère la reconnexion et le repli.
    let esp_wifi = wifi::new_wifi(peripherals.modem, sysloop.clone(), Some(nvs))?;
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
        ssid: AP_SSID,
        password: CONFIG.ap_password,
        ..Default::default()
    };
    let network = network::spawn(esp_wifi, sysloop, credentials, access_point)?;

    // Serveur HTTP : page de pilotage, API JSON, réception des identifiants Wi-Fi.
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
    let debug_hooks = cfg!(debug_assertions).then(|| DebugHooks {
        wifi_disconnect: Box::new(move || network.request_disconnect()),
    });
    let _server = http_server::start(shared, on_wifi_credentials, debug_hooks)?;

    loop {
        thread::sleep(Duration::from_secs(60));
        // SAFETY: lecture d'un compteur ESP-IDF, sans argument ni effet de bord.
        info!("tas libre : {} octets", unsafe {
            sys::esp_get_free_heap_size()
        });
    }
}

/// Laisse le temps à la réponse HTTP de partir, puis redémarre pour appliquer le nouveau réseau.
fn schedule_restart() {
    thread::spawn(|| {
        thread::sleep(Duration::from_secs(2));
        info!("redémarrage");
        reset::restart();
    });
}
