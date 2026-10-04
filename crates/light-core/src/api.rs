//! Représentation « transport » de l'état et des modifications (JSON sur HTTP, plus tard BLE).

use crate::state::{Effect, LightCommand, LightState};
use rgb::RGB8;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Vue sérialisable de l'état, renvoyée par `GET /api/light`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightView {
    pub power: bool,
    /// Couleur au format `#rrggbb`.
    pub color: String,
    pub brightness: u8,
    /// `solid`, `breathe` ou `rainbow`.
    pub effect: String,
}

impl From<&LightState> for LightView {
    fn from(s: &LightState) -> Self {
        Self {
            power: s.power,
            color: format_color(s.color),
            brightness: s.brightness,
            effect: s.effect.name().to_owned(),
        }
    }
}

/// Modification partielle reçue par `POST /api/light` : chaque champ présent devient une commande.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightPatch {
    pub power: Option<bool>,
    /// `#rrggbb` ou `rrggbb`.
    pub color: Option<String>,
    pub brightness: Option<u8>,
    pub effect: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    BadColor(String),
    BadEffect(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::BadColor(c) => write!(f, "couleur invalide « {c} », attendu #rrggbb"),
            ApiError::BadEffect(e) => write!(
                f,
                "effet inconnu « {e} », attendu solid, breathe ou rainbow"
            ),
        }
    }
}

impl std::error::Error for ApiError {}

impl LightPatch {
    /// Traduit la modification en commandes, dans un ordre tel qu'un `power: false` explicite
    /// l'emporte sur l'allumage implicite déclenché par les autres champs.
    pub fn into_commands(self) -> Result<Vec<LightCommand>, ApiError> {
        let mut cmds = Vec::with_capacity(4);
        if let Some(e) = self.effect {
            let effect = Effect::from_name(&e).ok_or(ApiError::BadEffect(e))?;
            cmds.push(LightCommand::SetEffect(effect));
        }
        if let Some(c) = self.color {
            let color = parse_color(&c).ok_or(ApiError::BadColor(c))?;
            cmds.push(LightCommand::SetColor(color));
        }
        if let Some(b) = self.brightness {
            cmds.push(LightCommand::SetBrightness(b));
        }
        if let Some(p) = self.power {
            cmds.push(if p {
                LightCommand::On
            } else {
                LightCommand::Off
            });
        }
        Ok(cmds)
    }
}

/// `#rrggbb` ou `rrggbb`, insensible à la casse.
pub fn parse_color(s: &str) -> Option<RGB8> {
    let hex = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(RGB8::new(byte(0)?, byte(2)?, byte(4)?))
}

pub fn format_color(c: RGB8) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_round_trip() {
        for s in ["#ff8800", "FF8800", " #ff8800 "] {
            assert_eq!(parse_color(s), Some(RGB8::new(255, 136, 0)), "{s}");
        }
        assert_eq!(format_color(RGB8::new(255, 136, 0)), "#ff8800");
        for bad in ["", "#fff", "#gg0000", "#ff88000", "rouge"] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
    }

    #[test]
    fn patch_becomes_ordered_commands() {
        let patch: LightPatch = serde_json::from_str(
            r##"{"power":false,"color":"#102030","brightness":40,"effect":"rainbow"}"##,
        )
        .unwrap();
        assert_eq!(
            patch.into_commands().unwrap(),
            vec![
                LightCommand::SetEffect(Effect::Rainbow),
                LightCommand::SetColor(RGB8::new(16, 32, 48)),
                LightCommand::SetBrightness(40),
                LightCommand::Off,
            ]
        );
    }

    #[test]
    fn applying_a_patch_with_power_false_leaves_the_lamp_off() {
        let patch = LightPatch {
            power: Some(false),
            color: Some("#ffffff".into()),
            ..Default::default()
        };
        let mut state = LightState::default();
        for cmd in patch.into_commands().unwrap() {
            state.apply(cmd);
        }
        assert!(!state.power);
        assert_eq!(state.color, RGB8::new(255, 255, 255));
    }

    #[test]
    fn patch_rejects_bad_values_and_unknown_fields() {
        let bad_effect = LightPatch {
            effect: Some("disco".into()),
            ..Default::default()
        };
        assert!(matches!(
            bad_effect.into_commands(),
            Err(ApiError::BadEffect(_))
        ));
        let bad_color = LightPatch {
            color: Some("bleu".into()),
            ..Default::default()
        };
        assert!(matches!(
            bad_color.into_commands(),
            Err(ApiError::BadColor(_))
        ));
        assert!(serde_json::from_str::<LightPatch>(r##"{"colour":"#000000"}"##).is_err());
        assert!(serde_json::from_str::<LightPatch>(r#"{"brightness":300}"#).is_err());
    }

    #[test]
    fn view_serializes_state() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetColor(RGB8::new(1, 2, 3)));
        let view = LightView::from(&s);
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains(r##""color":"#010203""##), "{json}");
        assert!(json.contains(r#""power":true"#), "{json}");
        assert_eq!(serde_json::from_str::<LightView>(&json).unwrap(), view);
    }
}
