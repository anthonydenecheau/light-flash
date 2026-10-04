//! Provisioning Wi-Fi par Improv sur BLE (protocole dans `crates/improv`). Ce thread possède la
//! pile BLE, le bouton BOOT et la machine à états ; il demande les connexions au thread réseau et
//! enregistre les identifiants acceptés. Un appui long sur BOOT (5 s) réinitialise la lampe.

use anyhow::{anyhow, Result};
use esp32_nimble::{
    utilities::{mutex::Mutex as BleMutex, BleUuid},
    uuid128, BLEAdvertisementData, BLEAdvertising, BLECharacteristic, BLEDevice, NimbleProperties,
};
use esp_idf_svc::hal::{
    gpio::{Gpio9, Input, PinDriver},
    reset,
};
use improv::{
    advertisement_service_data, capability, DeviceInfo, Error, Machine, Outcome, State,
    SERVICE_DATA_UUID16,
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
const CAPABILITIES: u8 = capability::IDENTIFY | capability::DEVICE_INFO;

pub struct Config {
    pub device_name: &'static str,
    pub hardware: &'static str,
    pub version: &'static str,
    /// Exiger un appui sur BOOT avant d'accepter des identifiants (recommandé).
    pub require_authorization: bool,
}

/// Poignée pour les autres threads (debug HTTP) : simuler un appui court sur BOOT.
#[derive(Clone, Default)]
pub struct ProvisioningHandle {
    authorize_request: Arc<AtomicBool>,
}

impl ProvisioningHandle {
    pub fn authorize_flag(&self) -> Arc<AtomicBool> {
        self.authorize_request.clone()
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
    let flag = handle.authorize_request.clone();
    thread::Builder::new()
        .name("provision".into())
        .stack_size(STACK_SIZE)
        .spawn(move || run(button, storage, network, indication, config, flag))?;
    Ok(handle)
}

/// Caractéristiques GATT et advertising du service Improv.
struct Ble {
    name: &'static str,
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
            advertising: device.get_advertising(),
            state,
            error,
            result,
            commands,
        })
    }

    fn publish_state(&self, state: State, error: Error) {
        self.state.lock().set_value(&[state as u8]).notify();
        self.error.lock().set_value(&[error as u8]).notify();
        self.advertise(state);
    }

    fn publish_error(&self, error: Error) {
        self.error.lock().set_value(&[error as u8]).notify();
    }

    fn publish_result(&self, packet: &[u8]) {
        self.result.lock().set_value(packet).notify();
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
    authorize_request: Arc<AtomicBool>,
) {
    let ble = match Ble::setup(config.device_name) {
        Ok(ble) => ble,
        Err(e) => {
            error!("BLE indisponible, provisioning Improv désactivé : {e}");
            return;
        }
    };
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
        "Improv BLE actif : « {} », autorisation par BOOT : {}",
        config.device_name, config.require_authorization
    );

    loop {
        thread::sleep(POLL);
        let now = now_ms();

        // Bouton BOOT : appui court = autorisation, appui long = réinitialisation d'usine.
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
        if authorize && machine.authorize(now) {
            info!("Improv : autorisation accordée pour 60 s");
            ble.publish_state(machine.state(), machine.error());
            indication_until = None;
            show(indication_for(machine.state()));
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
