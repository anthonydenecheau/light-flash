//! Serveur HTTP de la lampe : page de pilotage (`/`), API JSON (`/api/light`) et
//! réception des identifiants Wi-Fi (`/connect`).
//!
//! Les handlers tournent dans la tâche httpd d'ESP-IDF : ils appliquent les commandes à
//! l'état partagé et ne touchent jamais au driver LED (voir la tâche lumière du firmware).

use anyhow::Result;
use embedded_svc::{
    http::{Headers, Method},
    io::{Read, Write},
};
use esp_idf_svc::http::server::{Configuration, EspHttpConnection, EspHttpServer, Request};
use light_core::{LightPatch, LightView, SharedState};
use log::{info, warn};
use serde::Deserialize;

static INDEX_HTML: &str = include_str!("static/index.html");

/// Taille maximale d'un corps JSON (modification de l'état ou identifiants Wi-Fi).
const MAX_BODY: usize = 512;
/// Pile de la tâche httpd : le parsing JSON et anyhow dépassent les 4 Ko par défaut.
const STACK_SIZE: usize = 10 * 1024;

/// Appelé avec le SSID et le mot de passe reçus par `POST /connect`.
/// Doit valider, enregistrer et rendre la main rapidement (le redémarrage éventuel est
/// à planifier dans un autre thread).
pub type OnWifiCredentials = dyn Fn(&str, &str) -> Result<()> + Send + Sync + 'static;

/// Points d'entrée réservés aux builds de debug (`cfg!(debug_assertions)` côté firmware).
pub struct DebugHooks {
    /// `POST /api/debug/wifi-disconnect` : force une déconnexion de la station pour tester la
    /// reconnexion automatique.
    pub wifi_disconnect: Box<dyn Fn() + Send + Sync + 'static>,
    /// `POST /api/debug/improv-authorize` : simule un appui court sur BOOT (autorisation Improv).
    pub improv_authorize: Box<dyn Fn() + Send + Sync + 'static>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WifiForm<'a> {
    wifi_ssid: &'a str,
    wifi_psk: &'a str,
}

pub fn start(
    light: SharedState,
    on_wifi_credentials: Box<OnWifiCredentials>,
    debug: Option<DebugHooks>,
) -> Result<EspHttpServer<'static>> {
    let mut server = EspHttpServer::new(&Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    })?;

    server.fn_handler::<anyhow::Error, _>("/", Method::Get, |req| {
        let mut resp = req.into_response(
            200,
            Some("OK"),
            &[("Content-Type", "text/html; charset=utf-8")],
        )?;
        resp.write_all(INDEX_HTML.as_bytes())?;
        Ok(())
    })?;

    let state = light.clone();
    server.fn_handler::<anyhow::Error, _>("/api/light", Method::Get, move |req| {
        let view = LightView::from(&*lock(&state));
        write_json(req, 200, &serde_json::to_vec(&view)?)
    })?;

    let state = light;
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
        info!("HTTP : {} commande(s) appliquée(s)", commands.len());
        write_json(req, 200, &serde_json::to_vec(&view)?)
    })?;

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
                let body = serde_json::json!({
                    "ok": true,
                    "message": "Identifiants enregistrés, la lampe redémarre et rejoint ce réseau.",
                });
                write_json(req, 200, &serde_json::to_vec(&body)?)
            }
            Err(e) => {
                warn!("identifiants Wi-Fi refusés : {e}");
                write_json(req, 400, &error_json(&e.to_string()))
            }
        }
    })?;

    if let Some(hooks) = debug {
        let DebugHooks {
            wifi_disconnect,
            improv_authorize,
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
        info!("points d'entrée de debug HTTP actifs");
    }

    info!("serveur HTTP démarré sur le port 80");
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

fn write_json(req: Request<&mut EspHttpConnection<'_>>, status: u16, body: &[u8]) -> Result<()> {
    let mut resp = req.into_response(status, None, &[("Content-Type", "application/json")])?;
    resp.write_all(body)?;
    Ok(())
}

fn error_json(message: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "error": message })).unwrap_or_default()
}
