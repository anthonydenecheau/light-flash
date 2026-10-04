//! Rendu d'une trame à partir de l'état et du temps écoulé.

use crate::color::{hsv_to_rgb, scale_rgb, Gamma};
use crate::state::{Effect, LightState};
use rgb::RGB8;

/// Produit les trames ; conserve l'horloge des effets et la table gamma.
#[derive(Debug, Clone)]
pub struct Renderer {
    gamma: Gamma,
    clock_ms: u32,
}

impl Renderer {
    /// Période de la respiration.
    pub const BREATHE_PERIOD_MS: u32 = 4000;
    /// Niveau minimal de la respiration (environ 10 %), pour ne jamais s'éteindre.
    pub const BREATHE_FLOOR: u8 = 26;
    /// Durée d'un tour complet de teinte pour l'arc-en-ciel.
    pub const RAINBOW_PERIOD_MS: u32 = 8000;

    pub fn new(gamma: Gamma) -> Self {
        Self { gamma, clock_ms: 0 }
    }

    /// Fait avancer l'horloge des effets.
    pub fn tick(&mut self, dt_ms: u32) {
        self.clock_ms = self.clock_ms.wrapping_add(dt_ms);
    }

    pub fn clock_ms(&self) -> u32 {
        self.clock_ms
    }

    /// Remplit `frame` pour l'état donné. La trame n'est pas encore plafonnée en
    /// puissance : appeler ensuite [`crate::power::limit`].
    pub fn render(&self, state: &LightState, frame: &mut [RGB8]) {
        if !state.power || state.brightness == 0 {
            frame.fill(RGB8::new(0, 0, 0));
            return;
        }
        match state.effect {
            Effect::Solid => {
                let c = self
                    .gamma
                    .apply_rgb(scale_rgb(state.color, state.brightness));
                frame.fill(c);
            }
            Effect::Breathe => {
                let wave = triangle(self.clock_ms, Self::BREATHE_PERIOD_MS) as u32;
                let floor = Self::BREATHE_FLOOR as u32;
                let level = (floor + wave * (255 - floor) / 255) as u8;
                let c = self
                    .gamma
                    .apply_rgb(scale_rgb(scale_rgb(state.color, state.brightness), level));
                frame.fill(c);
            }
            Effect::Rainbow => {
                let n = frame.len().max(1) as u32;
                let base =
                    (self.clock_ms % Self::RAINBOW_PERIOD_MS) * 256 / Self::RAINBOW_PERIOD_MS;
                for (i, px) in frame.iter_mut().enumerate() {
                    let hue = (base + i as u32 * 256 / n) as u8; // modulo 256 par troncature
                    *px = self.gamma.apply_rgb(hsv_to_rgb(hue, 255, state.brightness));
                }
            }
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(Gamma::default())
    }
}

/// Onde triangulaire 0..=255 de période `period_ms`.
pub fn triangle(t_ms: u32, period_ms: u32) -> u8 {
    let half = period_ms / 2;
    let t = t_ms % period_ms;
    let up = if t < half { t } else { period_ms - t };
    (up * 255 / half) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LightCommand;

    const N: usize = 144;
    const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0 };

    fn linear() -> Renderer {
        Renderer::new(Gamma::new(1.0))
    }

    #[test]
    fn off_renders_black() {
        let mut frame = vec![RGB8::new(9, 9, 9); N];
        linear().render(&LightState::default(), &mut frame);
        assert!(frame.iter().all(|p| *p == BLACK));
    }

    #[test]
    fn solid_at_full_brightness_is_the_exact_color() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetColor(RGB8::new(10, 20, 30)));
        s.apply(LightCommand::SetBrightness(255));
        let mut frame = vec![BLACK; N];
        linear().render(&s, &mut frame);
        assert!(frame.iter().all(|p| *p == RGB8::new(10, 20, 30)));
    }

    #[test]
    fn solid_brightness_scales_the_color() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetColor(RGB8::new(200, 100, 0)));
        s.apply(LightCommand::SetBrightness(128));
        let mut frame = vec![BLACK; 1];
        linear().render(&s, &mut frame);
        assert_eq!(frame[0], RGB8::new(100, 50, 0));
    }

    #[test]
    fn rainbow_spreads_distinct_hues_along_the_strip() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetEffect(Effect::Rainbow));
        s.apply(LightCommand::SetBrightness(255));
        let mut frame = vec![BLACK; N];
        linear().render(&s, &mut frame);
        assert!(frame.iter().all(|p| *p != BLACK));
        assert_ne!(frame[0], frame[N / 3]);
        assert_ne!(frame[N / 3], frame[2 * N / 3]);
    }

    #[test]
    fn rainbow_moves_with_time() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetEffect(Effect::Rainbow));
        let mut r = linear();
        let mut a = vec![BLACK; N];
        r.render(&s, &mut a);
        r.tick(Renderer::RAINBOW_PERIOD_MS / 4);
        let mut b = vec![BLACK; N];
        r.render(&s, &mut b);
        assert_ne!(a[0], b[0]);
    }

    #[test]
    fn breathe_never_goes_fully_dark() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetEffect(Effect::Breathe));
        s.apply(LightCommand::SetBrightness(255));
        let mut r = linear();
        for _ in 0..(Renderer::BREATHE_PERIOD_MS / 100) {
            let mut frame = vec![BLACK; 1];
            r.render(&s, &mut frame);
            assert!(frame[0] != BLACK, "à t={} ms", r.clock_ms());
            r.tick(100);
        }
    }

    #[test]
    fn triangle_wave_shape() {
        assert_eq!(triangle(0, 4000), 0);
        assert_eq!(triangle(2000, 4000), 255);
        assert_eq!(triangle(4000, 4000), 0);
        assert!(triangle(1000, 4000) > 120 && triangle(1000, 4000) < 135);
    }
}
