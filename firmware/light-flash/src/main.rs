//! Firmware de l'applique : tâche lumière, Wi-Fi (station ou point d'accès de secours)
//! et serveur HTTP de pilotage.

use anyhow::Result;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{peripherals::Peripherals, reset},
    log::EspLogger,
    nvs::EspDefaultNvsPartition,
    sys,
    wifi::EspWifi,
};
use light_core::{LightState, SharedState};
use log::{info, warn};
use rgb_led::WS2812RMT;
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use storage::{Storage, WifiCredentials};
use wifi::AccessPoint;

mod light_task;

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
    let storage = Arc::new(Mutex::new(Storage::new(nvs.clone())?));

    // Tâche lumière : seule propriétaire du driver LED. La lampe démarre allumée.
    let shared: SharedState = Arc::new(Mutex::new(LightState {
        power: true,
        ..LightState::default()
    }));
    let led = WS2812RMT::new(peripherals.pins.gpio2, peripherals.rmt.channel0)?;
    light_task::spawn(led, shared.clone())?;

    // Réseau : station si des identifiants existent (NVS, sinon cfg.toml), point d'accès sinon.
    let mut esp_wifi = wifi::new_wifi(peripherals.modem, sysloop.clone(), Some(nvs))?;
    let credentials = match lock(&storage).wifi_credentials()? {
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
    bring_up_network(&mut esp_wifi, sysloop, credentials.as_ref())?;

    // Serveur HTTP : page de pilotage, API JSON, réception des identifiants Wi-Fi.
    let on_wifi_credentials = {
        let storage = storage.clone();
        Box::new(move |ssid: &str, psk: &str| -> Result<()> {
            let creds = WifiCredentials::new(ssid, psk)?;
            lock(&storage).set_wifi_credentials(&creds)?;
            schedule_restart();
            Ok(())
        })
    };
    let _server = http_server::start(shared, on_wifi_credentials)?;

    loop {
        thread::sleep(Duration::from_secs(60));
        // SAFETY: lecture d'un compteur ESP-IDF, sans argument ni effet de bord.
        info!("tas libre : {} octets", unsafe {
            sys::esp_get_free_heap_size()
        });
    }
}

fn lock(storage: &Mutex<Storage>) -> std::sync::MutexGuard<'_, Storage> {
    storage
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Station avec les identifiants fournis ; en cas d'échec ou sans identifiants, point d'accès.
fn bring_up_network(
    esp_wifi: &mut EspWifi<'static>,
    sysloop: EspSystemEventLoop,
    credentials: Option<&WifiCredentials>,
) -> Result<()> {
    if let Some(c) = credentials {
        match wifi::connect_sta(esp_wifi, sysloop.clone(), &c.ssid, &c.psk) {
            Ok(ip) => {
                info!("page de pilotage : http://{}/", ip.ip);
                return Ok(());
            }
            Err(e) => warn!(
                "connexion à « {} » impossible ({e}) : repli sur le point d'accès",
                c.ssid
            ),
        }
    }
    let ap = AccessPoint {
        ssid: AP_SSID,
        password: CONFIG.ap_password,
        ..Default::default()
    };
    let ip = wifi::start_access_point(esp_wifi, sysloop, &ap)?;
    info!(
        "point d'accès « {AP_SSID} » : s'y connecter puis ouvrir http://{}/",
        ip.ip
    );
    Ok(())
}

/// Laisse le temps à la réponse HTTP de partir, puis redémarre pour appliquer le nouveau réseau.
fn schedule_restart() {
    thread::spawn(|| {
        thread::sleep(Duration::from_secs(2));
        info!("redémarrage");
        reset::restart();
    });
}
