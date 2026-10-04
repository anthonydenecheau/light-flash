//! Scènes : réglages mémorisés (couleur, luminosité, effet) rappelables d'un geste.

use crate::state::{Effect, LightCommand, LightState};
use serde::{Deserialize, Serialize};

pub const SCENES_MAX: usize = 8;
pub const SCENE_NAME_MAX: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scene {
    pub id: u8,
    pub name: String,
    /// `#rrggbb`.
    pub color: String,
    pub brightness: u8,
    /// `solid`, `breathe` ou `rainbow`.
    pub effect: String,
}

impl Scene {
    /// Commandes qui appliquent la scène (et allument la lampe).
    pub fn commands(&self) -> Vec<LightCommand> {
        let mut cmds = Vec::with_capacity(4);
        if let Some(e) = Effect::from_name(&self.effect) {
            cmds.push(LightCommand::SetEffect(e));
        }
        if let Some(c) = crate::api::parse_color(&self.color) {
            cmds.push(LightCommand::SetColor(c));
        }
        cmds.push(LightCommand::SetBrightness(self.brightness.max(1)));
        cmds.push(LightCommand::On);
        cmds
    }

    pub fn from_state(id: u8, name: &str, state: &LightState) -> Self {
        Self {
            id,
            name: name.to_owned(),
            color: crate::api::format_color(state.color),
            brightness: state.brightness,
            effect: state.effect.name().to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneList {
    pub scenes: Vec<Scene>,
}

impl SceneList {
    /// Scènes de départ d'une lampe neuve.
    pub fn defaults() -> Self {
        let mk = |id, name: &str, color: &str, brightness, effect: &str| Scene {
            id,
            name: name.into(),
            color: color.into(),
            brightness,
            effect: effect.into(),
        };
        Self {
            scenes: vec![
                mk(1, "Lecture", "#ffd9a8", 220, "solid"),
                mk(2, "Soirée", "#ff8c1a", 90, "solid"),
                mk(3, "Veilleuse", "#ff5a1f", 25, "breathe"),
                mk(4, "Fête", "#2f80ff", 200, "rainbow"),
            ],
        }
    }

    pub fn get(&self, id: u8) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.id == id)
    }

    /// Ajoute ou remplace (même `id`, ou même nom si `id` = 0). Renvoie l'id attribué.
    pub fn save(&mut self, mut scene: Scene) -> Result<u8, &'static str> {
        let name = scene.name.trim().to_owned();
        if name.is_empty() {
            return Err("le nom de la scène est vide");
        }
        if name.chars().count() > SCENE_NAME_MAX {
            return Err("nom de scène trop long (24 caractères maximum)");
        }
        if crate::api::parse_color(&scene.color).is_none() {
            return Err("couleur invalide");
        }
        if Effect::from_name(&scene.effect).is_none() {
            return Err("effet inconnu");
        }
        scene.name = name.clone();
        if scene.id == 0 {
            if let Some(existing) = self
                .scenes
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(&name))
            {
                scene.id = existing.id;
            } else {
                scene.id = (1..=SCENES_MAX as u8)
                    .find(|id| self.get(*id).is_none())
                    .ok_or("8 scènes maximum : en supprimer une d'abord")?;
            }
        }
        match self.scenes.iter_mut().find(|s| s.id == scene.id) {
            Some(slot) => *slot = scene.clone(),
            None => {
                if self.scenes.len() >= SCENES_MAX {
                    return Err("8 scènes maximum : en supprimer une d'abord");
                }
                self.scenes.push(scene.clone());
            }
        }
        self.scenes.sort_by_key(|s| s.id);
        Ok(scene.id)
    }

    pub fn delete(&mut self, id: u8) -> bool {
        let before = self.scenes.len();
        self.scenes.retain(|s| s.id != id);
        self.scenes.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgb::RGB8;

    #[test]
    fn defaults_are_valid_and_apply() {
        let list = SceneList::defaults();
        assert_eq!(list.scenes.len(), 4);
        let mut state = LightState::default();
        for cmd in list.get(3).unwrap().commands() {
            state.apply(cmd);
        }
        assert!(state.power);
        assert_eq!(state.brightness, 25);
        assert_eq!(state.effect, Effect::Breathe);
        assert_eq!(state.color, RGB8::new(0xff, 0x5a, 0x1f));
    }

    #[test]
    fn save_assigns_ids_replaces_by_name_and_caps_at_eight() {
        let mut list = SceneList::default();
        let mk = |name: &str| Scene {
            id: 0,
            name: name.into(),
            color: "#ffffff".into(),
            brightness: 100,
            effect: "solid".into(),
        };
        assert_eq!(list.save(mk("A")), Ok(1));
        assert_eq!(list.save(mk("B")), Ok(2));
        assert_eq!(list.save(mk(" a ")), Ok(1), "même nom = remplacement");
        assert_eq!(list.scenes.len(), 2);
        for n in 3..=8 {
            assert_eq!(list.save(mk(&format!("S{n}"))), Ok(n));
        }
        assert!(list.save(mk("trop")).is_err());
        assert!(list.delete(5));
        assert!(!list.delete(5));
        assert_eq!(list.save(mk("trou")), Ok(5), "l'id libéré est réutilisé");
    }

    #[test]
    fn save_validates_fields() {
        let mut list = SceneList::default();
        let bad = |name: &str, color: &str, effect: &str| Scene {
            id: 0,
            name: name.into(),
            color: color.into(),
            brightness: 1,
            effect: effect.into(),
        };
        assert!(list.save(bad("", "#000000", "solid")).is_err());
        assert!(list.save(bad("x", "rouge", "solid")).is_err());
        assert!(list.save(bad("x", "#000000", "disco")).is_err());
        assert!(list.save(bad(&"n".repeat(25), "#000000", "solid")).is_err());
    }

    #[test]
    fn from_state_round_trips() {
        let mut s = LightState::default();
        s.apply(LightCommand::SetColor(RGB8::new(1, 2, 3)));
        s.apply(LightCommand::SetBrightness(77));
        let scene = Scene::from_state(1, "Test", &s);
        let mut back = LightState::default();
        for cmd in scene.commands() {
            back.apply(cmd);
        }
        assert_eq!(back, s);
        let json = serde_json::to_string(&scene).unwrap();
        assert_eq!(serde_json::from_str::<Scene>(&json).unwrap(), scene);
    }
}
