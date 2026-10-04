//! Thread réseau : possède le driver Wi-Fi, applique la politique de reconnexion
//! (`light_core::reconnect::Policy`), exécute les demandes de connexion du provisioning et
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
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use storage::WifiCredentials;
use wifi::AccessPoint;

const TICK: Duration = Duration::from_secs(1);
const STACK_SIZE: usize = 12 * 1024;
/// Une tentative de connexion dure au plus ~17 s, plus l'attente d'un tick ou d'une tentative
/// en cours : au-delà, le thread réseau est considéré bloqué.
const CONNECT_REPLY_TIMEOUT: Duration = Duration::from_secs(45);

/// Commandes des autres threads vers le thread réseau.
pub enum NetworkCommand {
    /// Tenter la station avec ces identifiants (provisioning) ; la réponse arrive sur `reply`.
    Connect {
        credentials: WifiCredentials,
        reply: Sender<Result<IpInfo, String>>,
    },
}

/// Poignée pour les autres threads.
#[derive(Clone)]
pub struct NetworkHandle {
    disconnect_request: Arc<AtomicBool>,
    commands: Sender<NetworkCommand>,
}

impl NetworkHandle {
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
    };
    let flag = handle.disconnect_request.clone();
    thread::Builder::new()
        .name("network".into())
        .stack_size(STACK_SIZE)
        .spawn(move || run(wifi, sysloop, credentials, access_point, flag, inbox))?;
    Ok(handle)
}

fn run(
    mut wifi: EspWifi<'static>,
    sysloop: EspSystemEventLoop,
    mut credentials: Option<WifiCredentials>,
    ap: AccessPoint<'static>,
    disconnect_request: Arc<AtomicBool>,
    inbox: Receiver<NetworkCommand>,
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
        while let Ok(NetworkCommand::Connect {
            credentials: new_credentials,
            reply,
        }) = inbox.try_recv()
        {
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
                        start_access_point(&mut wifi, &sysloop, &ap);
                    }
                }
            }
            let _ = reply.send(result.map_err(|e| e.to_string()));
        }

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
