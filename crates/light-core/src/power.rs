//! Budget de puissance : estimation du courant d'une trame et plafonnement.
//!
//! C'est la protection principale de l'alimentation et du ruban (voir HARDWARE.md).

use rgb::RGB8;

/// Courant d'un canal à 255, en mA (fiche WS2812B : environ 20 mA par couleur).
pub const MA_PER_CHANNEL: u32 = 20;
/// Courant de veille d'une LED éteinte, en mA.
pub const MA_IDLE: u32 = 1;

/// Estime le courant d'une trame, en mA.
pub fn estimate_ma(frame: &[RGB8]) -> u32 {
    let sum: u32 = frame
        .iter()
        .map(|p| p.r as u32 + p.g as u32 + p.b as u32)
        .sum();
    sum * MA_PER_CHANNEL / 255 + frame.len() as u32 * MA_IDLE
}

/// Réduit uniformément la trame pour rester sous `max_ma`.
/// Retourne le courant estimé après réduction.
pub fn limit(frame: &mut [RGB8], max_ma: u32) -> u32 {
    let estimated = estimate_ma(frame);
    if estimated <= max_ma {
        return estimated;
    }
    let idle = frame.len() as u32 * MA_IDLE;
    if max_ma <= idle {
        frame.iter_mut().for_each(|p| *p = RGB8::new(0, 0, 0));
        return idle;
    }
    let num = max_ma - idle;
    let den = estimated - idle;
    for p in frame.iter_mut() {
        p.r = (p.r as u32 * num / den) as u8;
        p.g = (p.g as u32 * num / den) as u8;
        p.b = (p.b as u32 * num / den) as u8;
    }
    estimate_ma(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 144;

    #[test]
    fn full_white_strip_is_about_8_7_amps() {
        let frame = vec![RGB8::new(255, 255, 255); N];
        assert_eq!(
            estimate_ma(&frame),
            N as u32 * (3 * MA_PER_CHANNEL + MA_IDLE)
        );
    }

    #[test]
    fn off_strip_only_draws_idle_current() {
        let frame = vec![RGB8::new(0, 0, 0); N];
        assert_eq!(estimate_ma(&frame), N as u32 * MA_IDLE);
    }

    #[test]
    fn limit_leaves_a_frame_under_budget_untouched() {
        let mut frame = vec![RGB8::new(10, 20, 30); N];
        let before = frame.clone();
        let ma = limit(&mut frame, 5000);
        assert_eq!(frame, before);
        assert_eq!(ma, estimate_ma(&before));
    }

    #[test]
    fn limit_scales_a_frame_down_to_the_budget() {
        let mut frame = vec![RGB8::new(255, 200, 100); N];
        let ma = limit(&mut frame, 5000);
        assert!(ma <= 5000, "{ma}");
        assert!(ma > 4900, "réduction trop forte : {ma}");
        // Les proportions entre canaux sont conservées à l'arrondi près.
        let p = frame[0];
        assert!(p.r > p.g && p.g > p.b, "{p:?}");
    }

    #[test]
    fn limit_below_idle_current_blanks_the_strip() {
        let mut frame = vec![RGB8::new(255, 255, 255); N];
        let ma = limit(&mut frame, 10);
        assert_eq!(ma, N as u32 * MA_IDLE);
        assert!(frame.iter().all(|p| *p == RGB8::new(0, 0, 0)));
    }
}
