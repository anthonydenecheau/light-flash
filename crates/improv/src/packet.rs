//! Paquets RPC : `[commande][longueur][données…][checksum]`, checksum = somme des octets
//! précédents, octet de poids faible.

use crate::state::Error;

pub const CMD_WIFI_SETTINGS: u8 = 0x01;
pub const CMD_IDENTIFY: u8 = 0x02;
pub const CMD_DEVICE_INFO: u8 = 0x03;
pub const CMD_SCAN_WIFI: u8 = 0x04;

/// Longueurs maximales Wi-Fi (octets).
pub const SSID_MAX: usize = 32;
pub const PASSWORD_MAX: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    WifiSettings { ssid: String, password: String },
    Identify,
    DeviceInfo,
}

/// Réponse à la commande Device info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub firmware: String,
    pub version: String,
    pub hardware: String,
    pub name: String,
}

impl DeviceInfo {
    pub fn to_result(&self) -> Vec<u8> {
        encode_result(
            CMD_DEVICE_INFO,
            &[&self.firmware, &self.version, &self.hardware, &self.name],
        )
    }
}

pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, b| acc.wrapping_add(*b))
}

/// Décode un paquet écrit sur la caractéristique RPC Command.
pub fn parse_command(packet: &[u8]) -> Result<Command, Error> {
    let [command, len, rest @ ..] = packet else {
        return Err(Error::InvalidRpcPacket);
    };
    let len = *len as usize;
    if rest.len() != len + 1 {
        return Err(Error::InvalidRpcPacket);
    }
    let (data, check) = rest.split_at(len);
    if check[0] != checksum(&packet[..packet.len() - 1]) {
        return Err(Error::InvalidRpcPacket);
    }
    match *command {
        CMD_WIFI_SETTINGS => parse_wifi_settings(data),
        CMD_IDENTIFY => Ok(Command::Identify),
        CMD_DEVICE_INFO => Ok(Command::DeviceInfo),
        _ => Err(Error::UnknownCommand),
    }
}

fn parse_wifi_settings(data: &[u8]) -> Result<Command, Error> {
    let (ssid, rest) = take_string(data)?;
    let (password, rest) = take_string(rest)?;
    if !rest.is_empty() || ssid.is_empty() || ssid.len() > SSID_MAX || password.len() > PASSWORD_MAX
    {
        return Err(Error::InvalidRpcPacket);
    }
    Ok(Command::WifiSettings { ssid, password })
}

/// Chaîne préfixée par sa longueur ; UTF-8 obligatoire.
fn take_string(data: &[u8]) -> Result<(String, &[u8]), Error> {
    let [len, rest @ ..] = data else {
        return Err(Error::InvalidRpcPacket);
    };
    let len = *len as usize;
    if rest.len() < len {
        return Err(Error::InvalidRpcPacket);
    }
    let (s, rest) = rest.split_at(len);
    let s = core::str::from_utf8(s).map_err(|_| Error::InvalidRpcPacket)?;
    Ok((s.to_owned(), rest))
}

