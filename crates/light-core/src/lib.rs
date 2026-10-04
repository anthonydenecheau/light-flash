//! Domaine de l'applique lumineuse, indépendant du matériel.
//!
//! Ce crate ne dépend d'aucune bibliothèque ESP : il se compile et se teste sur
//! l'hôte (`make test`) comme sur la cible. Le firmware l'utilise ainsi :
//!
//! 1. les canaux de pilotage (BLE, HTTP, bouton) produisent des [`LightCommand`] ;
//! 2. une tâche « lumière » possède le [`LightState`], lui applique les commandes,
//!    appelle [`Renderer::render`] à chaque image puis [`power::limit`] ;
//! 3. la trame obtenue est envoyée au driver WS2812.

pub mod color;
pub mod power;
pub mod render;
pub mod state;

pub use color::Gamma;
pub use render::Renderer;
pub use rgb::RGB8;
pub use state::{Effect, LightCommand, LightState};
