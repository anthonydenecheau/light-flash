//! Scènes et programmation : heure par SNTP et fuseau POSIX, minuterie d'extinction, horaires
//! hebdomadaires. Les documents (scènes, programme, fuseau) sont en NVS au format JSON. Pas de
//! thread dédié : `tick` est appelé chaque seconde par la boucle principale (pile déjà payée) et
//! applique les actions dues à l'état partagé, comme le ferait la page.

use crate::network::{NetMode, NetworkHandle};
use anyhow::{bail, Result};
use esp_idf_svc::{sntp::EspSntp, sys};
use http_server::{Automation, AutomationView, TimeView};
use light_core::{
    scenes::{Scene, SceneList},
    schedule::{Action, LocalTime, Runner, Schedule, SleepTimer},
    LightCommand, SharedState,
};
use log::{info, warn};
use std::{
    ffi::CString,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};
use storage::{SharedStorage, KEY_SCENES, KEY_SCHEDULE, KEY_TIMEZONE};

/// Fuseau par défaut (Europe de l'Ouest, heure d'été incluse), au format POSIX.
pub const DEFAULT_TZ: &str = "CET-1CEST,M3.5.0,M10.5.0/3";
const TZ_MAX: usize = 64;
/// En dessous (novembre 2023), l'horloge n'a pas été mise à l'heure.
const EPOCH_SYNCED: i64 = 1_700_000_000;

struct Inner {
    scenes: SceneList,
    schedule: Schedule,
    tz: String,
    timer: SleepTimer,
    runner: Runner,
}

#[derive(Clone)]
pub struct AutomationHandle {
    inner: Arc<Mutex<Inner>>,
    storage: SharedStorage,
    light: SharedState,
    boot: Instant,
}

pub fn start(
    storage: SharedStorage,
    light: SharedState,
    boot: Instant,
) -> Result<AutomationHandle> {
    let (scenes, schedule, tz) = {
        let st = storage::lock(&storage);
        let scenes = st
            .json(KEY_SCENES)?
            .and_then(|s| serde_json::from_str::<SceneList>(&s).ok())
            .unwrap_or_else(SceneList::defaults);
        let schedule = st
            .json(KEY_SCHEDULE)?
            .and_then(|s| serde_json::from_str::<Schedule>(&s).ok())
            .unwrap_or_default();
        let tz = st
            .json(KEY_TIMEZONE)?
            .unwrap_or_else(|| DEFAULT_TZ.to_owned());
        (scenes, schedule, tz)
    };
    info!(
        "scènes : {}, horaires : {}, fuseau : {tz}",
        scenes.scenes.len(),
        schedule.entries.len()
    );
    apply_timezone(&tz)?;
    Ok(AutomationHandle {
        inner: Arc::new(Mutex::new(Inner {
            scenes,
            schedule,
            tz,
            timer: SleepTimer::default(),
            runner: Runner::default(),
        })),
        storage,
        light,
        boot,
    })
}

impl AutomationHandle {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn now_ms(&self) -> u64 {
        self.boot.elapsed().as_millis() as u64
    }

    /// À appeler chaque seconde. Démarre SNTP dès que la station est connectée (le serveur est
    /// résolu par DNS) ; l'horloge se resynchronise ensuite toutes les heures. Puis applique la
    /// minuterie et les horaires dus.
    pub fn tick(&self, network: &NetworkHandle, sntp: &mut Option<EspSntp<'static>>) {
        if sntp.is_none() && network.status().mode == NetMode::Station {
            match EspSntp::new_default() {
                Ok(s) => {
                    info!("SNTP démarré");
                    *sntp = Some(s);
                }
                Err(e) => warn!("SNTP : {e}"),
            }
        }
        let now_ms = self.now_ms();
        let mut actions = Vec::new();
        {
            let inner = &mut *self.lock();
            if inner.timer.fire(now_ms) {
                info!("minuterie : extinction");
                actions.push(Action::Off);
            }
            if let Some(now) = local_time() {
                let due = inner.runner.due(&inner.schedule, now);
                if !due.is_empty() {
                    info!(
                        "programmation : {} action(s) à {:02}:{:02}",
                        due.len(),
                        now.hour,
                        now.minute
                    );
                }
                actions.extend(due);
            }
        }
        for action in actions {
            if let Err(e) = self.perform(action) {
                warn!("programmation : {e}");
            }
        }
    }

    fn perform(&self, action: Action) -> Result<()> {
        let commands = match action {
            Action::Off => vec![LightCommand::Off],
            Action::On => vec![LightCommand::On],
            Action::Scene(id) => match self.lock().scenes.get(id) {
                Some(scene) => scene.commands(),
                None => bail!("scène {id} introuvable"),
            },
        };
        let mut state = self.light.lock().unwrap_or_else(|p| p.into_inner());
        for cmd in commands {
            state.apply(cmd);
        }
        Ok(())
    }

