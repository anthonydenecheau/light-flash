//! Politique de (re)connexion Wi-Fi, indépendante du matériel : quand retenter la station,
//! quand se replier sur le point d'accès de secours, quand retenter la station depuis celui-ci.
//!
//! Le thread réseau appelle [`Policy::on_tick`] chaque seconde avec l'état de la station et
//! exécute l'action rendue ; après une tentative de connexion il appelle
//! [`Policy::on_station_result`].

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Station,
    AccessPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Wait,
    ConnectStation,
    StartAccessPoint,
}

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// Premier délai entre deux tentatives, en ms.
    pub initial_backoff_ms: u64,
    /// Délai maximal entre deux tentatives, en ms.
    pub max_backoff_ms: u64,
    /// Durée de déconnexion continue avant le repli sur le point d'accès, en ms.
    pub fallback_after_ms: u64,
    /// Depuis le point d'accès, période des nouveaux essais de station, en ms.
    pub retry_station_every_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            initial_backoff_ms: 2_000,
            max_backoff_ms: 30_000,
            fallback_after_ms: 90_000,
            retry_station_every_ms: 120_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Policy {
    settings: Settings,
    has_credentials: bool,
    mode: Mode,
    ap_started: bool,
    backoff_ms: u64,
    next_attempt_ms: u64,
    disconnected_since_ms: Option<u64>,
    ap_since_ms: u64,
}

