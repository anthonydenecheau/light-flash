//! Serveur HTTP de la lampe : page de pilotage (`/`), API JSON (`/api/light`, `/api/status`),
//! réception des identifiants Wi-Fi (`/connect`), nom et actions système (`/api/name`,
//! `/api/system/*`), mises à jour (`/api/update*`), scènes et programmation (`/api/automation`,
//! `/api/scenes*`, `/api/schedule`, `/api/timer`, `/api/time`), manifeste et icône pour l'écran
//! d'accueil.
//!
//! Les handlers tournent dans la tâche httpd d'ESP-IDF : ils appliquent les commandes à
//! l'état partagé et ne touchent jamais au driver LED (voir la tâche lumière du firmware).
//! La page est servie compressée (gzip préparé à la compilation) depuis la flash.

use anyhow::Result;
use embedded_svc::{
    http::{Headers, Method},
    io::{Read, Write},
};
use esp_idf_svc::http::server::{Configuration, EspHttpConnection, EspHttpServer, Request};
use light_core::{
    scenes::Scene,
    schedule::{Entry, Schedule},
    LightPatch, LightView, SharedState,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

static INDEX_HTML: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index.html"));
static INDEX_GZ: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index.html.gz"));
static ICON_PNG: &[u8] = include_bytes!("static/icon-192.png");

/// Taille maximale d'un corps JSON (le programme complet, 8 horaires, fait ≈ 650 octets).
const MAX_BODY: usize = 1024;
/// Pile de la tâche httpd : le parsing JSON et anyhow dépassent les 4 Ko par défaut.
const STACK_SIZE: usize = 10 * 1024;
const MAX_URI_HANDLERS: usize = 32;

/// Appelé avec le SSID et le mot de passe reçus par `POST /connect`.
/// Doit valider, enregistrer et rendre la main rapidement (le redémarrage éventuel est
/// à planifier dans un autre thread).
pub type OnWifiCredentials = dyn Fn(&str, &str) -> Result<()> + Send + Sync + 'static;

/// Instantané pour `GET /api/status` et la page.
#[derive(Debug, Clone, Serialize)]
pub struct StatusView {
    pub name: String,
    pub hostname: String,
    pub version: String,
    pub hardware: String,
    pub uptime_s: u64,
    pub heap_free: u32,
    /// `connecting`, `station` ou `access-point`.
    pub wifi_mode: String,
    pub ssid: String,
    pub ip: Option<String>,
    pub rssi: Option<i32>,
    pub ble_active: bool,
    pub led_count: usize,
    pub max_ma: u32,
}

pub type StatusProvider = dyn Fn() -> StatusView + Send + Sync + 'static;

/// Une autre lampe découverte sur le réseau (mode groupe).
#[derive(Debug, Clone, Serialize)]
pub struct PeerView {
    pub name: String,
    pub hostname: String,
    pub ip: String,
    pub port: u16,
}

pub type PeersProvider = dyn Fn() -> Vec<PeerView> + Send + Sync + 'static;

/// Action déclenchée par la page, sans résultat.
pub type Action = dyn Fn() + Send + Sync + 'static;
/// Action déclenchée par la page, pouvant échouer (message renvoyé au client).
pub type FallibleAction = dyn Fn() -> Result<()> + Send + Sync + 'static;
/// Enregistrement d'une chaîne validée par le firmware (nom, adresse du serveur).
pub type StringSetter = dyn Fn(&str) -> Result<()> + Send + Sync + 'static;

/// Actions système fournies par le firmware ; chacune doit rendre la main vite.
pub struct SystemHooks {
    pub restart: Box<Action>,
    pub forget_wifi: Box<FallibleAction>,
    /// Rendre la lampe visible en Bluetooth et autoriser la configuration (= appui sur BOOT).
    pub ble_visible: Box<Action>,
    /// Enregistre un nouveau nom (validé par le firmware) ; prend effet au redémarrage.
    pub set_name: Box<StringSetter>,
}

/// Mise à jour disponible sur le serveur local.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateAvailable {
    pub version: String,
    pub notes: String,
    pub size: u64,
}

