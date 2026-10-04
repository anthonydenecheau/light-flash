//! Mise à jour du firmware depuis un serveur HTTP local : `manifest.json` (version, url, sha256,
//! notes) et image application `.bin` (produits par `make publish`). Le téléchargement se fait en
//! flux dans l'emplacement OTA inactif (`EspOta`), le SHA-256 est vérifié avant validation, puis
//! la lampe redémarre ; au démarrage suivant, le firmware se déclare valide une fois le réseau
//! opérationnel, sinon le bootloader revient à l'image précédente.

use anyhow::{anyhow, bail, Context, Result};
use esp_idf_svc::{hal::reset, ota::EspOta};
use light_core::version;
use log::{error, info, warn};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::{
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use storage::SharedStorage;

/// Vérification périodique du manifeste.
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
/// Première vérification après le démarrage (le temps que le réseau monte).
const FIRST_CHECK: Duration = Duration::from_secs(25);
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);
const CHUNK: usize = 4096;
const MANIFEST_MAX: usize = 4096;
const STACK_SIZE: usize = 12 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Checking,
    Downloading,
    Verifying,
    Rebooting,
    Failed,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Checking => "checking",
            Phase::Downloading => "downloading",
            Phase::Verifying => "verifying",
            Phase::Rebooting => "rebooting",
            Phase::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateState {
    pub current: &'static str,
    /// Adresse de base du serveur (sans `/manifest.json`), vide = désactivé.
    pub url: String,
    pub available: Option<Manifest>,
    pub phase: Phase,
    /// Progression du téléchargement, 0 à 100.
    pub progress: u8,
    pub error: Option<String>,
    /// Instant de la dernière vérification, en secondes depuis le démarrage.
    pub last_check_s: Option<u64>,
    pub running_slot: String,
}

pub type SharedUpdate = Arc<Mutex<UpdateState>>;

enum Command {
    Check,
    Install,
}

#[derive(Clone)]
pub struct UpdateHandle {
    state: SharedUpdate,
    commands: Sender<Command>,
    storage: SharedStorage,
}

