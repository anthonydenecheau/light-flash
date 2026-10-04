//! Provisioning Wi-Fi par Improv sur BLE (protocole dans `crates/improv`). Ce thread possède la
//! pile BLE, le bouton BOOT et la machine à états ; il demande connexions et scans au thread
//! réseau et enregistre les identifiants acceptés. Un appui long sur BOOT (5 s) réinitialise la
//! lampe. Le BLE est arrêté cinq minutes après l'allumage, le dernier appui sur BOOT ou le dernier
//! provisioning, pour rendre sa mémoire ; un appui sur BOOT le rallume.

use anyhow::{anyhow, Result};
use esp32_nimble::{
    utilities::{mutex::Mutex as BleMutex, BleUuid},
    uuid128, BLEAdvertisementData, BLEAdvertising, BLECharacteristic, BLEDevice, NimbleProperties,
};
use esp_idf_svc::{
    hal::{
        gpio::{Gpio9, Input, PinDriver},
        reset,
    },
    sys::esp_get_free_heap_size,
};
use improv::{
    advertisement_service_data, capability, encode_scan_end, encode_scan_entry, DeviceInfo, Error,
    Machine, Outcome, State, SERVICE_DATA_UUID16,
};
use light_core::{
    button::{ButtonTracker, Press},
    Indication, SharedIndication,
};
use log::{error, info, warn};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use storage::{SharedStorage, WifiCredentials};

const POLL: Duration = Duration::from_millis(50);
/// NimBLE est initialisé depuis ce thread : pile confortable.
const STACK_SIZE: usize = 16 * 1024;
const CAPABILITIES: u8 = capability::IDENTIFY | capability::DEVICE_INFO | capability::SCAN_WIFI;
/// Le BLE reste actif ce temps après l'allumage, un appui sur BOOT ou un provisioning, puis est
/// arrêté pour rendre sa mémoire. Un appui sur BOOT le rallume.
const BLE_WINDOW_MS: u64 = 5 * 60 * 1000;
/// Pause entre deux notifications de résultat de scan, pour ne pas saturer le client.
const SCAN_NOTIFY_GAP: Duration = Duration::from_millis(30);

pub struct Config {
    pub device_name: &'static str,
    pub hardware: &'static str,
    pub version: &'static str,
    /// Exiger un appui sur BOOT avant d'accepter des identifiants (recommandé).
    pub require_authorization: bool,
}

/// Poignée pour les autres threads : simuler un appui court sur BOOT (page, debug), couper le
/// BLE (debug), savoir si le BLE est actif (page).
#[derive(Clone, Default)]
pub struct ProvisioningHandle {
    authorize_request: Arc<AtomicBool>,
    ble_off_request: Arc<AtomicBool>,
    ble_active: Arc<AtomicBool>,
}

impl ProvisioningHandle {
    pub fn authorize_flag(&self) -> Arc<AtomicBool> {
        self.authorize_request.clone()
    }

    pub fn ble_off_flag(&self) -> Arc<AtomicBool> {
        self.ble_off_request.clone()
    }

    pub fn ble_active_flag(&self) -> Arc<AtomicBool> {
        self.ble_active.clone()
    }
}

pub fn spawn(
    button: PinDriver<'static, Gpio9, Input>,
    storage: SharedStorage,
    network: crate::network::NetworkHandle,
    indication: SharedIndication,
    config: Config,
) -> Result<ProvisioningHandle> {
    let handle = ProvisioningHandle::default();
    let flags = handle.clone();
    thread::Builder::new()
        .name("provision".into())
        .stack_size(STACK_SIZE)
        .spawn(move || run(button, storage, network, indication, config, flags))?;
    Ok(handle)
}

/// Caractéristiques GATT et advertising du service Improv. Le GATT est créé une seule fois ;
/// la pile NimBLE peut ensuite être arrêtée et relancée (`disable` / `enable`), esp32-nimble
/// ré-enregistre les services au redémarrage.
struct Ble {
    name: &'static str,
    enabled: bool,
    advertising: &'static BleMutex<BLEAdvertising>,
    state: Arc<BleMutex<BLECharacteristic>>,
    error: Arc<BleMutex<BLECharacteristic>>,
    result: Arc<BleMutex<BLECharacteristic>>,
    /// Paquets écrits sur RPC Command, relayés depuis le callback NimBLE.
    commands: Receiver<Vec<u8>>,
}

