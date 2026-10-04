//! Signalisation visuelle des états système (provisioning, identification) : remplace
//! temporairement le rendu normal de la lampe.

use crate::render::triangle;
use rgb::RGB8;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indication {
    /// Clignotement blanc rapide : « c'est moi » (commande Improv Identify).
    Identify,
    /// Respiration bleue : fenêtre d'autorisation ouverte, en attente des identifiants.
    Authorized,
    /// Clignotement bleu rapide : connexion en cours.
    Provisioning,
    /// Vert fixe : identifiants acceptés.
    Success,
    /// Rouge fixe : connexion impossible.
    Failure,
}

/// Indication courante partagée avec la tâche lumière ; `None` = rendu normal.
pub type SharedIndication = Arc<Mutex<Option<Indication>>>;

impl Indication {
    /// Durée d'affichage des indications ponctuelles, en ms ; `None` = tant que l'état dure.
    pub fn duration_ms(self) -> Option<u64> {
        match self {
            Indication::Identify | Indication::Success | Indication::Failure => Some(2_000),
            Indication::Authorized | Indication::Provisioning => None,
        }
    }
}

/// Niveau maximal des indications (pas de plein blanc dans les yeux).
const LEVEL: u8 = 120;
const FLOOR: u8 = 20;

pub fn render(indication: Indication, clock_ms: u32, frame: &mut [RGB8]) {
    let off = RGB8::new(0, 0, 0);
    let color = match indication {
        Indication::Identify => {
            if (clock_ms / 125) % 2 == 0 {
                RGB8::new(LEVEL, LEVEL, LEVEL)
            } else {
                off
            }
        }
        Indication::Authorized => {
            let wave = triangle(clock_ms, 2_000) as u32;
            let level = FLOOR as u32 + wave * (LEVEL - FLOOR) as u32 / 255;
            RGB8::new(0, 0, level as u8)
        }
        Indication::Provisioning => {
            if (clock_ms / 100) % 2 == 0 {
                RGB8::new(0, 0, LEVEL)
            } else {
                off
            }
        }
        Indication::Success => RGB8::new(0, LEVEL, 0),
        Indication::Failure => RGB8::new(LEVEL, 0, 0),
    };
    frame.fill(color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_blinks_white() {
        let mut f = [RGB8::new(1, 1, 1); 3];
        render(Indication::Identify, 0, &mut f);
        assert!(f.iter().all(|p| *p == RGB8::new(LEVEL, LEVEL, LEVEL)));
        render(Indication::Identify, 125, &mut f);
        assert!(f.iter().all(|p| *p == RGB8::new(0, 0, 0)));
    }

    #[test]
    fn authorized_breathes_blue_without_going_dark() {
        let mut f = [RGB8::new(0, 0, 0); 1];
        for t in (0..4_000).step_by(50) {
            render(Indication::Authorized, t, &mut f);
            assert_eq!((f[0].r, f[0].g), (0, 0));
            assert!(f[0].b >= FLOOR && f[0].b <= LEVEL, "t={t} b={}", f[0].b);
        }
    }

    #[test]
    fn success_and_failure_are_solid() {
        let mut f = [RGB8::new(0, 0, 0); 2];
        render(Indication::Success, 123, &mut f);
        assert_eq!(f[1], RGB8::new(0, LEVEL, 0));
        render(Indication::Failure, 456, &mut f);
        assert_eq!(f[0], RGB8::new(LEVEL, 0, 0));
        assert_eq!(Indication::Success.duration_ms(), Some(2_000));
        assert_eq!(Indication::Authorized.duration_ms(), None);
    }
}
