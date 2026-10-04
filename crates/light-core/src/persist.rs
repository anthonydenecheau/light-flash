//! Décide quand enregistrer l'état : après une période de calme sans changement, et seulement
//! s'il diffère de ce qui est déjà enregistré. Protège la flash des rafales de commandes
//! (curseur de luminosité, effets) : une rafale ne coûte qu'une écriture.

use crate::state::LightState;

#[derive(Debug, Clone)]
pub struct SaveScheduler {
    quiet_ms: u64,
    /// Dernier état enregistré (ou `None` si rien n'est enregistré / la dernière écriture a échoué).
    saved: Option<LightState>,
    /// État candidat et instant de son dernier changement.
    pending: Option<(LightState, u64)>,
}

impl SaveScheduler {
    /// Calme requis avant d'écrire, en ms.
    pub const DEFAULT_QUIET_MS: u64 = 2000;

    pub fn new(saved: Option<LightState>, quiet_ms: u64) -> Self {
        Self {
            quiet_ms,
            saved,
            pending: None,
        }
    }

    /// À appeler régulièrement avec l'état courant et une horloge monotone en ms.
    /// Renvoie l'état à enregistrer quand il est resté stable `quiet_ms` et diffère de
    /// l'état enregistré ; l'appelant écrit, puis signale un échec via [`Self::save_failed`].
    pub fn observe(&mut self, state: LightState, now_ms: u64) -> Option<LightState> {
        if self.saved == Some(state) {
            self.pending = None;
            return None;
        }
        match self.pending {
            Some((candidate, since)) if candidate == state => {
                if now_ms.saturating_sub(since) >= self.quiet_ms {
                    self.saved = Some(state);
                    self.pending = None;
                    Some(state)
                } else {
                    None
                }
            }
            _ => {
                self.pending = Some((state, now_ms));
                None
            }
        }
    }

    /// L'écriture renvoyée par [`Self::observe`] a échoué : elle sera retentée après un nouveau
    /// délai de calme.
    pub fn save_failed(&mut self) {
        self.saved = None;
    }

    pub fn saved(&self) -> Option<LightState> {
        self.saved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LightCommand;
    use rgb::RGB8;

    const QUIET: u64 = 2000;

    fn changed(seed: u8) -> LightState {
        let mut s = LightState::default();
        s.apply(LightCommand::SetColor(RGB8::new(seed, 0, 0)));
        s
    }

    #[test]
    fn unchanged_state_is_never_saved() {
        let base = LightState::default();
        let mut sch = SaveScheduler::new(Some(base), QUIET);
        for t in (0..10_000).step_by(250) {
            assert_eq!(sch.observe(base, t), None);
        }
    }

    #[test]
    fn saves_once_after_quiet_period() {
        let mut sch = SaveScheduler::new(Some(LightState::default()), QUIET);
        let s = changed(1);
        assert_eq!(sch.observe(s, 1000), None);
        assert_eq!(sch.observe(s, 2500), None, "pas encore 2 s de calme");
        assert_eq!(sch.observe(s, 3000), Some(s));
        assert_eq!(sch.observe(s, 3250), None, "déjà enregistré");
        assert_eq!(sch.saved(), Some(s));
    }

    #[test]
    fn a_burst_of_changes_costs_a_single_write() {
        let mut sch = SaveScheduler::new(Some(LightState::default()), QUIET);
        let mut writes = 0;
        // Curseur bougé toutes les 100 ms pendant 5 s.
        for i in 0..50u64 {
            if sch.observe(changed(i as u8 + 1), i * 100).is_some() {
                writes += 1;
            }
        }
        assert_eq!(writes, 0);
        let last = changed(50);
        assert_eq!(sch.observe(last, 5000), None);
        assert_eq!(sch.observe(last, 4900 + QUIET), Some(last));
        assert_eq!(sch.observe(last, 20_000), None);
    }

    #[test]
    fn reverting_before_the_quiet_period_cancels_the_write() {
        let base = LightState::default();
        let mut sch = SaveScheduler::new(Some(base), QUIET);
        assert_eq!(sch.observe(changed(1), 0), None);
        assert_eq!(sch.observe(base, 500), None);
        assert_eq!(sch.observe(base, 10_000), None);
    }

    #[test]
    fn failed_write_is_retried_after_another_quiet_period() {
        let mut sch = SaveScheduler::new(None, QUIET);
        let s = changed(1);
        assert_eq!(sch.observe(s, 0), None);
        assert_eq!(sch.observe(s, QUIET), Some(s));
        sch.save_failed();
        assert_eq!(
            sch.observe(s, QUIET + 100),
            None,
            "réarmé, pas de réécriture immédiate"
        );
        assert_eq!(sch.observe(s, 2 * QUIET + 100), Some(s));
    }

    #[test]
    fn nothing_saved_yet_saves_the_initial_state_after_quiet() {
        let mut sch = SaveScheduler::new(None, QUIET);
        let s = LightState::default();
        assert_eq!(sch.observe(s, 0), None);
        assert_eq!(sch.observe(s, QUIET), Some(s));
    }
}