impl Ble {
    fn setup(name: &'static str) -> Result<Self> {
        let device = BLEDevice::take();
        BLEDevice::set_device_name(name).map_err(|e| anyhow!("nom BLE : {e:?}"))?;
        let server = device.get_server();
        server.advertise_on_disconnect(true);

        let service = server.create_service(uuid128!("00467768-6228-2272-4663-277478268000"));
        let mut service = service.lock();
        let capabilities = service.create_characteristic(
            uuid128!("00467768-6228-2272-4663-277478268005"),
            NimbleProperties::READ,
        );
        capabilities.lock().set_value(&[CAPABILITIES]);
        let state = service.create_characteristic(
            uuid128!("00467768-6228-2272-4663-277478268001"),
            NimbleProperties::READ | NimbleProperties::NOTIFY,
        );
        let error = service.create_characteristic(
            uuid128!("00467768-6228-2272-4663-277478268002"),
            NimbleProperties::READ | NimbleProperties::NOTIFY,
        );
        let result = service.create_characteristic(
            uuid128!("00467768-6228-2272-4663-277478268004"),
            NimbleProperties::READ | NimbleProperties::NOTIFY,
        );
        let command = service.create_characteristic(
            uuid128!("00467768-6228-2272-4663-277478268003"),
            NimbleProperties::WRITE | NimbleProperties::WRITE_NO_RSP,
        );
        let (tx, commands) = mpsc::channel::<Vec<u8>>();
        let tx = Mutex::new(tx);
        // Callback dans la tâche hôte NimBLE : on relaie, rien de plus.
        command.lock().on_write(move |args| {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(args.recv_data().to_vec());
            }
        });

        Ok(Self {
            name,
            enabled: true,
            advertising: device.get_advertising(),
            state,
            error,
            result,
            commands,
        })
    }

    /// Relance la pile BLE après `disable` ; l'appelant republie l'état, ce qui relance
    /// l'advertising.
    fn enable(&mut self) {
        if self.enabled {
            return;
        }
        // `take()` seul ne suffit pas : l'initialisation paresseuse ne s'exécute qu'une fois,
        // `init()` relance explicitement la pile après un `deinit()`.
        BLEDevice::init();
        BLEDevice::take();
        if let Err(e) = BLEDevice::set_device_name(self.name) {
            warn!("nom BLE : {e:?}");
        }
        self.enabled = true;
        info!("BLE relancé");
    }

    /// Arrête l'advertising et la pile NimBLE pour rendre leur mémoire.
    fn disable(&mut self) {
        if !self.enabled {
            return;
        }
        let _ = self.advertising.lock().stop();
        match BLEDevice::deinit() {
            // SAFETY: lecture d'un compteur ESP-IDF, sans argument ni effet de bord.
            Ok(()) => info!("BLE arrêté, tas libre : {} octets", unsafe {
                esp_get_free_heap_size()
            }),
            Err(e) => warn!("arrêt du BLE : {e}"),
        }
        self.enabled = false;
    }

    fn publish_state(&self, state: State, error: Error) {
        if !self.enabled {
            return;
        }
        self.state.lock().set_value(&[state as u8]).notify();
        self.error.lock().set_value(&[error as u8]).notify();
        self.advertise(state);
    }

    fn publish_error(&self, error: Error) {
        if self.enabled {
            self.error.lock().set_value(&[error as u8]).notify();
        }
    }

    fn publish_result(&self, packet: &[u8]) {
        if self.enabled {
            self.result.lock().set_value(packet).notify();
        }
    }

    /// (Re)démarre l'advertising avec l'état courant dans le *service data* ; le nom passe dans
    /// la réponse de scan (le paquet d'advertising est limité à 31 octets).
    fn advertise(&self, state: State) {
        let mut data = BLEAdvertisementData::new();
        data.name(self.name)
            .add_service_uuid(uuid128!("00467768-6228-2272-4663-277478268000"));
        data.service_data(
            BleUuid::Uuid16(SERVICE_DATA_UUID16),
            &advertisement_service_data(state, CAPABILITIES),
        );
        let mut advertising = self.advertising.lock();
        let _ = advertising.stop();
        if let Err(e) = advertising.scan_response(true).set_data(&mut data) {
            warn!("advertising BLE : données refusées : {e:?}");
        }
        if let Err(e) = advertising.start() {
            warn!("advertising BLE : démarrage impossible : {e:?}");
        }
    }
}

