//! Persistance en NVS (flash) : identifiants Wi-Fi, et plus tard dernier état de la lampe.

use anyhow::{bail, Context, Result};
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};

/// Espace de noms NVS (15 caractères maximum).
const NAMESPACE: &str = "light";
const KEY_SSID: &str = "wifi_ssid";
const KEY_PSK: &str = "wifi_psk";

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
}