impl Policy {
    pub fn new(has_credentials: bool, settings: Settings) -> Self {
        Self {
            settings,
            has_credentials,
            mode: Mode::Station,
            ap_started: false,
            backoff_ms: settings.initial_backoff_ms,
            next_attempt_ms: 0,
            disconnected_since_ms: None,
            ap_since_ms: 0,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// À appeler régulièrement. `connected` : la station a une association (ignoré en mode
    /// point d'accès).
    pub fn on_tick(&mut self, now_ms: u64, connected: bool) -> Action {
        if !self.has_credentials {
            return self.enter_access_point(now_ms);
        }
        match self.mode {
            Mode::Station => {
                if connected {
                    self.disconnected_since_ms = None;
                    self.backoff_ms = self.settings.initial_backoff_ms;
                    return Action::Wait;
                }
                let since = *self.disconnected_since_ms.get_or_insert(now_ms);
                if now_ms.saturating_sub(since) >= self.settings.fallback_after_ms {
                    return self.enter_access_point(now_ms);
                }
                if now_ms >= self.next_attempt_ms {
                    Action::ConnectStation
                } else {
                    Action::Wait
                }
            }
            Mode::AccessPoint => {
                if now_ms.saturating_sub(self.ap_since_ms) >= self.settings.retry_station_every_ms {
                    Action::ConnectStation
                } else {
                    Action::Wait
                }
            }
        }
    }

    /// Résultat d'une tentative `ConnectStation`. Une tentative depuis le point d'accès l'arrête :
    /// en cas d'échec, l'action rendue le relance.
    pub fn on_station_result(&mut self, now_ms: u64, success: bool) -> Action {
        if success {
            self.mode = Mode::Station;
            self.ap_started = false;
            self.disconnected_since_ms = None;
            self.backoff_ms = self.settings.initial_backoff_ms;
            self.next_attempt_ms = now_ms;
            return Action::Wait;
        }
        match self.mode {
            Mode::Station => {
                self.next_attempt_ms = now_ms + self.backoff_ms;
                self.backoff_ms = (self.backoff_ms * 2).min(self.settings.max_backoff_ms);
                Action::Wait
            }
            Mode::AccessPoint => {
                self.ap_started = false;
                self.enter_access_point(now_ms)
            }
        }
    }

    fn enter_access_point(&mut self, now_ms: u64) -> Action {
        self.mode = Mode::AccessPoint;
        self.disconnected_since_ms = None;
        if self.ap_started {
            Action::Wait
        } else {
            self.ap_started = true;
            self.ap_since_ms = now_ms;
            Action::StartAccessPoint
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Settings = Settings {
        initial_backoff_ms: 2_000,
        max_backoff_ms: 30_000,
        fallback_after_ms: 90_000,
        retry_station_every_ms: 120_000,
    };

    /// Simule des ticks d'une seconde sur `[from, to)` où toute tentative de station échoue ;
    /// renvoie les instants des tentatives et des démarrages de point d'accès.
    fn run_failing(p: &mut Policy, from: u64, to: u64) -> (Vec<u64>, Vec<u64>) {
        let (mut attempts, mut aps) = (vec![], vec![]);
        let mut t = from;
        while t < to {
            match p.on_tick(t, false) {
                Action::ConnectStation => {
                    attempts.push(t);
                    if p.on_station_result(t, false) == Action::StartAccessPoint {
                        aps.push(t);
                    }
                }
                Action::StartAccessPoint => aps.push(t),
                Action::Wait => {}
            }
            t += 1_000;
        }
        (attempts, aps)
    }

    #[test]
    fn without_credentials_only_the_access_point_is_started_once() {
        let mut p = Policy::new(false, S);
        assert_eq!(p.on_tick(0, false), Action::StartAccessPoint);
        for t in 1..1_000u64 {
            assert_eq!(p.on_tick(t * 1_000, false), Action::Wait);
        }
        assert_eq!(p.mode(), Mode::AccessPoint);
    }

    #[test]
    fn boot_connects_then_waits_while_connected() {
        let mut p = Policy::new(true, S);
        assert_eq!(p.on_tick(0, false), Action::ConnectStation);
        assert_eq!(p.on_station_result(3_000, true), Action::Wait);
        for t in 4..100u64 {
            assert_eq!(p.on_tick(t * 1_000, true), Action::Wait);
        }
        assert_eq!(p.mode(), Mode::Station);
    }

    #[test]
    fn disconnection_is_retried_with_exponential_backoff_capped() {
        let mut p = Policy::new(true, S);
        p.on_tick(0, false);
        p.on_station_result(3_000, true);
        // Coupure à t = 10 s : tentatives à 10, 12, 16, 24, 40, 70 s (2, 4, 8, 16, 30 s)...
        let (attempts, aps) = run_failing(&mut p, 10_000, 90_000);
        assert_eq!(
            attempts,
            vec![10_000, 12_000, 16_000, 24_000, 40_000, 70_000]
        );
        assert!(aps.is_empty());
    }

    #[test]
    fn falls_back_to_access_point_after_90_s_and_retries_station_every_120_s() {
        let mut p = Policy::new(true, S);
        let (attempts, aps) = run_failing(&mut p, 0, 400_000);
        assert_eq!(aps[0], 90_000, "repli après 90 s de déconnexion : {aps:?}");
        assert!(
            attempts.iter().all(|t| *t < 90_000 || *t >= 210_000),
            "{attempts:?}"
        );
        // Depuis le point d'accès : essai station à 210 s, échec → point d'accès relancé aussitôt.
        assert!(attempts.contains(&210_000), "{attempts:?}");
        assert_eq!(aps, vec![90_000, 210_000, 330_000]);
        assert_eq!(p.mode(), Mode::AccessPoint);
    }

    #[test]
    fn station_success_from_access_point_returns_to_station_mode() {
        let mut p = Policy::new(true, S);
        run_failing(&mut p, 0, 100_000);
        assert_eq!(p.mode(), Mode::AccessPoint);
        assert_eq!(p.on_tick(210_000, false), Action::ConnectStation);
        assert_eq!(p.on_station_result(214_000, true), Action::Wait);
        assert_eq!(p.mode(), Mode::Station);
        assert_eq!(p.on_tick(215_000, true), Action::Wait);
        // Nouvelle coupure : on repart avec le petit backoff, pas le repli immédiat.
        assert_eq!(p.on_tick(300_000, false), Action::ConnectStation);
        assert_eq!(p.on_station_result(300_000, false), Action::Wait);
        assert_eq!(p.on_tick(301_000, false), Action::Wait);
        assert_eq!(p.on_tick(302_000, false), Action::ConnectStation);
    }

    #[test]
    fn reconnection_resets_the_backoff() {
        let mut p = Policy::new(true, S);
        run_failing(&mut p, 0, 50_000);
        assert!(p.on_tick(70_000, false) == Action::ConnectStation);
        p.on_station_result(72_000, true);
        assert_eq!(p.on_tick(80_000, false), Action::ConnectStation);
        p.on_station_result(80_000, false);
        assert_eq!(
            p.on_tick(82_000, false),
            Action::ConnectStation,
            "backoff revenu à 2 s"
        );
    }
}
