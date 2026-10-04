//! Thread réseau : possède le driver Wi-Fi, applique la politique de reconnexion
//! (`light_core::reconnect::Policy`), exécute les demandes du provisioning (connexion, scan) et
//! journalise les événements. La station est retentée avec un délai croissant ; après 90 s sans
//! connexion, le point d'accès de secours est démarré et la station retentée toutes les 2 minutes.

use anyhow::Result;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    ipv4::IpInfo,
    wifi::{EspWifi, WifiEvent},
};
use light_core::reconnect::{Action, Mode, Policy, Settings};
use log::{error, info, warn};
use std::{
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use storage::WifiCredentials;
use wifi::{AccessPoint, ScanEntry};

const TICK: Duration = Duration::from_secs(1);
const STACK_SIZE: usize = 12 * 1024;
/// Une tentative de connexion dure au plus ~17 s, plus l'attente d'un tick ou d'une tentative
/// en cours : au-delà, le thread réseau est considéré bloqué.
const CONNECT_REPLY_TIMEOUT: Duration = Duration::from_secs(45);
/// Nombre maximal de réseaux renvoyés par un scan.
const SCAN_MAX: usize = 20;
const SCAN_REPLY_TIMEOUT: Duration = Duration::from_secs(25);

/// Commandes des autres threads vers le thread réseau.
pub enum NetworkCommand {
    /// Tenter la station avec ces identifiants (provisioning) ; la réponse arrive sur `reply`.
    Connect {
        credentials: WifiCredentials,
        reply: Sender<Result<IpInfo, String>>,
    },
    /// Lister les réseaux à portée (provisioning).
    Scan {
        reply: Sender<Result<Vec<ScanEntry>, String>>,
    },
}

/// Situation réseau, lisible par les autres threads (page de pilotage).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkStatus {
    pub mode: NetMode,
    pub ssid: String,
    pub ip: Option<Ipv4Addr>,
    pub rssi: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NetMode {
    #[default]
    Connecting,
    Station,
    AccessPoint,
}

/// Poignée pour les autres threads.
#[derive(Clone)]
pub struct NetworkHandle {
    disconnect_request: Arc<AtomicBool>,
    commands: Sender<NetworkCommand>,
    status: Arc<Mutex<NetworkStatus>>,
}

impl NetworkHandle {
    pub fn status(&self) -> NetworkStatus {
        self.status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Drapeau partageable (debug HTTP) : une déconnexion de la station au prochain tick.
    pub fn disconnect_flag(&self) -> Arc<AtomicBool> {
        self.disconnect_request.clone()
    }

    /// Tente la station avec ces identifiants et attend le résultat. En cas de succès, le thread
    /// réseau les adopte pour ses reconnexions ; sinon il revient à sa situation précédente.
    pub fn connect(&self, credentials: WifiCredentials) -> Result<IpInfo, String> {
        let (reply, result) = mpsc::channel();
        self.commands
            .send(NetworkCommand::Connect { credentials, reply })
            .map_err(|_| "thread réseau arrêté".to_owned())?;
        result
            .recv_timeout(CONNECT_REPLY_TIMEOUT)
            .map_err(|_| "pas de réponse du thread réseau".to_owned())?
    }

    /// Réseaux à portée, du plus fort au plus faible (scan bloquant de quelques secondes).
    pub fn scan(&self) -> Result<Vec<ScanEntry>, String> {
        let (reply, result) = mpsc::channel();
        self.commands
            .send(NetworkCommand::Scan { reply })
            .map_err(|_| "thread réseau arrêté".to_owned())?;
        result
            .recv_timeout(SCAN_REPLY_TIMEOUT)
            .map_err(|_| "pas de réponse du thread réseau".to_owned())?
    }
}

pub fn spawn(
    wifi: EspWifi<'static>,
    sysloop: EspSystemEventLoop,
    credentials: Option<WifiCredentials>,
    access_point: AccessPoint<'static>,
) -> Result<NetworkHandle> {
    let (commands, inbox) = mpsc::channel();
    let handle = NetworkHandle {
        disconnect_request: Arc::new(AtomicBool::new(false)),
        commands,
        status: Arc::new(Mutex::new(NetworkStatus::default())),
    };
    let flag = handle.disconnect_request.clone();
    let status = handle.status.clone();
    thread::Builder::new()
        .name("network".into())
        .stack_size(STACK_SIZE)
        .spawn(move || {
            run(
                wifi,
                sysloop,
                credentials,
                access_point,
                flag,
                inbox,
                status,
            )
        })?;
    Ok(handle)
}

fn run(
    mut wifi: EspWifi<'static>,
    sysloop: EspSystemEventLoop,
    mut credentials: Option<WifiCredentials>,
    ap: AccessPoint<'static>,
    disconnect_request: Arc<AtomicBool>,
    inbox: Receiver<NetworkCommand>,
    status: Arc<Mutex<NetworkStatus>>,
) {
    let publish = |s: NetworkStatus| *status.lock().unwrap_or_else(|p| p.into_inner()) = s;
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
        while let Ok(command) = inbox.try_recv() {
            match command {
                NetworkCommand::Connect {
                    credentials: new_credentials,
                    reply,
                } => {
                    info!("provisioning : essai de « {} »", new_credentials.ssid);
                    let result = wifi::connect_sta(
                        &mut wifi,
                        sysloop.clone(),
                        &new_credentials.ssid,
                        &new_credentials.psk,
                    );
                    let now = now_ms();
                    match &result {
                        Ok(ip) => {
                            info!("page de pilotage : http://{}/", ip.ip);
                            publish(NetworkStatus {
                                mode: NetMode::Station,
                                ssid: new_credentials.ssid.clone(),
                                ip: Some(ip.ip),
                                rssi: wifi.get_rssi().ok(),
                            });
                            credentials = Some(new_credentials);
                            policy.set_has_credentials(true);
                            policy.on_station_result(now, true);
                        }
                        Err(e) => {
                            warn!(
                                "provisioning : « {} » injoignable : {e}",
                                new_credentials.ssid
                            );
                            if policy.on_station_result(now, false) == Action::StartAccessPoint {
                                if let Some(s) = start_access_point(&mut wifi, &sysloop, &ap) {
                                    publish(s);
                                }
                            }
                        }
                    }
                    let _ = reply.send(result.map_err(|e| e.to_string()));
                }
                NetworkCommand::Scan { reply } => {
                    let result = wifi::scan(&mut wifi, SCAN_MAX).map_err(|e| e.to_string());
                    if let Err(e) = &result {
                        warn!("scan Wi-Fi impossible : {e}");
                    }
                    let _ = reply.send(result);
                }
            }
        }

        if disconnect_request.swap(false, Ordering::Relaxed) {
            if let Err(e) = wifi.disconnect() {
                warn!("déconnexion forcée impossible : {e}");
            }
        }
        let connected = policy.mode() == Mode::Station && wifi.is_connected().unwrap_or(false);
        if policy.mode() == Mode::Station {
            let mut s = status.lock().unwrap_or_else(|p| p.into_inner());
            if connected {
                s.rssi = wifi.get_rssi().ok();
            } else if s.mode != NetMode::Connecting {
                s.mode = NetMode::Connecting;
                s.ip = None;
                s.rssi = None;
            }
        }
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
                        publish(NetworkStatus {
                            mode: NetMode::Station,
                            ssid: c.ssid.clone(),
                            ip: Some(ip.ip),
                            rssi: wifi.get_rssi().ok(),
                        });
                        policy.on_station_result(now_ms(), true)
                    }
                    Err(e) => {
                        warn!("connexion à « {} » échouée : {e}", c.ssid);
                        policy.on_station_result(now_ms(), false)
                    }
                };
                if follow_up == Action::StartAccessPoint {
                    if let Some(s) = start_access_point(&mut wifi, &sysloop, &ap) {
                        publish(s);
                    }
                }
            }
            Action::StartAccessPoint => {
                if let Some(s) = start_access_point(&mut wifi, &sysloop, &ap) {
                    publish(s);
                }
            }
        }
        thread::sleep(TICK);
    }
}

fn start_access_point(
    wifi: &mut EspWifi<'static>,
    sysloop: &EspSystemEventLoop,
    ap: &AccessPoint,
) -> Option<NetworkStatus> {
    match wifi::start_access_point(wifi, sysloop.clone(), ap) {
        Ok(ip) => {
            info!(
                "point d'accès « {} » : s'y connecter puis ouvrir http://{}/",
                ap.ssid, ip.ip
            );
            Some(NetworkStatus {
                mode: NetMode::AccessPoint,
                ssid: ap.ssid.to_owned(),
                ip: Some(ip.ip),
                rssi: None,
            })
        }
        Err(e) => {
            error!("démarrage du point d'accès : {e}");
            None
        }
    }
}
