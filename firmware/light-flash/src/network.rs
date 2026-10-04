//! Thread réseau : possède le driver Wi-Fi, applique la politique de reconnexion
//! (`light_core::reconnect::Policy`) et journalise les événements. La station est retentée avec
//! un délai croissant ; après 90 s sans connexion, le point d'accès de secours est démarré et la
//! station est retentée toutes les 2 minutes.

use anyhow::Result;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    wifi::{EspWifi, WifiEvent},
};
use light_core::reconnect::{Action, Mode, Policy, Settings};
use log::{error, info, warn};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use storage::WifiCredentials;
use wifi::AccessPoint;

const TICK: Duration = Duration::from_secs(1);
const STACK_SIZE: usize = 12 * 1024;

/// Poignée pour les autres threads : demander une déconnexion (debug).
#[derive(Clone, Default)]
pub struct NetworkHandle {
    disconnect_request: Arc<AtomicBool>,
}

impl NetworkHandle {
    pub fn request_disconnect(&self) {
        self.disconnect_request.store(true, Ordering::Relaxed);
    }
}

pub fn spawn(
    wifi: EspWifi<'static>,
    sysloop: EspSystemEventLoop,
    credentials: Option<WifiCredentials>,
    access_point: AccessPoint<'static>,
) -> Result<NetworkHandle> {
    let handle = NetworkHandle::default();
    let flag = handle.disconnect_request.clone();
    thread::Builder::new()
        .name("network".into())
        .stack_size(STACK_SIZE)
        .spawn(move || run(wifi, sysloop, credentials, access_point, flag))?;
    Ok(handle)
}

fn run(
    mut wifi: EspWifi<'static>,
    sysloop: EspSystemEventLoop,
    credentials: Option<WifiCredentials>,
    ap: AccessPoint<'static>,
    disconnect_request: Arc<AtomicBool>,
) {
    // Journal des événements station ; le callback tourne dans la tâche événements : court.
    let subscription = sysloop.subscribe::<WifiEvent, _>(|event| match event {
        WifiEvent::StaDisconnected(d) => warn!("Wi-Fi station déconnectée : {d:?}"),
        WifiEvent::StaConnected(_) => info!("Wi-Fi station associée"),
        _ => {}
    });
    if let Err(e) = &subscription {
        warn!("abonnement aux événements Wi-Fi impossible : {e}");
    }

    let mut policy = Policy::new(credentials.is_some(), Settings::default());
    let start = Instant::now();
    let now_ms = || start.elapsed().as_millis() as u64;

    loop {
        if disconnect_request.swap(false, Ordering::Relaxed) {
            if let Err(e) = wifi.disconnect() {
                warn!("déconnexion forcée impossible : {e}");
            }
        }
        let connected = policy.mode() == Mode::Station && wifi.is_connected().unwrap_or(false);
        match policy.on_tick(now_ms(), connected) {
            Action::Wait => {}
            Action::ConnectStation => {
                let c = credentials
                    .as_ref()
                    .expect("ConnectStation n'est rendu qu'avec des identifiants");
                let result = wifi::connect_sta(&mut wifi, sysloop.clone(), &c.ssid, &c.psk);
                let follow_up = match result {
                    Ok(ip) => {
                        info!("page de pilotage : http://{}/", ip.ip);
                        policy.on_station_result(now_ms(), true)
                    }
                    Err(e) => {
                        warn!("connexion à « {} » échouée : {e}", c.ssid);
                        policy.on_station_result(now_ms(), false)
                    }
                };
                if follow_up == Action::StartAccessPoint {
                    start_access_point(&mut wifi, &sysloop, &ap);
                }
            }
            Action::StartAccessPoint => start_access_point(&mut wifi, &sysloop, &ap),
        }
        thread::sleep(TICK);
    }
}

fn start_access_point(wifi: &mut EspWifi<'static>, sysloop: &EspSystemEventLoop, ap: &AccessPoint) {
    match wifi::start_access_point(wifi, sysloop.clone(), ap) {
        Ok(ip) => info!(
            "point d'accès « {} » : s'y connecter puis ouvrir http://{}/",
            ap.ssid, ip.ip
        ),
        Err(e) => error!("démarrage du point d'accès : {e}"),
    }
}
