//! Machine à états Improv : autorisation par action physique (bouton), provisioning, résultat.

use crate::packet::{self, Command, DeviceInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    AuthorizationRequired = 0x01,
    Authorized = 0x02,
    Provisioning = 0x03,
    Provisioned = 0x04,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Error {
    None = 0x00,
    InvalidRpcPacket = 0x01,
    UnknownCommand = 0x02,
    UnableToConnect = 0x03,
    NotAuthorized = 0x04,
    BadHostname = 0x05,
    Unknown = 0xFF,
}

/// Ce que le firmware doit faire après un paquet reçu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Erreur à publier sur Error State ; l'état ne change pas.
    Reject(Error),
    /// Tenter la connexion Wi-Fi, puis appeler `provisioning_succeeded` ou `provisioning_failed`.
    StartProvisioning { ssid: String, password: String },
    /// Faire clignoter la lampe.
    Identify,
    /// Résultat RPC à publier (Device info).
    Reply(Vec<u8>),
    /// Scanner les réseaux et publier un résultat par réseau, puis un résultat vide.
    ScanWifi,
}

#[derive(Debug, Clone)]
pub struct Machine {
    require_authorization: bool,
    authorization_timeout_ms: u64,
    state: State,
    error: Error,
    authorized_until_ms: Option<u64>,
}

impl Machine {
    /// Délai conseillé par la spécification avant retrait de l'autorisation.
    pub const DEFAULT_AUTHORIZATION_TIMEOUT_MS: u64 = 60_000;