/// Instantané pour `GET /api/update`.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateView {
    pub current: String,
    pub url: String,
    pub available: Option<UpdateAvailable>,
    /// `idle`, `checking`, `downloading`, `verifying`, `rebooting`, `failed`.
    pub phase: String,
    pub progress: u8,
    pub error: Option<String>,
    pub last_check_s: Option<u64>,
    pub running_slot: String,
}

pub struct UpdateHooks {
    pub status: Box<dyn Fn() -> UpdateView + Send + Sync + 'static>,
    pub check: Box<Action>,
    pub install: Box<FallibleAction>,
    pub set_url: Box<StringSetter>,
}

/// Points d'entrée de test, activés par la feature `debug-hooks` du firmware.
pub struct DebugHooks {
    /// `POST /api/debug/wifi-disconnect` : force une déconnexion de la station pour tester la
    /// reconnexion automatique.
    pub wifi_disconnect: Box<Action>,
    /// `POST /api/debug/improv-authorize` : simule un appui court sur BOOT (autorisation Improv).
    pub improv_authorize: Box<Action>,
    /// `POST /api/debug/ble-off` : coupe le BLE tout de suite (test du cycle arrêt / rallumage).
    pub ble_off: Box<Action>,
}

/// Scènes, minuterie et horaires, tenus par le firmware (`automation.rs`).
pub trait Automation: Send + Sync {
    fn view(&self) -> AutomationView;
    /// Ajoute ou remplace une scène ; rend l'identifiant attribué.
    fn save_scene(&self, scene: Scene) -> Result<u8>;
    fn delete_scene(&self, id: u8) -> Result<()>;
    fn apply_scene(&self, id: u8) -> Result<()>;
    fn set_schedule(&self, schedule: Schedule) -> Result<()>;
    /// Minuterie d'extinction ; 0 l'annule.
    fn set_timer(&self, minutes: u32) -> Result<()>;
    /// Fuseau horaire au format POSIX (`CET-1CEST,M3.5.0,M10.5.0/3`).
    fn set_timezone(&self, tz: &str) -> Result<()>;
}

/// `GET /api/automation`.
#[derive(Debug, Clone, Serialize)]
pub struct AutomationView {
    pub scenes: Vec<Scene>,
    pub schedule: Vec<Entry>,
    /// Secondes avant l'extinction par la minuterie, `None` si inactive.
    pub timer_s: Option<u64>,
    pub time: TimeView,
}

/// Heure locale de la lampe ; les champs sont absents tant que l'horloge n'est pas à l'heure.
#[derive(Debug, Clone, Serialize)]
pub struct TimeView {
    pub synced: bool,
    /// 0 = lundi.
    pub weekday: Option<u8>,
    pub hour: Option<u8>,
    pub minute: Option<u8>,
    pub tz: String,
}

pub struct HttpContext {
    pub light: SharedState,
    pub on_wifi_credentials: Box<OnWifiCredentials>,
    pub status: Box<StatusProvider>,
    pub peers: Box<PeersProvider>,
    pub system: SystemHooks,
    pub update: UpdateHooks,
    pub automation: Arc<dyn Automation>,
    pub debug: Option<DebugHooks>,
}

