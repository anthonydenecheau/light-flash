fn main() {
    // `toml-cfg` lit cfg.toml à la racine du workspace ; recompiler quand il apparaît ou change.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let cfg = std::path::Path::new(&manifest_dir).join("../../cfg.toml");
    println!("cargo:rerun-if-changed={}", cfg.display());

    embuild::espidf::sysenv::output();
}
