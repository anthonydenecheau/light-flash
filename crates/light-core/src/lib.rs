//! Domaine de l'applique lumineuse, indépendant du matériel.
//!
//! Ce crate ne dépend d'aucune bibliothèque ESP : il se compile et se teste sur
//! l'hôte (`make test`) comme sur la cible. Le firmware l'utilise ainsi :
//!
//! 1. les canaux de pilotage (BLE, HTTP, bouton) produisent des [`LightCommand`] ;
//! 2. ces commandes sont appliquées au [`LightState`] partagé ([`SharedState`]) ; une tâche
//!    « lumière », seule propriétaire du driver, en prend une copie à chaque image, appelle
//!    [`Renderer::render`] puis [`power::limit`] ;
//! 3. la trame obtenue est envoyée au driver WS2812 ;
//! 4. un thread de persistance observe le même état et l'enregistre en NVS après 2 s de calme
//!    ([`SaveScheduler`]) ; il est restauré au démarrage.

pub mod api;
pub mod color;
pub mod persist;
pub mod power;
pub mod reconnect;
pub mod render;
pub mod state;

pub use api::{LightPatch, LightView};
pub use color::Gamma;
pub use persist::SaveScheduler;
pub use render::Renderer;
pub use rgb::RGB8;
pub use state::{Effect, LightCommand, LightState};

/// État partagé entre les producteurs de commandes (HTTP, BLE, bouton) et la tâche lumière.
/// Les producteurs appellent `lock().apply(cmd)` ; la tâche copie l'état à chaque image.
pub type SharedState = std::sync::Arc<std::sync::Mutex<LightState>>;
