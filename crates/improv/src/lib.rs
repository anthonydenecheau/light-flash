//! Protocole Improv Wi-Fi sur BLE (<https://www.improv-wifi.com/ble/>), partie indépendante du
//! matériel : constantes du service GATT, encodage et décodage des paquets RPC, machine à états
//! (autorisation par bouton, provisioning, résultat). Le firmware fournit la pile BLE, le bouton,
//! la connexion Wi-Fi et le stockage.

pub mod packet;
pub mod state;

pub use packet::{checksum, encode_result, parse_command, Command, DeviceInfo};
pub use state::{Error, Machine, Outcome, State};

/// UUID 128 bits du service et des caractéristiques (chaînes pour `uuid128!`).
pub const SERVICE_UUID: &str = "00467768-6228-2272-4663-277478268000";
pub const CHR_CURRENT_STATE: &str = "00467768-6228-2272-4663-277478268001";
pub const CHR_ERROR_STATE: &str = "00467768-6228-2272-4663-277478268002";
pub const CHR_RPC_COMMAND: &str = "00467768-6228-2272-4663-277478268003";
pub const CHR_RPC_RESULT: &str = "00467768-6228-2272-4663-277478268004";
pub const CHR_CAPABILITIES: &str = "00467768-6228-2272-4663-277478268005";
/// UUID 16 bits du *service data* de l'advertising.
pub const SERVICE_DATA_UUID16: u16 = 0x4677;

/// Bits de la caractéristique Capabilities.
pub mod capability {
    pub const IDENTIFY: u8 = 1 << 0;
    pub const DEVICE_INFO: u8 = 1 << 1;
    pub const SCAN_WIFI: u8 = 1 << 2;
    pub const HOSTNAME: u8 = 1 << 3;
    pub const DEVICE_NAME: u8 = 1 << 4;
}

/// Contenu du *service data* de l'advertising : état courant, capacités, 4 octets réservés.
pub fn advertisement_service_data(state: State, capabilities: u8) -> [u8; 6] {
    [state as u8, capabilities, 0, 0, 0, 0]
}