    fn persist_scenes(&self, scenes: &SceneList) -> Result<()> {
        storage::lock(&self.storage).set_json(KEY_SCENES, &serde_json::to_string(scenes)?)
    }

    fn persist_schedule(&self, schedule: &Schedule) -> Result<()> {
        storage::lock(&self.storage).set_json(KEY_SCHEDULE, &serde_json::to_string(schedule)?)
    }
}

impl Automation for AutomationHandle {
    fn view(&self) -> AutomationView {
        let inner = self.lock();
        let now = local_time();
        AutomationView {
            scenes: inner.scenes.scenes.clone(),
            schedule: inner.schedule.entries.clone(),
            timer_s: inner.timer.remaining_s(self.now_ms()),
            time: TimeView {
                synced: now.is_some(),
                weekday: now.map(|t| t.weekday),
                hour: now.map(|t| t.hour),
                minute: now.map(|t| t.minute),
                tz: inner.tz.clone(),
            },
        }
    }

    fn save_scene(&self, scene: Scene) -> Result<u8> {
        let mut inner = self.lock();
        let id = inner.scenes.save(scene).map_err(anyhow::Error::msg)?;
        self.persist_scenes(&inner.scenes)?;
        info!("scène {id} enregistrée");
        Ok(id)
    }

    fn delete_scene(&self, id: u8) -> Result<()> {
        let mut inner = self.lock();
        if inner
            .schedule
            .entries
            .iter()
            .any(|e| e.action() == Action::Scene(id))
        {
            bail!("scène utilisée par un horaire : supprimer l'horaire d'abord");
        }
        if !inner.scenes.delete(id) {
            bail!("scène {id} introuvable");
        }
        self.persist_scenes(&inner.scenes)?;
        Ok(())
    }

    fn apply_scene(&self, id: u8) -> Result<()> {
        self.perform(Action::Scene(id))
    }

    fn set_schedule(&self, schedule: Schedule) -> Result<()> {
        schedule.validate().map_err(anyhow::Error::msg)?;
        let scenes_ok = {
            let inner = self.lock();
            schedule.entries.iter().all(|e| match e.action() {
                Action::Scene(id) => inner.scenes.get(id).is_some(),
                _ => true,
            })
        };
        if !scenes_ok {
            bail!("un horaire fait référence à une scène qui n'existe plus");
        }
        self.persist_schedule(&schedule)?;
        self.lock().schedule = schedule;
        Ok(())
    }

    fn set_timer(&self, minutes: u32) -> Result<()> {
        if minutes > 24 * 60 {
            bail!("minuterie limitée à 24 h");
        }
        let now_ms = self.now_ms();
        self.lock().timer.set(now_ms, minutes);
        info!("minuterie : {minutes} min");
        Ok(())
    }

    fn set_timezone(&self, tz: &str) -> Result<()> {
        let tz = tz.trim();
        if tz.is_empty() || tz.len() > TZ_MAX || !tz.bytes().all(|b| b.is_ascii_graphic()) {
            bail!("fuseau invalide : chaîne POSIX attendue, ex. CET-1CEST,M3.5.0,M10.5.0/3");
        }
        apply_timezone(tz)?;
        storage::lock(&self.storage).set_json(KEY_TIMEZONE, tz)?;
        self.lock().tz = tz.to_owned();
        info!("fuseau : {tz}");
        Ok(())
    }
}

/// Variable d'environnement TZ (format POSIX) prise en compte par `localtime_r`.
fn apply_timezone(tz: &str) -> Result<()> {
    let name = CString::new("TZ")?;
    let value = CString::new(tz)?;
    // SAFETY: deux chaînes C valides le temps de l'appel ; setenv copie la valeur.
    if unsafe { sys::setenv(name.as_ptr(), value.as_ptr(), 1) } != 0 {
        bail!("setenv(TZ) a échoué");
    }
    // SAFETY: relit TZ, sans argument.
    unsafe { sys::tzset() };
    Ok(())
}

/// Heure locale, ou `None` tant que l'horloge n'a pas été mise à l'heure par SNTP.
fn local_time() -> Option<LocalTime> {
    // SAFETY: time(NULL) rend l'epoch courant sans écrire nulle part.
    let now = unsafe { sys::time(std::ptr::null_mut()) };
    if (now as i64) < EPOCH_SYNCED {
        return None;
    }
    // SAFETY: `tm` est un POD ; localtime_r écrit dans notre tampon et rend NULL en cas d'échec.
    let mut tm: sys::tm = unsafe { std::mem::zeroed() };
    if unsafe { sys::localtime_r(&now, &mut tm) }.is_null() {
        return None;
    }
    Some(LocalTime {
        // tm_wday : 0 = dimanche ; le programme compte à partir de lundi.
        weekday: ((tm.tm_wday + 6) % 7) as u8,
        hour: tm.tm_hour as u8,
        minute: tm.tm_min as u8,
        minute_index: (now as i64 / 60) as u64,
    })
}
