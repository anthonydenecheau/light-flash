//! Programmation : minuterie d'extinction et horaires hebdomadaires, évalués sur l'heure locale
//! fournie par le firmware (SNTP + fuseau). Sans dépendance ESP, testable sur l'hôte.

use serde::{Deserialize, Serialize};

pub const ENTRIES_MAX: usize = 8;

/// Jours de la semaine, bit 0 = lundi … bit 6 = dimanche.
pub const MONDAY: u8 = 1 << 0;
pub const SUNDAY: u8 = 1 << 6;
pub const EVERY_DAY: u8 = 0x7f;
pub const WEEKDAYS: u8 = 0x1f;
pub const WEEKEND: u8 = 0x60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Off,
    On,
    Scene(u8),
}

/// Nature de l'action en JSON ; la scène visée est dans le champ `scene` de l'entrée
/// (pas de `#[serde(flatten)]`, trop coûteux en code sur la cible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionKind {
    Off,
    On,
    Scene,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// Masque des jours (bit 0 = lundi).
    pub days: u8,
    pub hour: u8,
    pub minute: u8,
    pub action: ActionKind,
    /// Identifiant de scène, pour `action = scene`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub scene: u8,
}

fn enabled_default() -> bool {
    true
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

impl Entry {
    pub fn new(days: u8, hour: u8, minute: u8, action: Action) -> Self {
        let (kind, scene) = match action {
            Action::Off => (ActionKind::Off, 0),
            Action::On => (ActionKind::On, 0),
            Action::Scene(id) => (ActionKind::Scene, id),
        };
        Self {
            enabled: true,
            days,
            hour,
            minute,
            action: kind,
            scene,
        }
    }

    pub fn action(&self) -> Action {
        match self.action {
            ActionKind::Off => Action::Off,
            ActionKind::On => Action::On,
            ActionKind::Scene => Action::Scene(self.scene),
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.days & EVERY_DAY == 0 {
            return Err("aucun jour sélectionné");
        }
        if self.hour > 23 || self.minute > 59 {
            return Err("heure invalide");
        }
        if self.action == ActionKind::Scene && self.scene == 0 {
            return Err("scène manquante");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub entries: Vec<Entry>,
}

impl Schedule {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.entries.len() > ENTRIES_MAX {
            return Err("8 programmations maximum");
        }
        self.entries.iter().try_for_each(Entry::validate)
    }
}

/// Instant local : jour de semaine (0 = lundi), heure, minute, et un numéro de minute absolu
/// (ex. minutes depuis l'epoch) pour ne déclencher chaque entrée qu'une fois par minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub minute_index: u64,
}

/// Évalue le programme à intervalle régulier et rend les actions dues, chacune une seule fois.
#[derive(Debug, Clone, Default)]
pub struct Runner {
    last_minute: Option<u64>,
}

impl Runner {
    /// À appeler toutes les quelques secondes. Les entrées dues à cette minute sont rendues la
    /// première fois que la minute est vue ; un appel à la minute suivante ne les rejoue pas.
    /// Après un redémarrage ou une remise à l'heure, les minutes sautées ne sont pas rattrapées.
    pub fn due(&mut self, schedule: &Schedule, now: LocalTime) -> Vec<Action> {
        if self.last_minute == Some(now.minute_index) {
            return Vec::new();
        }
        self.last_minute = Some(now.minute_index);
        schedule
            .entries
            .iter()
            .filter(|e| {
                e.enabled
                    && e.days & (1 << now.weekday) != 0
                    && e.hour == now.hour
                    && e.minute == now.minute
            })
            .map(Entry::action)
            .collect()
    }
}

/// Minuterie d'extinction, en millisecondes d'horloge monotone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SleepTimer {
    off_at_ms: Option<u64>,
}

impl SleepTimer {
    pub fn set(&mut self, now_ms: u64, minutes: u32) {
        self.off_at_ms = (minutes > 0).then(|| now_ms + minutes as u64 * 60_000);
    }

    pub fn cancel(&mut self) {
        self.off_at_ms = None;
    }

    pub fn remaining_s(&self, now_ms: u64) -> Option<u64> {
        self.off_at_ms
            .map(|t| t.saturating_sub(now_ms).div_ceil(1_000))
    }

    /// `true` une seule fois, quand l'échéance est atteinte.
    pub fn fire(&mut self, now_ms: u64) -> bool {
        match self.off_at_ms {
            Some(t) if now_ms >= t => {
                self.off_at_ms = None;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(weekday: u8, hour: u8, minute: u8) -> LocalTime {
        LocalTime {
            weekday,
            hour,
            minute,
            minute_index: weekday as u64 * 1440 + hour as u64 * 60 + minute as u64,
        }
    }

    fn entry(days: u8, hour: u8, minute: u8, action: Action) -> Entry {
        Entry::new(days, hour, minute, action)
    }

    #[test]
    fn fires_once_per_minute_on_matching_days() {
        let schedule = Schedule {
            entries: vec![
                entry(WEEKDAYS, 7, 30, Action::Scene(1)),
                entry(EVERY_DAY, 23, 0, Action::Off),
            ],
        };
        let mut r = Runner::default();
        assert_eq!(r.due(&schedule, at(0, 7, 29)), vec![]);
        assert_eq!(r.due(&schedule, at(0, 7, 30)), vec![Action::Scene(1)]);
        assert_eq!(
            r.due(&schedule, at(0, 7, 30)),
            vec![],
            "même minute : pas de rejeu"
        );
        assert_eq!(r.due(&schedule, at(0, 7, 31)), vec![]);
        assert_eq!(
            r.due(&schedule, at(5, 7, 30)),
            vec![],
            "samedi : pas un jour de semaine"
        );
        assert_eq!(r.due(&schedule, at(6, 23, 0)), vec![Action::Off]);
    }

    #[test]
    fn disabled_entries_and_skipped_minutes_are_ignored() {
        let mut e = entry(EVERY_DAY, 8, 0, Action::On);
        e.enabled = false;
        let schedule = Schedule { entries: vec![e] };
        let mut r = Runner::default();
        assert_eq!(r.due(&schedule, at(2, 8, 0)), vec![]);
        let schedule = Schedule {
            entries: vec![entry(EVERY_DAY, 8, 0, Action::On)],
        };
        let mut r = Runner::default();
        r.due(&schedule, at(2, 7, 59));
        assert_eq!(
            r.due(&schedule, at(2, 8, 5)),
            vec![],
            "minute 8:00 sautée, pas rattrapée"
        );
    }

    #[test]
    fn validation() {
        assert!(entry(0, 8, 0, Action::On).validate().is_err());
        assert!(entry(EVERY_DAY, 24, 0, Action::On).validate().is_err());
        assert!(entry(EVERY_DAY, 8, 60, Action::On).validate().is_err());
        let too_many = Schedule {
            entries: (0..9).map(|_| entry(EVERY_DAY, 8, 0, Action::On)).collect(),
        };
        assert!(too_many.validate().is_err());
    }

    #[test]
    fn json_shape_is_flat_and_stable() {
        let e = entry(WEEKEND, 9, 15, Action::Scene(2));
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"enabled":true,"days":96,"hour":9,"minute":15,"action":"scene","scene":2}"#
        );
        let off: Entry =
            serde_json::from_str(r#"{"days":127,"hour":23,"minute":0,"action":"off"}"#).unwrap();
        assert_eq!(off.action(), Action::Off);
        assert!(off.enabled);
        assert_eq!(
            serde_json::to_string(&off).unwrap(),
            r#"{"enabled":true,"days":127,"hour":23,"minute":0,"action":"off"}"#
        );
        assert!(entry(EVERY_DAY, 8, 0, Action::Scene(0)).validate().is_err());
    }

    #[test]
    fn sleep_timer_counts_down_and_fires_once() {
        let mut t = SleepTimer::default();
        assert_eq!(t.remaining_s(0), None);
        t.set(1_000, 2);
        assert_eq!(t.remaining_s(1_000), Some(120));
        assert_eq!(t.remaining_s(61_000), Some(60));
        assert!(!t.fire(120_999));
        assert!(t.fire(121_000));
        assert!(!t.fire(130_000));
        assert_eq!(t.remaining_s(130_000), None);
        t.set(0, 5);
        t.cancel();
        assert_eq!(t.remaining_s(0), None);
    }
}
