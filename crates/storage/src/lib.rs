//! Persistance en NVS (flash) : identifiants Wi-Fi et dernier état de la lampe.

use anyhow::{bail, Context, Result};
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use light_core::LightState;
use std::sync::{Arc, Mutex, MutexGuard};

/// Espace de noms NVS (15 caractères maximum).
const NAMESPACE: &str = "light";
const KEY_SSID: &str = "wifi_ssid";
const KEY_PSK: &str = "wifi_psk";
const KEY_STATE: &str = "light_state";

/// Stockage partagé entre threads (HTTP, persistance).
pub type SharedStorage = Arc<Mutex<Storage>>;

/// Verrouille sans propager un empoisonnement : une panique ailleurs ne doit pas bloquer la NVS.
pub fn lock(storage: &Mutex<Storage>) -> MutexGuard<'_, Storage> {
    storage
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub const SSID_MAX: usize = 32;
pub const PSK_MAX: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiCredentials {
    pub ssid: String,
    pub psk: String,
}

impl WifiCredentials {
    /// Valide les longueurs imposées par le Wi-Fi (SSID 1..=32 octets, mot de passe 0..=64).
    pub fn new(ssid: &str, psk: &str) -> Result<Self> {
        let ssid = ssid.trim();
        if ssid.is_empty() || ssid.len() > SSID_MAX {
            bail!("SSID vide ou plus long que {SSID_MAX} octets");
        }
        if psk.len() > PSK_MAX {
            bail!("mot de passe Wi-Fi plus long que {PSK_MAX} octets");
        }
        Ok(Self {
            ssid: ssid.to_owned(),
            psk: psk.to_owned(),
        })
    }
}

pub struct Storage {
    nvs: EspNvs<NvsDefault>,
}

impl Storage {
    pub fn new(partition: EspDefaultNvsPartition) -> Result<Self> {
        let nvs = EspNvs::new(partition, NAMESPACE, true).context("ouverture de la NVS")?;
        Ok(Self { nvs })
    }

    /// `None` si aucun SSID n'est enregistré.
    pub fn wifi_credentials(&self) -> Result<Option<WifiCredentials>> {
        let mut buf = [0u8; PSK_MAX + 1];
        let ssid = match self.nvs.get_str(KEY_SSID, &mut buf)? {
            Some(s) if !s.is_empty() => s.to_owned(),
            _ => return Ok(None),
        };
        let psk = self
            .nvs
            .get_str(KEY_PSK, &mut buf)?
            .unwrap_or_default()
            .to_owned();
        Ok(Some(WifiCredentials { ssid, psk }))
    }

    pub fn set_wifi_credentials(&mut self, creds: &WifiCredentials) -> Result<()> {
        self.nvs.set_str(KEY_SSID, &creds.ssid)?;
        self.nvs.set_str(KEY_PSK, &creds.psk)?;
        log::info!("identifiants Wi-Fi enregistrés pour « {} »", creds.ssid);
        Ok(())
    }

    pub fn clear_wifi_credentials(&mut self) -> Result<()> {
        self.nvs.remove(KEY_SSID)?;
        self.nvs.remove(KEY_PSK)?;
        Ok(())
    }

    /// Dernier état enregistré. `None` si absent ou illisible (ancien format) : on repart alors
    /// de l'état par défaut.
    pub fn light_state(&self) -> Result<Option<LightState>> {
        let mut buf = [0u8; 32];
        let Some(raw) = self.nvs.get_raw(KEY_STATE, &mut buf)? else {
            return Ok(None);
        };
        let state = LightState::from_bytes(raw);
        if state.is_none() {
            log::warn!("état enregistré illisible ({} octets), ignoré", raw.len());
        }
        Ok(state)
    }

    pub fn set_light_state(&mut self, state: &LightState) -> Result<()> {
        self.nvs.set_raw(KEY_STATE, &state.to_bytes())?;
        Ok(())
    }

    /// Réinitialisation d'usine : identifiants Wi-Fi et état de la lampe effacés.
    pub fn factory_reset(&mut self) -> Result<()> {
        self.clear_wifi_credentials()?;
        self.nvs.remove(KEY_STATE)?;
        log::warn!("NVS effacée : réinitialisation d'usine");
        Ok(())
    }
}
