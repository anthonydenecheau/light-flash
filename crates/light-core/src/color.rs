//! Outils couleur : correction gamma, mise à l'échelle, conversion HSV.

use rgb::RGB8;

/// Table de correction gamma (256 entrées) pour une luminosité perçue linéaire.
#[derive(Debug, Clone)]
pub struct Gamma {
    table: [u8; 256],
}

impl Gamma {
    /// Valeur courante pour des WS2812.
    pub const DEFAULT_GAMMA: f32 = 2.2;

    /// Construit la table pour l'exposant donné (1.0 = identité).
    pub fn new(gamma: f32) -> Self {
        let mut table = [0u8; 256];
        for (i, v) in table.iter_mut().enumerate() {
            *v = ((i as f32 / 255.0).powf(gamma) * 255.0 + 0.5) as u8;
        }
        Self { table }
    }

    pub fn apply(&self, value: u8) -> u8 {
        self.table[value as usize]
    }

    pub fn apply_rgb(&self, c: RGB8) -> RGB8 {
        RGB8::new(self.apply(c.r), self.apply(c.g), self.apply(c.b))
    }
}

impl Default for Gamma {
    fn default() -> Self {
        Self::new(Self::DEFAULT_GAMMA)
    }
}

/// Multiplie une composante par un facteur 0..=255 (255 = identité), arrondi au plus proche.
pub fn scale(value: u8, factor: u8) -> u8 {
    ((value as u16 * factor as u16 + 127) / 255) as u8
}

pub fn scale_rgb(c: RGB8, factor: u8) -> RGB8 {
    RGB8::new(scale(c.r, factor), scale(c.g, factor), scale(c.b, factor))
}

/// Teinte, saturation et valeur (0..=255 chacune) vers RGB, en arithmétique entière.
pub fn hsv_to_rgb(h: u8, s: u8, v: u8) -> RGB8 {
    if s == 0 {
        return RGB8::new(v, v, v);
    }
    let region = h / 43;
    let remainder = (h as u32 - region as u32 * 43) * 6; // 0..=252
    let v32 = v as u32;
    let s32 = s as u32;
    let p = (v32 * (255 - s32) / 255) as u8;
    let q = (v32 * (255 - s32 * remainder / 255) / 255) as u8;
    let t = (v32 * (255 - s32 * (255 - remainder) / 255) / 255) as u8;
    match region {
        0 => RGB8::new(v, t, p),
        1 => RGB8::new(q, v, p),
        2 => RGB8::new(p, v, t),
        3 => RGB8::new(p, q, v),
        4 => RGB8::new(t, p, v),
        _ => RGB8::new(v, p, q),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_one_is_identity() {
        let g = Gamma::new(1.0);
        for i in 0..=255u8 {
            assert_eq!(g.apply(i), i);
        }
    }

    #[test]
    fn gamma_keeps_endpoints_and_is_monotonic() {
        let g = Gamma::default();
        assert_eq!(g.apply(0), 0);
        assert_eq!(g.apply(255), 255);
        for i in 1..=255u8 {
            assert!(g.apply(i) >= g.apply(i - 1));
        }
        // Une gamma > 1 assombrit les tons moyens.
        assert!(g.apply(128) < 128);
    }

    #[test]
    fn scale_identity_and_zero() {
        assert_eq!(scale(200, 255), 200);
        assert_eq!(scale(200, 0), 0);
        assert_eq!(scale(255, 128), 128);
        assert_eq!(
            scale_rgb(RGB8::new(255, 255, 255), 51),
            RGB8::new(51, 51, 51)
        );
    }

    #[test]
    fn hsv_primaries() {
        assert_eq!(hsv_to_rgb(0, 255, 255), RGB8::new(255, 0, 0));
        // Six secteurs de 43 : vert pur à h = 86, bleu pur à h = 172.
        assert_eq!(hsv_to_rgb(86, 255, 255), RGB8::new(0, 255, 0));
        assert_eq!(hsv_to_rgb(172, 255, 255), RGB8::new(0, 0, 255));
    }

    #[test]
    fn hsv_without_saturation_is_gray() {
        assert_eq!(hsv_to_rgb(123, 0, 77), RGB8::new(77, 77, 77));
    }

    #[test]
    fn hsv_value_bounds_output() {
        for h in 0..=255u8 {
            let c = hsv_to_rgb(h, 255, 100);
            assert!(c.r <= 100 && c.g <= 100 && c.b <= 100);
            assert!(c.r == 100 || c.g == 100 || c.b == 100, "h={h} {c:?}");
        }
    }
}
