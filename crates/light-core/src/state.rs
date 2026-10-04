//! État de la lampe et commandes qui le modifient.

use rgb::RGB8;

/// Effet appliqué à la trame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Effect {
    /// Couleur unie.
    #[default]
    Solid,
    /// Respiration : la luminosité oscille lentement.
    Breathe,
    /// Arc-en-ciel qui défile le long du ruban.
    Rainbow,
}

impl Effect {
    /// Identifiant transporté sur BLE / HTTP (un octet).
    pub fn to_u8(self) -> u8 {
        match self {
            Effect::Solid => 0,
            Effect::Breathe => 1,
            Effect::Rainbow => 2,
        }
    }

    /// Inverse de [`Effect::to_u8`]. `None` si la valeur est inconnue.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Effect::Solid),
            1 => Some(Effect::Breathe),
            2 => Some(Effect::Rainbow),
            _ => None,
        }
    }

    /// Nom transporté en JSON.
    pub fn name(self) -> &'static str {
        match self {
            Effect::Solid => "solid",
            Effect::Breathe => "breathe",
            Effect::Rainbow => "rainbow",
        }
    }

    /// Inverse de [`Effect::name`], insensible à la casse.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "solid" => Some(Effect::Solid),
            "breathe" => Some(Effect::Breathe),
            "rainbow" => Some(Effect::Rainbow),
            _ => None,
        }
    }

    pub const ALL: [Effect; 3] = [Effect::Solid, Effect::Breathe, Effect::Rainbow];
}

/// Commande reçue d'un canal de pilotage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightCommand {
    Off,
    On,
    Toggle,
    SetColor(RGB8),
    /// 0 = éteint, 255 = maximum (avant plafond de puissance).
    SetBrightness(u8),
    SetEffect(Effect),
}

/// État courant de la lampe. C'est ce qui est persisté en NVS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightState {
    pub power: bool,
    pub color: RGB8,
    pub brightness: u8,
    pub effect: Effect,
}

impl LightState {
    /// Luminosité rétablie quand on allume une lampe dont la luminosité est à 0.
    pub const DEFAULT_BRIGHTNESS: u8 = 160;

    /// Applique une commande. Régler la couleur, la luminosité ou l'effet allume
    /// la lampe ; une luminosité nulle l'éteint.
    pub fn apply(&mut self, cmd: LightCommand) {
        match cmd {
            LightCommand::Off => self.power = false,
            LightCommand::On => self.turn_on(),
            LightCommand::Toggle => {
                if self.power {
                    self.power = false;
                } else {
                    self.turn_on();
                }
            }
            LightCommand::SetColor(color) => {
                self.color = color;
                self.turn_on();
            }
            LightCommand::SetBrightness(0) => {
                self.brightness = 0;
                self.power = false;
            }
            LightCommand::SetBrightness(brightness) => {
                self.brightness = brightness;
                self.power = true;
            }
            LightCommand::SetEffect(effect) => {
                self.effect = effect;
                self.turn_on();
            }
        }
    }

    fn turn_on(&mut self) {
        if self.brightness == 0 {
            self.brightness = Self::DEFAULT_BRIGHTNESS;
        }
        self.power = true;
    }

    /// Version du format de sérialisation (NVS, plus tard BLE).
    pub const FORMAT_VERSION: u8 = 1;
    pub const SERIALIZED_LEN: usize = 7;

    /// Forme compacte : `[version, power, r, g, b, brightness, effect]`.
    pub fn to_bytes(&self) -> [u8; Self::SERIALIZED_LEN] {
        [
            Self::FORMAT_VERSION,
            self.power as u8,
            self.color.r,
            self.color.g,
            self.color.b,
            self.brightness,
            self.effect.to_u8(),
        ]
    }

    /// Inverse de [`LightState::to_bytes`]. `None` si la taille, la version ou l'effet
    /// est inconnu : l'appelant repart alors de l'état par défaut.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match *bytes {
            [Self::FORMAT_VERSION, power, r, g, b, brightness, effect] => Some(Self {
                power: power != 0,
                color: RGB8::new(r, g, b),
                brightness,
                effect: Effect::from_u8(effect)?,
            }),
            _ => None,
        }
    }
}

impl core::fmt::Display for LightState {
    /// Pour les journaux : `allumée #ffb464 160/255 solid`.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} #{:02x}{:02x}{:02x} {}/255 {}",
            if self.power { "allumée" } else { "éteinte" },
            self.color.r,
            self.color.g,
            self.color.b,
            self.brightness,
            self.effect.name()
        )
    }
}

impl Default for LightState {
    /// Éteinte, blanc chaud, luminosité modérée, couleur unie.
    fn default() -> Self {
        Self {
            power: false,
            color: RGB8::new(255, 180, 100),
            brightness: Self::DEFAULT_BRIGHTNESS,
            effect: Effect::Solid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_off() {
        assert!(!LightState::default().power);
    }

    #[test]
    fn on_off_toggle() {
        let mut s = LightState::default();
        s.apply(LightCommand::On);
        assert!(s.power);
        s.apply(LightCommand::Off);
        assert!(!s.power);
        s.apply(LightCommand::Toggle);
        assert!(s.power);
        s.apply(LightCommand::Toggle);
        assert!(!s.power);
    }

    #[test]
    fn setting_color_turns_on() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetColor(RGB8::new(1, 2, 3)));
        assert!(s.power);
        assert_eq!(s.color, RGB8::new(1, 2, 3));
    }

    #[test]
    fn zero_brightness_turns_off_and_on_restores_a_usable_level() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetBrightness(0));
        assert!(!s.power);
        assert_eq!(s.brightness, 0);
        s.apply(LightCommand::On);
        assert!(s.power);
        assert_eq!(s.brightness, LightState::DEFAULT_BRIGHTNESS);
    }

    #[test]
    fn state_round_trips_through_bytes() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetColor(RGB8::new(1, 2, 3)));
        s.apply(LightCommand::SetEffect(Effect::Rainbow));
        s.apply(LightCommand::SetBrightness(77));
        let bytes = s.to_bytes();
        assert_eq!(bytes, [1, 1, 1, 2, 3, 77, 2]);
        assert_eq!(LightState::from_bytes(&bytes), Some(s));
        s.apply(LightCommand::Off);
        assert_eq!(LightState::from_bytes(&s.to_bytes()), Some(s));
    }

    #[test]
    fn from_bytes_rejects_bad_input() {
        assert_eq!(LightState::from_bytes(&[]), None);
        assert_eq!(
            LightState::from_bytes(&[1, 1, 0, 0, 0, 10]),
            None,
            "trop court"
        );
        assert_eq!(
            LightState::from_bytes(&[2, 1, 0, 0, 0, 10, 0]),
            None,
            "version inconnue"
        );
        assert_eq!(
            LightState::from_bytes(&[1, 1, 0, 0, 0, 10, 9]),
            None,
            "effet inconnu"
        );
    }

    #[test]
    fn display_is_compact() {
        assert_eq!(
            LightState::default().to_string(),
            "éteinte #ffb464 160/255 solid"
        );
    }

    #[test]
    fn effect_round_trips_through_u8_and_name() {
        for e in Effect::ALL {
            assert_eq!(Effect::from_u8(e.to_u8()), Some(e));
            assert_eq!(Effect::from_name(e.name()), Some(e));
        }
        assert_eq!(Effect::from_u8(42), None);
        assert_eq!(Effect::from_name("RAINBOW "), Some(Effect::Rainbow));
        assert_eq!(Effect::from_name("disco"), None);
    }
}