/// Encode un résultat RPC : `[commande][longueur][chaînes préfixées…][checksum]`.
/// Les chaînes qui ne tiennent pas dans les 255 octets de données sont omises.
pub fn encode_result(command: u8, strings: &[&str]) -> Vec<u8> {
    let mut data = Vec::new();
    for s in strings {
        let bytes = s.as_bytes();
        if bytes.len() > 255 || data.len() + 1 + bytes.len() > 255 {
            break;
        }
        data.push(bytes.len() as u8);
        data.extend_from_slice(bytes);
    }
    let mut packet = Vec::with_capacity(data.len() + 3);
    packet.push(command);
    packet.push(data.len() as u8);
    packet.extend_from_slice(&data);
    packet.push(checksum(&packet));
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un paquet valide à partir de la commande et des données.
    fn packet(command: u8, data: &[u8]) -> Vec<u8> {
        let mut p = vec![command, data.len() as u8];
        p.extend_from_slice(data);
        p.push(checksum(&p));
        p
    }

    fn wifi_data(ssid: &str, password: &str) -> Vec<u8> {
        let mut d = vec![ssid.len() as u8];
        d.extend_from_slice(ssid.as_bytes());
        d.push(password.len() as u8);
        d.extend_from_slice(password.as_bytes());
        d
    }

    #[test]
    fn checksum_keeps_the_low_byte() {
        assert_eq!(checksum(&[0x01, 0x00]), 0x01);
        assert_eq!(checksum(&[0xff, 0x02]), 0x01);
        assert_eq!(checksum(&[]), 0);
    }

    #[test]
    fn parses_wifi_settings() {
        let p = packet(CMD_WIFI_SETTINGS, &wifi_data("MonReseau", "secret123"));
        assert_eq!(
            parse_command(&p),
            Ok(Command::WifiSettings {
                ssid: "MonReseau".into(),
                password: "secret123".into()
            })
        );
        let open = packet(CMD_WIFI_SETTINGS, &wifi_data("Libre", ""));
        assert!(
            matches!(parse_command(&open), Ok(Command::WifiSettings { password, .. }) if password.is_empty())
        );
    }

    #[test]
    fn parses_identify_and_device_info() {
        assert_eq!(
            parse_command(&packet(CMD_IDENTIFY, &[])),
            Ok(Command::Identify)
        );
        assert_eq!(
            parse_command(&packet(CMD_DEVICE_INFO, &[])),
            Ok(Command::DeviceInfo)
        );
        assert_eq!(parse_command(&[0x02, 0x00, 0x02]), Ok(Command::Identify));
    }

    #[test]
    fn rejects_malformed_packets() {
        assert_eq!(parse_command(&[]), Err(Error::InvalidRpcPacket));
        assert_eq!(parse_command(&[0x02]), Err(Error::InvalidRpcPacket));
        let mut bad = packet(CMD_IDENTIFY, &[]);
        *bad.last_mut().unwrap() ^= 0xff;
        assert_eq!(
            parse_command(&bad),
            Err(Error::InvalidRpcPacket),
            "checksum"
        );
        let mut long = packet(CMD_WIFI_SETTINGS, &wifi_data("a", "b"));
        long[1] = 20;
        assert_eq!(
            parse_command(&long),
            Err(Error::InvalidRpcPacket),
            "longueur"
        );
        let truncated = packet(CMD_WIFI_SETTINGS, &[5, b'a', b'b']);
        assert_eq!(parse_command(&truncated), Err(Error::InvalidRpcPacket));
        let trailing = packet(
            CMD_WIFI_SETTINGS,
            &[&wifi_data("a", "b")[..], &[0][..]].concat(),
        );
        assert_eq!(parse_command(&trailing), Err(Error::InvalidRpcPacket));
        let empty_ssid = packet(CMD_WIFI_SETTINGS, &wifi_data("", "x"));
        assert_eq!(parse_command(&empty_ssid), Err(Error::InvalidRpcPacket));
        let long_ssid = packet(CMD_WIFI_SETTINGS, &wifi_data(&"s".repeat(33), ""));
        assert_eq!(parse_command(&long_ssid), Err(Error::InvalidRpcPacket));
        let bad_utf8 = packet(CMD_WIFI_SETTINGS, &[2, 0xff, 0xfe, 0]);
        assert_eq!(parse_command(&bad_utf8), Err(Error::InvalidRpcPacket));
    }

    #[test]
    fn unknown_command_is_reported_as_such() {
        assert_eq!(
            parse_command(&packet(0x42, &[])),
            Err(Error::UnknownCommand)
        );
        // Même inconnue, une commande au checksum faux est un paquet invalide.
        assert_eq!(
            parse_command(&[0x42, 0x00, 0x00]),
            Err(Error::InvalidRpcPacket)
        );
    }

    #[test]
    fn encodes_results_with_checksum() {
        let r = encode_result(CMD_WIFI_SETTINGS, &["http://a/"]);
        assert_eq!(&r[..2], &[0x01, 10]);
        assert_eq!(r[2], 9);
        assert_eq!(&r[3..12], b"http://a/");
        assert_eq!(*r.last().unwrap(), checksum(&r[..r.len() - 1]));
        let empty = encode_result(CMD_WIFI_SETTINGS, &[]);
        assert_eq!(empty, vec![0x01, 0x00, 0x01]);
        let info = DeviceInfo {
            firmware: "light-flash".into(),
            version: "0.1.0".into(),
            hardware: "esp32c3".into(),
            name: "lampe".into(),
        }
        .to_result();
        assert_eq!(info[0], CMD_DEVICE_INFO);
        assert_eq!(info[1] as usize, 4 + 11 + 5 + 7 + 5);
    }

    #[test]
    fn oversized_strings_are_dropped_not_truncated() {
        let big = "x".repeat(300);
        let r = encode_result(CMD_WIFI_SETTINGS, &[&big, "ok"]);
        assert_eq!(
            r,
            encode_result(CMD_WIFI_SETTINGS, &[]),
            "une chaîne trop longue arrête la liste"
        );
    }
}