impl UpdateHandle {
    pub fn state(&self) -> UpdateState {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn check(&self) {
        let _ = self.commands.send(Command::Check);
    }

    pub fn install(&self) -> Result<()> {
        let s = self.state();
        if s.available.is_none() {
            bail!("aucune mise à jour disponible");
        }
        if matches!(
            s.phase,
            Phase::Downloading | Phase::Verifying | Phase::Rebooting
        ) {
            bail!("mise à jour déjà en cours");
        }
        self.commands
            .send(Command::Install)
            .map_err(|_| anyhow!("thread de mise à jour arrêté"))
    }

    /// Enregistre l'adresse du serveur (NVS) et relance une vérification.
    pub fn set_url(&self, url: &str) -> Result<()> {
        let url = url.trim().trim_end_matches('/').to_owned();
        if !url.is_empty() && !url.starts_with("http://") {
            bail!("l'adresse doit commencer par http:// (pas de TLS sur la lampe)");
        }
        storage::lock(&self.storage).set_update_url(&url)?;
        {
            let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
            s.url = url;
            s.available = None;
            s.error = None;
        }
        self.check();
        Ok(())
    }
}

pub fn spawn(
    storage: SharedStorage,
    initial_url: String,
    current: &'static str,
    boot: Instant,
) -> Result<UpdateHandle> {
    let running_slot = EspOta::new()
        .and_then(|ota| ota.get_running_slot())
        .map(|slot| slot.label.to_string())
        .unwrap_or_else(|_| "?".to_owned());
    let state: SharedUpdate = Arc::new(Mutex::new(UpdateState {
        current,
        url: initial_url,
        available: None,
        phase: Phase::Idle,
        progress: 0,
        error: None,
        last_check_s: None,
        running_slot,
    }));
    let (commands, inbox) = mpsc::channel();
    let handle = UpdateHandle {
        state: state.clone(),
        commands,
        storage,
    };
    thread::Builder::new()
        .name("update".into())
        .stack_size(STACK_SIZE)
        .spawn(move || run(state, inbox, boot))?;
    Ok(handle)
}

/// À appeler une fois le réseau opérationnel après un démarrage : confirme l'image en cours
/// (sans effet si elle était déjà validée).
pub fn mark_running_valid() {
    match EspOta::new().and_then(|mut ota| ota.mark_running_slot_valid()) {
        Ok(()) => info!("image en cours confirmée (pas de retour arrière)"),
        Err(e) => warn!("confirmation de l'image : {e}"),
    }
}

fn run(state: SharedUpdate, inbox: Receiver<Command>, boot: Instant) {
    let mut wait = FIRST_CHECK;
    loop {
        let command = match inbox.recv_timeout(wait) {
            Ok(c) => c,
            Err(RecvTimeoutError::Timeout) => Command::Check,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        wait = CHECK_INTERVAL;
        match command {
            Command::Check => check(&state, boot),
            Command::Install => install(&state),
        }
    }
}

fn set<F: FnOnce(&mut UpdateState)>(state: &SharedUpdate, f: F) {
    f(&mut state.lock().unwrap_or_else(|p| p.into_inner()));
}

fn check(state: &SharedUpdate, boot: Instant) {
    let (url, current) = {
        let s = state.lock().unwrap_or_else(|p| p.into_inner());
        (s.url.clone(), s.current)
    };
    if url.is_empty() {
        set(state, |s| {
            s.available = None;
            s.phase = Phase::Idle;
        });
        return;
    }
    set(state, |s| {
        s.phase = Phase::Checking;
        s.error = None;
    });
    let result = fetch_manifest(&format!("{url}/manifest.json"));
    let now = boot.elapsed().as_secs();
    match result {
        Ok(m) if version::is_newer(&m.version, current) => {
            info!(
                "mise à jour disponible : {} (actuelle {current})",
                m.version
            );
            set(state, |s| {
                s.available = Some(m);
                s.phase = Phase::Idle;
                s.last_check_s = Some(now);
            });
        }
        Ok(m) => {
            info!("firmware à jour ({current}, publié : {})", m.version);
            set(state, |s| {
                s.available = None;
                s.phase = Phase::Idle;
                s.last_check_s = Some(now);
            });
        }
        Err(e) => {
            warn!("vérification des mises à jour : {e:#}");
            set(state, |s| {
                s.phase = Phase::Idle;
                s.error = Some(format!("vérification impossible : {e:#}"));
                s.last_check_s = Some(now);
            });
        }
    }
}

/// Réponse d'un GET HTTP/1.1 minimal : le corps se lit en flux sur `stream`.
struct HttpBody {
    stream: TcpStream,
    status: u16,
    content_length: Option<u64>,
    /// Octets du corps déjà reçus avec les en-têtes.
    head: Vec<u8>,
}

impl HttpBody {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.head.is_empty() {
            let n = self.head.len().min(buf.len());
            buf[..n].copy_from_slice(&self.head[..n]);
            self.head.drain(..n);
            return Ok(n);
        }
        self.stream.read(buf)
    }
}

/// GET en HTTP/1.1 sans TLS sur une socket de la bibliothèque standard : évite d'embarquer le
/// client HTTP d'ESP-IDF et mbedTLS (≈ 300 Ko d'image) pour un serveur du réseau local.
fn http_get(url: &str) -> Result<HttpBody> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow!("adresse non http:// : {url}"))?;
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let authority = if host_port.contains(':') {
        host_port.to_owned()
    } else {
        format!("{host_port}:80")
    };
    let addr = authority
        .to_socket_addrs()
        .with_context(|| format!("résolution de {host_port}"))?
        .next()
        .ok_or_else(|| anyhow!("aucune adresse pour {host_port}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, HTTP_TIMEOUT)
        .with_context(|| format!("connexion à {addr}"))?;
    stream.set_read_timeout(Some(HTTP_TIMEOUT))?;
    stream.set_write_timeout(Some(HTTP_TIMEOUT))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\nUser-Agent: light-flash\r\n\r\n"
    )?;

    // En-têtes : lus jusqu'à la ligne vide, 2 Ko maximum.
    let mut head = Vec::with_capacity(512);
    let mut chunk = [0u8; 256];
    let header_end = loop {
        if let Some(i) = head.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if head.len() > 2048 {
            bail!("en-têtes HTTP trop longs");
        }
        let n = stream.read(&mut chunk).context("lecture des en-têtes")?;
        if n == 0 {
            bail!("connexion fermée avant la fin des en-têtes");
        }
        head.extend_from_slice(&chunk[..n]);
    };
    let headers = String::from_utf8_lossy(&head[..header_end]).into_owned();
    let mut lines = headers.lines();
    let status_line = lines.next().unwrap_or_default();
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("ligne de statut invalide : {status_line}"))?;
    let mut content_length = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim();
            if k == "content-length" {
                content_length = v.parse::<u64>().ok();
            } else if k == "transfer-encoding" && v.to_ascii_lowercase().contains("chunked") {
                bail!(
                    "encodage chunked non pris en charge : servir les fichiers avec Content-Length"
                );
            }
        }
    }
    head.drain(..header_end);
    Ok(HttpBody {
        stream,
        status,
        content_length,
        head,
    })
}

