//! Appuis court et long sur un bouton, avec anti-rebond, à partir d'un échantillonnage régulier.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    None,
    Short,
    Long,
}

#[derive(Debug, Clone, Default)]
pub struct ButtonTracker {
    pressed_since_ms: Option<u64>,
    long_fired: bool,
}

impl ButtonTracker {
    /// Durée minimale d'un appui pris en compte (anti-rebond), en ms.
    pub const DEBOUNCE_MS: u64 = 50;
    /// Durée à partir de laquelle un appui est « long », en ms.
    pub const LONG_PRESS_MS: u64 = 5_000;

    /// À appeler à chaque échantillon. `Long` est rendu une seule fois, dès que la durée est
    /// atteinte (sans attendre le relâchement) ; `Short` au relâchement d'un appui bref.
    pub fn update(&mut self, pressed: bool, now_ms: u64) -> Press {
        match (pressed, self.pressed_since_ms) {
            (true, None) => {
                self.pressed_since_ms = Some(now_ms);
                self.long_fired = false;
                Press::None
            }
            (true, Some(since)) => {
                if !self.long_fired && now_ms.saturating_sub(since) >= Self::LONG_PRESS_MS {
                    self.long_fired = true;
                    Press::Long
                } else {
                    Press::None
                }
            }
            (false, Some(since)) => {
                self.pressed_since_ms = None;
                if !self.long_fired && now_ms.saturating_sub(since) >= Self::DEBOUNCE_MS {
                    Press::Short
                } else {
                    Press::None
                }
            }
            (false, None) => Press::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_press_is_reported_on_release() {
        let mut b = ButtonTracker::default();
        assert_eq!(b.update(true, 0), Press::None);
        assert_eq!(b.update(true, 100), Press::None);
        assert_eq!(b.update(false, 200), Press::Short);
        assert_eq!(b.update(false, 300), Press::None);
    }

    #[test]
    fn bounces_are_ignored() {
        let mut b = ButtonTracker::default();
        b.update(true, 0);
        assert_eq!(b.update(false, 20), Press::None);
    }

    #[test]
    fn long_press_fires_once_while_held_and_not_again_on_release() {
        let mut b = ButtonTracker::default();
        b.update(true, 0);
        assert_eq!(b.update(true, 4_999), Press::None);
        assert_eq!(b.update(true, 5_000), Press::Long);
        assert_eq!(b.update(true, 8_000), Press::None);
        assert_eq!(b.update(false, 9_000), Press::None);
        assert_eq!(b.update(true, 10_000), Press::None);
        assert_eq!(b.update(false, 10_200), Press::Short);
    }
}
