//! Nom de la lampe choisi par l'utilisateur (« Salon 1 ») et nom d'hôte qui en dérive
//! (`salon-1`, utilisé pour mDNS, DHCP, le point d'accès et l'annonce BLE).

/// Longueur maximale du nom affiché, en caractères.
pub const NAME_MAX: usize = 32;
/// Longueur maximale du nom d'hôte (contrainte ESP-IDF : 32 octets).
pub const HOSTNAME_MAX: usize = 32;

/// Nom par défaut, unique par carte : `light-flash-` suivi des deux derniers octets de
/// l'adresse MAC.
pub fn default_name(mac: [u8; 6]) -> String {
    format!("light-flash-{:02x}{:02x}", mac[4], mac[5])
}

/// Valide un nom saisi : espaces en trop retirés, 1 à 32 caractères, pas de caractère de
/// contrôle.
pub fn validate_name(name: &str) -> Result<String, &'static str> {
    let name = name.trim();
    if name.is_empty() {
        return Err("le nom est vide");
    }
    if name.chars().count() > NAME_MAX {
        return Err("le nom dépasse 32 caractères");
    }
    if name.chars().any(char::is_control) {
        return Err("le nom contient un caractère invalide");
    }
    Ok(name.to_owned())
}

/// Nom d'hôte dérivé du nom affiché : minuscules ASCII, lettres, chiffres et tirets, accents
/// français retirés, 32 octets maximum, jamais vide (`fallback` sinon).
pub fn hostname_from(name: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in name.chars() {
        let mapped = unaccent(c);
        for m in mapped.chars() {
            let m = m.to_ascii_lowercase();
            if m.is_ascii_alphanumeric() {
                out.push(m);
                last_dash = false;
            } else if !last_dash {
                out.push('-');
                last_dash = true;
            }
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(HOSTNAME_MAX);
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        fallback.to_owned()
    } else {
        out
    }
}

fn unaccent(c: char) -> &'static str {
    match c {
        'à' | 'â' | 'ä' | 'á' | 'ã' | 'À' | 'Â' | 'Ä' | 'Á' => "a",
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => "e",
        'î' | 'ï' | 'í' | 'Î' | 'Ï' => "i",
        'ô' | 'ö' | 'ó' | 'õ' | 'Ô' | 'Ö' => "o",
        'ù' | 'û' | 'ü' | 'ú' | 'Ù' | 'Û' | 'Ü' => "u",
        'ç' | 'Ç' => "c",
        'ñ' | 'Ñ' => "n",
        'œ' | 'Œ' => "oe",
        'æ' | 'Æ' => "ae",
        _ => {
            // Un caractère ASCII est rendu tel quel ; tout autre devient un séparateur.
            if c.is_ascii() {
                // SAFETY-free : on reconstruit un &'static str via une table ASCII.
                return ascii_str(c);
            }
            "-"
        }
    }
}

/// `&'static str` d'un caractère ASCII (table des 128 premiers codes).
fn ascii_str(c: char) -> &'static str {
    const TABLE: &str = "\0\u{1}\u{2}\u{3}\u{4}\u{5}\u{6}\u{7}\u{8}\t\n\u{b}\u{c}\r\u{e}\u{f}\u{10}\u{11}\u{12}\u{13}\u{14}\u{15}\u{16}\u{17}\u{18}\u{19}\u{1a}\u{1b}\u{1c}\u{1d}\u{1e}\u{1f} !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\u{7f}";
    let i = c as usize;
    &TABLE[i..i + 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_name_uses_mac_tail() {
        assert_eq!(
            default_name([0x34, 0x85, 0x18, 0x00, 0xdf, 0x60]),
            "light-flash-df60"
        );
    }

    #[test]
    fn validate_trims_and_bounds() {
        assert_eq!(validate_name("  Salon 1 "), Ok("Salon 1".into()));
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(33)).is_err());
        assert!(validate_name("a\nb").is_err());
        assert_eq!(validate_name(&"é".repeat(32)), Ok("é".repeat(32)));
    }

    #[test]
    fn hostname_is_dns_friendly() {
        assert_eq!(hostname_from("Salon 1", "x"), "salon-1");
        assert_eq!(hostname_from("Chambre d'Élise", "x"), "chambre-d-elise");
        assert_eq!(hostname_from("  --Cuisine--  ", "x"), "cuisine");
        assert_eq!(hostname_from("Lampe n°2", "x"), "lampe-n-2");
        assert_eq!(hostname_from("日本", "light-flash"), "light-flash");
        assert_eq!(hostname_from("light-flash-df60", "x"), "light-flash-df60");
        let long = hostname_from(&"a".repeat(40), "x");
        assert_eq!(long.len(), HOSTNAME_MAX);
        assert_eq!(
            hostname_from(&format!("{}-b", "a".repeat(31)), "x").len(),
            31,
            "tiret final retiré"
        );
    }
}