fn fetch_manifest(url: &str) -> Result<Manifest> {
    let mut response = http_get(url)?;
    if response.status != 200 {
        bail!("le serveur répond {} pour {url}", response.status);
    }
    let mut body = Vec::new();
    let mut buf = [0u8; 512];
    loop {
        let n = response.read(&mut buf).context("lecture du manifeste")?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
        if body.len() > MANIFEST_MAX {
            bail!("manifeste trop long");
        }
    }
    let manifest: Manifest = serde_json::from_slice(&body).context("manifeste JSON invalide")?;
    if manifest.sha256.len() != 64 || !manifest.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("empreinte SHA-256 absente ou invalide dans le manifeste");
    }
    Ok(manifest)
}

fn install(state: &SharedUpdate) {
    let Some(manifest) = state
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .available
        .clone()
    else {
        return;
    };
    info!(
        "mise à jour vers {} depuis {}",
        manifest.version, manifest.url
    );
    set(state, |s| {
        s.phase = Phase::Downloading;
        s.progress = 0;
        s.error = None;
    });
    match download_and_apply(&manifest, state) {
        Ok(()) => {
            info!("mise à jour écrite et vérifiée, redémarrage");
            set(state, |s| s.phase = Phase::Rebooting);
            thread::sleep(Duration::from_secs(2));
            reset::restart();
        }
        Err(e) => {
            error!("mise à jour échouée : {e:#}");
            set(state, |s| {
                s.phase = Phase::Failed;
                s.error = Some(format!("{e:#}"));
            });
        }
    }
}

fn download_and_apply(manifest: &Manifest, state: &SharedUpdate) -> Result<()> {
    let mut response = http_get(&manifest.url)?;
    if response.status != 200 {
        bail!("le serveur répond {} pour l'image", response.status);
    }
    let total = response.content_length.or(if manifest.size > 0 {
        Some(manifest.size)
    } else {
        None
    });

    let mut ota = EspOta::new().context("OTA indisponible (table de partitions ?)")?;
    let mut update = ota
        .initiate_update()
        .context("ouverture de l'emplacement OTA")?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut received: u64 = 0;
    let mut last_pct = 0u8;
    loop {
        let n = response.read(&mut buf).context("téléchargement")?;
        if n == 0 {
            break;
        }
        if let Err(e) = update.write(&buf[..n]) {
            let _ = update.abort();
            return Err(anyhow!("écriture en flash : {e}"));
        }
        hasher.update(&buf[..n]);
        received += n as u64;
        if let Some(total) = total {
            let pct = (received * 100 / total.max(1)).min(99) as u8;
            if pct != last_pct {
                last_pct = pct;
                set(state, |s| s.progress = pct);
            }
        }
    }
    set(state, |s| {
        s.phase = Phase::Verifying;
        s.progress = 99;
    });
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest.iter() {
        let _ = core::fmt::Write::write_fmt(&mut hex, format_args!("{b:02x}"));
    }
    if !hex.eq_ignore_ascii_case(&manifest.sha256) {
        let _ = update.abort();
        bail!("empreinte SHA-256 différente du manifeste ({received} octets reçus)");
    }
    if total.is_some_and(|t| t != received) {
        let _ = update.abort();
        bail!("taille reçue {received} différente de l'annoncée");
    }
    update
        .complete()
        .context("validation de l'image (format ou version)")?;
    set(state, |s| s.progress = 100);
    info!("{received} octets écrits, SHA-256 conforme");
    Ok(())
}
