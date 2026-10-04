//! Prépare la page à la compilation : logo incorporé en data URI, puis version gzip, pour
//! servir ~7 Ko au lieu de ~25 Ko depuis la flash, sans travail à l'exécution.

use base64::Engine;
use std::{env, fs, io::Write, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=src/static/index.html");
    println!("cargo:rerun-if-changed=src/static/logo-160.jpg");
    println!("cargo:rerun-if-changed=src/static/icon-192.png");

    let out = env::var("OUT_DIR").expect("OUT_DIR");
    let out = Path::new(&out);
    let html = fs::read_to_string("src/static/index.html").expect("index.html");
    let logo = fs::read("src/static/logo-160.jpg").expect("logo-160.jpg");
    let logo_uri = format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&logo)
    );
    let html = html.replace("__LOGO__", &logo_uri);
    fs::write(out.join("index.html"), &html).expect("écriture index.html");

    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(html.as_bytes()).expect("gzip");
    let gz = encoder.finish().expect("gzip");
    fs::write(out.join("index.html.gz"), &gz).expect("écriture index.html.gz");

    embuild::espidf::sysenv::output();
}