/// `POST /api/scenes` : les champs absents sont pris sur l'état courant de la lampe
/// (« mémoriser les réglages actuels »). `id` absent ou 0 : nouvelle scène, ou remplacement de
/// celle qui porte le même nom.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneForm {
    #[serde(default)]
    id: u8,
    name: String,
    color: Option<String>,
    brightness: Option<u8>,
    effect: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdForm {
    id: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimerForm {
    minutes: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TimezoneForm {
    tz: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UrlForm<'a> {
    url: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WifiForm<'a> {
    wifi_ssid: &'a str,
    wifi_psk: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NameForm<'a> {
    name: &'a str,
}

pub fn start(ctx: HttpContext) -> Result<EspHttpServer<'static>> {
    let HttpContext {
        light,
        on_wifi_credentials,
        status,
        peers,
        system,
        update,
        automation,
        debug,
    } = ctx;
    let status = Arc::new(status);
    let mut server = EspHttpServer::new(&Configuration {
        stack_size: STACK_SIZE,
        max_uri_handlers: MAX_URI_HANDLERS,
        lru_purge_enable: true,
        ..Default::default()
    })?;

    // ---- page, icône, manifeste
    server.fn_handler::<anyhow::Error, _>("/", Method::Get, |req| {
        let gzip = req
            .header("Accept-Encoding")
            .is_some_and(|v| v.contains("gzip"));
        let mut headers = vec![
            ("Content-Type", "text/html; charset=utf-8"),
            ("Cache-Control", "no-cache"),
        ];
        if gzip {
            headers.push(("Content-Encoding", "gzip"));
        }
        let mut resp = req.into_response(200, Some("OK"), &headers)?;
        resp.write_all(if gzip { INDEX_GZ } else { INDEX_HTML })?;
        Ok(())
    })?;

    server.fn_handler::<anyhow::Error, _>("/icon-192.png", Method::Get, |req| {
        let mut resp = req.into_response(
            200,
            Some("OK"),
            &[
                ("Content-Type", "image/png"),
                ("Cache-Control", "public, max-age=604800"),
            ],
        )?;
        resp.write_all(ICON_PNG)?;
        Ok(())
    })?;

    let st = status.clone();
    server.fn_handler::<anyhow::Error, _>("/manifest.json", Method::Get, move |req| {
        let s = st();
        let manifest = serde_json::json!({
            "name": s.name,
            "short_name": s.name,
            "start_url": "/",
            "display": "standalone",
            "background_color": "#15171c",
            "theme_color": "#15171c",
            "icons": [{ "src": "/icon-192.png", "sizes": "192x192", "type": "image/png" }],
        });
        let mut resp = req.into_response(
            200,
            Some("OK"),
            &[("Content-Type", "application/manifest+json")],
        )?;
        resp.write_all(&serde_json::to_vec(&manifest)?)?;
        Ok(())
    })?;

    // ---- état
    let st = status.clone();
    server.fn_handler::<anyhow::Error, _>("/api/status", Method::Get, move |req| {
        write_json(req, 200, &serde_json::to_vec(&st())?)
    })?;

    // Autres lampes découvertes (mode groupe) ; la page les pilote directement par leur adresse,
    // d'où l'en-tête CORS sur toutes les réponses JSON.
    server.fn_handler::<anyhow::Error, _>("/api/peers", Method::Get, move |req| {
        let body = serde_json::json!({ "peers": peers() });
        write_json(req, 200, &serde_json::to_vec(&body)?)
    })?;

    let state = light.clone();
    server.fn_handler::<anyhow::Error, _>("/api/light", Method::Get, move |req| {
        let view = LightView::from(&*lock(&state));
        write_json(req, 200, &serde_json::to_vec(&view)?)
    })?;

    let state = light.clone();
    server.fn_handler::<anyhow::Error, _>("/api/light", Method::Post, move |mut req| {
        let body = match read_body(&mut req)? {
            Ok(body) => body,
            Err(status) => {
                return write_json(req, status, &error_json("corps absent ou trop long"))
            }
        };
        let patch: LightPatch = match serde_json::from_slice(&body) {
            Ok(patch) => patch,
            Err(e) => return write_json(req, 400, &error_json(&format!("JSON invalide : {e}"))),
        };
        let commands = match patch.into_commands() {
            Ok(commands) => commands,
            Err(e) => return write_json(req, 400, &error_json(&e.to_string())),
        };
        let view = {
            let mut s = lock(&state);
            for cmd in &commands {
                s.apply(*cmd);
            }
            LightView::from(&*s)
        };
        write_json(req, 200, &serde_json::to_vec(&view)?)
    })?;

    // ---- Wi-Fi
    server.fn_handler::<anyhow::Error, _>("/connect", Method::Post, move |mut req| {
        let body = match read_body(&mut req)? {
            Ok(body) => body,
            Err(status) => {
                return write_json(req, status, &error_json("corps absent ou trop long"))
            }
        };
        let form: WifiForm = match serde_json::from_slice(&body) {
            Ok(form) => form,
            Err(e) => return write_json(req, 400, &error_json(&format!("JSON invalide : {e}"))),
        };
        // Le mot de passe n'est ni journalisé ni renvoyé au client.
        match on_wifi_credentials(form.wifi_ssid, form.wifi_psk) {
            Ok(()) => {
                info!("identifiants Wi-Fi reçus pour « {} »", form.wifi_ssid);
                write_json(req, 200, &ok_json("Réseau enregistré, la lampe redémarre."))
            }
            Err(e) => {
                warn!("identifiants Wi-Fi refusés : {e}");
                write_json(req, 400, &error_json(&e.to_string()))
            }
        }
    })?;

    // ---- nom et actions système
    let SystemHooks {
        restart,
        forget_wifi,
        ble_visible,
        set_name,
    } = system;
    server.fn_handler::<anyhow::Error, _>("/api/name", Method::Post, move |mut req| {
        let body = match read_body(&mut req)? {
            Ok(body) => body,
            Err(status) => {
                return write_json(req, status, &error_json("corps absent ou trop long"))
            }
        };
        let form: NameForm = match serde_json::from_slice(&body) {
            Ok(form) => form,
            Err(e) => return write_json(req, 400, &error_json(&format!("JSON invalide : {e}"))),
        };
        match set_name(form.name) {
            Ok(()) => write_json(req, 200, &ok_json("Nom enregistré, la lampe redémarre.")),
            Err(e) => write_json(req, 400, &error_json(&e.to_string())),
        }
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/system/restart", Method::Post, move |req| {
        warn!("redémarrage demandé depuis la page");
        restart();
        write_json(req, 200, &ok_json("La lampe redémarre."))
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/system/forget-wifi", Method::Post, move |req| {
        warn!("oubli du Wi-Fi demandé depuis la page");
        match forget_wifi() {
            Ok(()) => write_json(
                req,
                200,
                &ok_json("Wi-Fi oublié, la lampe redémarre sur son point d'accès."),
            ),
            Err(e) => write_json(req, 500, &error_json(&e.to_string())),
        }
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/system/ble", Method::Post, move |req| {
        info!("visibilité Bluetooth demandée depuis la page");
        ble_visible();
        write_json(
            req,
            200,
            &ok_json("Visible en Bluetooth pendant 5 minutes."),
        )
    })?;

    // ---- mise à jour du firmware
    let UpdateHooks {
        status: update_status,
        check,
        install,
        set_url,
    } = update;
    server.fn_handler::<anyhow::Error, _>("/api/update", Method::Get, move |req| {
        write_json(req, 200, &serde_json::to_vec(&update_status())?)
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/update/check", Method::Post, move |req| {
        check();
        write_json(req, 200, &ok_json("Vérification lancée."))
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/update/install", Method::Post, move |req| {
        match install() {
            Ok(()) => {
                warn!("mise à jour lancée depuis la page");
                write_json(req, 200, &ok_json("Mise à jour lancée."))
            }
            Err(e) => write_json(req, 409, &error_json(&e.to_string())),
        }
    })?;
    server.fn_handler::<anyhow::Error, _>("/api/update/url", Method::Post, move |mut req| {
        let body = match read_body(&mut req)? {
            Ok(body) => body,
            Err(status) => {
                return write_json(req, status, &error_json("corps absent ou trop long"))
            }
        };
        let form: UrlForm = match serde_json::from_slice(&body) {
            Ok(form) => form,
            Err(e) => return write_json(req, 400, &error_json(&format!("JSON invalide : {e}"))),
        };
        match set_url(form.url) {
            Ok(()) => write_json(
                req,
                200,
                &ok_json("Serveur enregistré, vérification lancée."),
            ),
            Err(e) => write_json(req, 400, &error_json(&e.to_string())),
        }
    })?;

    // ---- scènes et programmation
    let auto = automation.clone();
    server.fn_handler::<anyhow::Error, _>("/api/automation", Method::Get, move |req| {
        write_json(req, 200, &serde_json::to_vec(&auto.view())?)
    })?;
    let auto = automation.clone();
    let state = light;
    server.fn_handler::<anyhow::Error, _>("/api/scenes", Method::Post, move |mut req| {
        let form: SceneForm = match parse_body(&mut req)? {
            Ok(form) => form,
            Err((status, msg)) => return write_json(req, status, &error_json(&msg)),
        };
        let mut scene = Scene::from_state(form.id, &form.name, &lock(&state));
        if let Some(c) = form.color {
            scene.color = c;
        }
        if let Some(b) = form.brightness {
            scene.brightness = b;
        }
        if let Some(e) = form.effect {
            scene.effect = e;
        }
        match auto.save_scene(scene) {
            Ok(id) => write_json(
                req,
                200,
                &serde_json::to_vec(&serde_json::json!({
                    "ok": true, "id": id, "message": "Scène enregistrée."
                }))?,
            ),
            Err(e) => write_json(req, 400, &error_json(&e.to_string())),
        }
    })?;
    let auto = automation.clone();
    server.fn_handler::<anyhow::Error, _>("/api/scenes/delete", Method::Post, move |mut req| {
        let form: IdForm = match parse_body(&mut req)? {
            Ok(form) => form,
            Err((status, msg)) => return write_json(req, status, &error_json(&msg)),
        };
        match auto.delete_scene(form.id) {
            Ok(()) => write_json(req, 200, &ok_json("Scène supprimée.")),
            Err(e) => write_json(req, 404, &error_json(&e.to_string())),
        }
    })?;
    let auto = automation.clone();
    server.fn_handler::<anyhow::Error, _>("/api/scenes/apply", Method::Post, move |mut req| {
        let form: IdForm = match parse_body(&mut req)? {
            Ok(form) => form,
            Err((status, msg)) => return write_json(req, status, &error_json(&msg)),
        };
        match auto.apply_scene(form.id) {
            Ok(()) => write_json(req, 200, &ok_json("Scène appliquée.")),
            Err(e) => write_json(req, 404, &error_json(&e.to_string())),
        }
    })?;
    let auto = automation.clone();
    server.fn_handler::<anyhow::Error, _>("/api/schedule", Method::Post, move |mut req| {
        let schedule: Schedule = match parse_body(&mut req)? {
            Ok(s) => s,
            Err((status, msg)) => return write_json(req, status, &error_json(&msg)),
        };
        match auto.set_schedule(schedule) {
            Ok(()) => write_json(req, 200, &ok_json("Programme enregistré.")),
            Err(e) => write_json(req, 400, &error_json(&e.to_string())),
        }
    })?;
    let auto = automation.clone();
    server.fn_handler::<anyhow::Error, _>("/api/timer", Method::Post, move |mut req| {
        let form: TimerForm = match parse_body(&mut req)? {
            Ok(form) => form,
            Err((status, msg)) => return write_json(req, status, &error_json(&msg)),
        };
        match auto.set_timer(form.minutes) {
            Ok(()) if form.minutes == 0 => write_json(req, 200, &ok_json("Minuterie annulée.")),
            Ok(()) => write_json(
                req,
                200,
                &ok_json(&format!("Extinction dans {} min.", form.minutes)),
            ),
            Err(e) => write_json(req, 400, &error_json(&e.to_string())),
        }
    })?;
    let auto = automation;
    server.fn_handler::<anyhow::Error, _>("/api/time", Method::Post, move |mut req| {
        let form: TimezoneForm = match parse_body(&mut req)? {
            Ok(form) => form,
            Err((status, msg)) => return write_json(req, status, &error_json(&msg)),
        };
        match auto.set_timezone(&form.tz) {
            Ok(()) => write_json(req, 200, &ok_json("Fuseau horaire enregistré.")),
            Err(e) => write_json(req, 400, &error_json(&e.to_string())),
        }
    })?;

    if let Some(hooks) = debug {
        let DebugHooks {
            wifi_disconnect,
            improv_authorize,
            ble_off,
        } = hooks;
        server.fn_handler::<anyhow::Error, _>(
            "/api/debug/wifi-disconnect",
            Method::Post,
            move |req| {
                warn!("debug : déconnexion Wi-Fi forcée");
                wifi_disconnect();
                write_json(req, 200, br#"{"ok":true}"#)
            },
        )?;
        server.fn_handler::<anyhow::Error, _>(
            "/api/debug/improv-authorize",
            Method::Post,
            move |req| {
                warn!("debug : autorisation Improv simulée (appui sur BOOT)");
                improv_authorize();
                write_json(req, 200, br#"{"ok":true}"#)
            },
        )?;
        server.fn_handler::<anyhow::Error, _>("/api/debug/ble-off", Method::Post, move |req| {
            warn!("debug : arrêt du BLE demandé");
            ble_off();
            write_json(req, 200, br#"{"ok":true}"#)
        })?;
        info!("points d'entrée de debug HTTP actifs");
    }

    info!(
        "serveur HTTP démarré sur le port 80 (page {} octets, {} gzip)",
        INDEX_HTML.len(),
        INDEX_GZ.len()
    );
    Ok(server)
}

fn lock(state: &SharedState) -> std::sync::MutexGuard<'_, light_core::LightState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Lit le corps complet. `Err(code HTTP)` si absent ou trop long.
fn read_body(req: &mut Request<&mut EspHttpConnection<'_>>) -> Result<Result<Vec<u8>, u16>> {
    let len = req.content_len().unwrap_or(0) as usize;
    if len == 0 {
        return Ok(Err(400));
    }
    if len > MAX_BODY {
        return Ok(Err(413));
    }
    let mut buf = vec![0; len];
    req.read_exact(&mut buf)?;
    Ok(Ok(buf))
}

/// Lit et désérialise le corps JSON. `Err((code HTTP, message))` si absent, trop long ou invalide.
fn parse_body<T: serde::de::DeserializeOwned>(
    req: &mut Request<&mut EspHttpConnection<'_>>,
) -> Result<Result<T, (u16, String)>> {
    let body = match read_body(req)? {
        Ok(body) => body,
        Err(status) => return Ok(Err((status, "corps absent ou trop long".to_owned()))),
    };
    Ok(serde_json::from_slice(&body).map_err(|e| (400, format!("JSON invalide : {e}"))))
}

fn write_json(req: Request<&mut EspHttpConnection<'_>>, status: u16, body: &[u8]) -> Result<()> {
    let mut resp = req.into_response(
        status,
        None,
        &[
            ("Content-Type", "application/json"),
            ("Cache-Control", "no-store"),
            // Mode groupe : la page d'une lampe pilote les autres par leur adresse.
            ("Access-Control-Allow-Origin", "*"),
        ],
    )?;
    resp.write_all(body)?;
    Ok(())
}

fn error_json(message: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "error": message })).unwrap_or_default()
}

fn ok_json(message: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "ok": true, "message": message })).unwrap_or_default()
}
