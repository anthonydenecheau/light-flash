#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

fn main() {
    // `toml-cfg` lit `cfg.toml` à la racine du workspace (le parent de `target/`),
    // pas dans le répertoire de cette crate.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let cfg = std::path::Path::new(&manifest_dir).join("../../cfg.toml");
    // Recompiler quand les identifiants changent, sans `cargo clean`.
    println!("cargo:rerun-if-changed={}", cfg.display());
    if !cfg.exists() {
        panic!(
            "Créer `cfg.toml` à la racine du workspace avec vos identifiants Wi-Fi \
             (modèle : `cfg.toml.example`)."
        );
    }

    // La constante `CONFIG` est générée par `toml_config`.
    let app_config = CONFIG;
    if app_config.wifi_ssid == "FBI Surveillance Van" || app_config.wifi_psk == "hunter2" {
        panic!("Renseigner wifi_ssid / wifi_psk dans `cfg.toml` : les valeurs du modèle sont encore présentes.");
    }

    embuild::espidf::sysenv::output();
}