    pub fn new(require_authorization: bool, authorization_timeout_ms: u64) -> Self {
        Self {
            require_authorization,
            authorization_timeout_ms,
            state: if require_authorization {
                State::AuthorizationRequired
            } else {
                State::Authorized
            },
            error: Error::None,
            authorized_until_ms: None,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn error(&self) -> Error {
        self.error
    }

    /// Action physique de l'utilisateur (appui sur le bouton). Ouvre ou prolonge la fenêtre
    /// d'autorisation ; ignorée pendant une connexion. Renvoie `true` si l'état a changé.
    pub fn authorize(&mut self, now_ms: u64) -> bool {
        if self.state == State::Provisioning {
            return false;
        }
        self.authorized_until_ms = Some(now_ms + self.authorization_timeout_ms);
        let changed = self.state != State::Authorized;
        self.state = State::Authorized;
        self.error = Error::None;
        changed
    }

    /// À appeler régulièrement : retire l'autorisation expirée. Renvoie `true` si l'état a changé.
    pub fn tick(&mut self, now_ms: u64) -> bool {
        if self.state == State::Authorized && self.require_authorization {
            if let Some(until) = self.authorized_until_ms {
                if now_ms >= until {
                    self.state = State::AuthorizationRequired;
                    self.authorized_until_ms = None;
                    return true;
                }
            }
        }
        false
    }

    /// Paquet écrit sur RPC Command.
    pub fn handle_packet(&mut self, packet: &[u8], device_info: &DeviceInfo) -> Outcome {
        let command = match packet::parse_command(packet) {
            Ok(c) => c,
            Err(e) => {
                self.error = e;
                return Outcome::Reject(e);
            }
        };
        match command {
            Command::Identify => Outcome::Identify,
            Command::DeviceInfo => Outcome::Reply(device_info.to_result()),
            Command::ScanWifi => Outcome::ScanWifi,
            Command::WifiSettings { ssid, password } => {
                if self.state != State::Authorized {
                    self.error = Error::NotAuthorized;
                    return Outcome::Reject(Error::NotAuthorized);
                }
                self.state = State::Provisioning;
                self.error = Error::None;
                Outcome::StartProvisioning { ssid, password }
            }
        }
    }

    /// Connexion réussie : état Provisioned ; renvoie le résultat RPC à publier (URL à ouvrir).
    pub fn provisioning_succeeded(&mut self, urls: &[&str]) -> Vec<u8> {
        self.state = State::Provisioned;
        self.error = Error::None;
        self.authorized_until_ms = None;
        packet::encode_result(packet::CMD_WIFI_SETTINGS, urls)
    }

    /// Connexion échouée : erreur Unable to connect, retour à Authorized avec une nouvelle
    /// fenêtre d'autorisation pour que l'utilisateur puisse corriger sans retoucher au bouton.
    pub fn provisioning_failed(&mut self, now_ms: u64) {
        self.state = State::Authorized;
        self.error = Error::UnableToConnect;
        self.authorized_until_ms = Some(now_ms + self.authorization_timeout_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{checksum, CMD_IDENTIFY, CMD_WIFI_SETTINGS};

    const TIMEOUT: u64 = 60_000;

    fn info() -> DeviceInfo {
        DeviceInfo {
            firmware: "light-flash".into(),
            version: "0.1.0".into(),
            hardware: "esp32c3".into(),
            name: "lampe".into(),
        }
    }

    fn wifi_packet(ssid: &str, password: &str) -> Vec<u8> {
        let mut d = vec![ssid.len() as u8];
        d.extend_from_slice(ssid.as_bytes());
        d.push(password.len() as u8);
        d.extend_from_slice(password.as_bytes());
        let mut p = vec![CMD_WIFI_SETTINGS, d.len() as u8];
        p.extend_from_slice(&d);
        p.push(checksum(&p));
        p
    }

    #[test]
    fn starts_locked_when_authorization_is_required() {
        let m = Machine::new(true, TIMEOUT);
        assert_eq!(m.state(), State::AuthorizationRequired);
        let m = Machine::new(false, TIMEOUT);
        assert_eq!(m.state(), State::Authorized);
    }

    #[test]
    fn wifi_settings_without_authorization_are_rejected() {
        let mut m = Machine::new(true, TIMEOUT);
        let out = m.handle_packet(&wifi_packet("Box", "pw"), &info());
        assert_eq!(out, Outcome::Reject(Error::NotAuthorized));
        assert_eq!(m.state(), State::AuthorizationRequired);
        assert_eq!(m.error(), Error::NotAuthorized);
    }

    #[test]
    fn button_authorizes_then_times_out() {
        let mut m = Machine::new(true, TIMEOUT);
        assert!(m.authorize(1_000));
        assert_eq!(m.state(), State::Authorized);
        assert_eq!(m.error(), Error::None);
        assert!(!m.tick(1_000 + TIMEOUT - 1));
        assert!(m.tick(1_000 + TIMEOUT));
        assert_eq!(m.state(), State::AuthorizationRequired);
        // Un nouvel appui prolonge une autorisation en cours.
        m.authorize(100_000);
        assert!(!m.authorize(130_000));
        assert!(!m.tick(100_000 + TIMEOUT));
        assert!(m.tick(130_000 + TIMEOUT));
    }

    #[test]
    fn full_provisioning_flow() {
        let mut m = Machine::new(true, TIMEOUT);
        m.authorize(0);
        let out = m.handle_packet(&wifi_packet("Box", "pw"), &info());
        assert_eq!(
            out,
            Outcome::StartProvisioning {
                ssid: "Box".into(),
                password: "pw".into()
            }
        );
        assert_eq!(m.state(), State::Provisioning);
        assert!(!m.authorize(10), "ignoré pendant la connexion");
        assert!(
            !m.tick(10 * TIMEOUT),
            "pas d'expiration pendant la connexion"
        );
        let result = m.provisioning_succeeded(&["http://light-flash.local/"]);
        assert_eq!(m.state(), State::Provisioned);
        assert_eq!(result[0], CMD_WIFI_SETTINGS);
        assert_eq!(&result[3..3 + 25], b"http://light-flash.local/");
        // Après succès, une nouvelle configuration demande un nouvel appui.
        assert_eq!(
            m.handle_packet(&wifi_packet("Autre", ""), &info()),
            Outcome::Reject(Error::NotAuthorized)
        );
        assert!(m.authorize(1));
        assert!(matches!(
            m.handle_packet(&wifi_packet("Autre", ""), &info()),
            Outcome::StartProvisioning { .. }
        ));
    }

    #[test]
    fn failure_reports_unable_to_connect_and_keeps_authorization() {
        let mut m = Machine::new(true, TIMEOUT);
        m.authorize(0);
        m.handle_packet(&wifi_packet("Box", "faux"), &info());
        m.provisioning_failed(20_000);
        assert_eq!(m.state(), State::Authorized);
        assert_eq!(m.error(), Error::UnableToConnect);
        assert!(matches!(
            m.handle_packet(&wifi_packet("Box", "bon"), &info()),
            Outcome::StartProvisioning { .. }
        ));
        m.provisioning_failed(40_000);
        assert!(m.tick(40_000 + TIMEOUT));
    }

    #[test]
    fn identify_and_device_info_never_need_authorization() {
        let mut m = Machine::new(true, TIMEOUT);
        assert_eq!(
            m.handle_packet(&[CMD_IDENTIFY, 0, CMD_IDENTIFY], &info()),
            Outcome::Identify
        );
        match m.handle_packet(&[0x03, 0, 0x03], &info()) {
            Outcome::Reply(bytes) => assert_eq!(bytes, info().to_result()),
            other => panic!("{other:?}"),
        }
        assert_eq!(m.state(), State::AuthorizationRequired);
    }

    #[test]
    fn scan_never_needs_authorization() {
        let mut m = Machine::new(true, TIMEOUT);
        assert_eq!(
            m.handle_packet(&[0x04, 0, 0x04], &info()),
            Outcome::ScanWifi
        );
        assert_eq!(m.state(), State::AuthorizationRequired);
    }

    #[test]
    fn invalid_packets_set_the_error_without_changing_state() {
        let mut m = Machine::new(true, TIMEOUT);
        m.authorize(0);
        assert_eq!(
            m.handle_packet(&[1, 2, 3], &info()),
            Outcome::Reject(Error::InvalidRpcPacket)
        );
        assert_eq!(m.error(), Error::InvalidRpcPacket);
        assert_eq!(m.state(), State::Authorized);
        assert_eq!(
            m.handle_packet(&[0x42, 0, 0x42], &info()),
            Outcome::Reject(Error::UnknownCommand)
        );
    }

    #[test]
    fn service_data_carries_state_and_capabilities() {
        use crate::{advertisement_service_data, capability};
        let caps = capability::IDENTIFY | capability::DEVICE_INFO;
        assert_eq!(
            advertisement_service_data(State::AuthorizationRequired, caps),
            [0x01, 0x03, 0, 0, 0, 0]
        );
    }
}