fn run(
    button: PinDriver<'static, Gpio9, Input>,
    storage: SharedStorage,
    network: crate::network::NetworkHandle,
    indication: SharedIndication,
    config: Config,
    flags: ProvisioningHandle,
) {
    let ProvisioningHandle {
        authorize_request,
        ble_off_request,
        ble_active,
    } = flags;
    let mut ble = match Ble::setup(config.device_name) {
        Ok(ble) => ble,
        Err(e) => {
            error!("BLE indisponible, provisioning Improv désactivé : {e}");
            return;
        }
    };
    ble_active.store(true, Ordering::Relaxed);
    let device_info = DeviceInfo {
        firmware: "light-flash".into(),
        version: config.version.into(),
        hardware: config.hardware.into(),
        name: config.device_name.into(),
    };
    let mut machine = Machine::new(
        config.require_authorization,
        Machine::DEFAULT_AUTHORIZATION_TIMEOUT_MS,
    );
    let mut tracker = ButtonTracker::default();
    let mut indication_until: Option<u64> = None;
    let start = Instant::now();
    let now_ms = || start.elapsed().as_millis() as u64;
    let mut ble_until = BLE_WINDOW_MS;

    let show = |what: Option<Indication>| {
        *indication.lock().unwrap_or_else(|p| p.into_inner()) = what;
    };
    let indication_for = |state: State| match state {
        State::Authorized => Some(Indication::Authorized),
        State::Provisioning => Some(Indication::Provisioning),
        State::AuthorizationRequired | State::Provisioned => None,
    };

    ble.publish_state(machine.state(), machine.error());
    info!(
        "Improv BLE actif : « {} », autorisation par BOOT : {}, arrêt après {} min sans appui",
        config.device_name,
        config.require_authorization,
        BLE_WINDOW_MS / 60_000
    );

    loop {
        thread::sleep(POLL);
        let now = now_ms();

        // Bouton BOOT : appui court = autorisation (et BLE), appui long = réinitialisation d'usine.
        let press = tracker.update(button.is_low(), now);
        let authorize = press == Press::Short || authorize_request.swap(false, Ordering::Relaxed);
        if press == Press::Long {
            if let Err(e) = storage::lock(&storage).factory_reset() {
                error!("réinitialisation d'usine : {e}");
            }
            show(Some(Indication::Failure));
            thread::sleep(Duration::from_millis(500));
            reset::restart();
        }
        if authorize {
            ble_until = now + BLE_WINDOW_MS;
            if !ble.enabled {
                ble.enable();
                ble_active.store(true, Ordering::Relaxed);
                ble.publish_state(machine.state(), machine.error());
            }
            if machine.authorize(now) {
                info!("Improv : autorisation accordée pour 60 s");
                ble.publish_state(machine.state(), machine.error());
                indication_until = None;
                show(indication_for(machine.state()));
            }
        }

        let ble_off = ble_off_request.swap(false, Ordering::Relaxed);
        if ble.enabled && (ble_off || now >= ble_until) && machine.state() != State::Provisioning {
            ble.disable();
            ble_active.store(false, Ordering::Relaxed);
        }

        if machine.tick(now) {
            info!("Improv : autorisation expirée");
            ble.publish_state(machine.state(), machine.error());
            show(indication_for(machine.state()));
        }

        if indication_until.is_some_and(|until| now >= until) {
            indication_until = None;
            show(indication_for(machine.state()));
        }

        while let Ok(packet) = ble.commands.try_recv() {
            match machine.handle_packet(&packet, &device_info) {
                Outcome::Reject(e) => {
                    warn!("Improv : paquet refusé : {e:?}");
                    ble.publish_error(e);
                }
                Outcome::Identify => {
                    info!("Improv : identification");
                    show(Some(Indication::Identify));
                    indication_until = Some(now + 2_000);
                }
                Outcome::Reply(bytes) => ble.publish_result(&bytes),
                Outcome::ScanWifi => {
                    info!("Improv : scan des réseaux demandé");
                    match network.scan() {
                        Ok(entries) => {
                            for e in &entries {
                                ble.publish_result(&encode_scan_entry(&e.ssid, e.rssi, e.secured));
                                thread::sleep(SCAN_NOTIFY_GAP);
                            }
                        }
                        Err(e) => warn!("Improv : scan impossible : {e}"),
                    }
                    ble.publish_result(&encode_scan_end());
                }
                Outcome::StartProvisioning { ssid, password } => {
                    info!("Improv : identifiants reçus pour « {ssid} »");
                    ble.publish_state(State::Provisioning, Error::None);
                    show(Some(Indication::Provisioning));
                    let outcome = WifiCredentials::new(&ssid, &password)
                        .map_err(|e| e.to_string())
                        .and_then(|creds| network.connect(creds.clone()).map(|ip| (creds, ip)));
                    match outcome {
                        Ok((creds, ip)) => {
                            if let Err(e) = storage::lock(&storage).set_wifi_credentials(&creds) {
                                error!("enregistrement des identifiants : {e}");
                            }
                            let urls = [
                                format!("http://{}.local/", config.device_name),
                                format!("http://{}/", ip.ip),
                            ];
                            let result = machine.provisioning_succeeded(&[&urls[0], &urls[1]]);
                            ble.publish_result(&result);
                            ble.publish_state(machine.state(), machine.error());
                            info!("Improv : provisionnée, {}", urls[1]);
                            show(Some(Indication::Success));
                            ble_until = now_ms() + BLE_WINDOW_MS;
                        }
                        Err(e) => {
                            warn!("Improv : connexion impossible : {e}");
                            machine.provisioning_failed(now_ms());
                            ble.publish_state(machine.state(), machine.error());
                            show(Some(Indication::Failure));
                        }
                    }
                    indication_until = Some(now_ms() + 2_000);
                }
            }
        }
    }
}
