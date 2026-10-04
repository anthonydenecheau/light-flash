# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Objet du projet

Firmware Rust (`std`, ESP-IDF) pour une **applique lumineuse** pilotée par une carte
**ESP32-C3-DevKit-RUST-1** (RISC-V `riscv32imc`, 4 Mo flash, LED WS2812 embarquée sur **GPIO2**,
USB-Serial-JTAG natif exposé en `/dev/ttyACM0`) et un ruban WS2812B de 144 LED.

Le projet est issu du template `esp-rs/std-training` (crates `hardware-check`, `rgb-led`, `wifi`
copiées puis adaptées). La langue de travail (commits, docs, échanges) est le **français**.
Les messages de commit suivent la convention gitmoji (`:sparkles:`, `:bug:`, `:memo:`, `:truck:`…)
et ne contiennent **aucune mention de Claude** : pas de `Co-Authored-By`, pas de `Claude-Session`.

## Matériel

- Carte ESP32-C3-DevKit-RUST-1 : LED WS2812 embarquée sur GPIO2 (RMT canal 0, sert de voyant
  d'état), bouton BOOT sur GPIO9, capteurs I2C sur GPIO8/GPIO10, USB sur GPIO18/19.
- Source lumineuse : ruban **WS2812B 5 V, 1 m, 144 LED, IP65**. Données sur **GPIO3** (RMT canal 1)
  via un adaptateur de niveau 74AHCT125 et une résistance de 330 Ω. Alimentation 5 V externe de
  8 à 10 A, masse commune avec la carte. Jusqu'à 8,6 A en blanc plein : le courant du ruban ne
  passe jamais par la breadboard ni par la broche 5V de la carte quand l'USB est branché.
- Aucun prototype construit au 2026-10-04. Guide de montage complet (contraintes, achats, câblage,
  sécurité, mise en route) dans `HARDWARE.md` ; tâches correspondantes dans `BACKLOG.md` §6.

## Organisation du dépôt

**Workspace Cargo unique à la racine** (depuis le 2026-10-04) : un seul `target/`, un seul build
d'ESP-IDF, versions communes dans `[workspace.dependencies]`, `Cargo.lock` commité.

| Chemin | Type | Rôle |
|---|---|---|
| `firmware/light-flash/` | binaire | Firmware principal : tâche lumière (`light_task.rs`, 50 images/s, seule à parler au driver), Wi-Fi station si identifiants (NVS puis `cfg.toml`) sinon point d'accès de secours `light-flash`, serveur HTTP de pilotage. Démarre allumée. |
| `firmware/hardware-check/` | binaire | Test de la carte : Wi-Fi STA + clignotement LED bleu/vert (rouge si échec Wi-Fi). |
| `crates/light-core/` | lib | Domaine **sans dépendance ESP** : `LightState`, `LightCommand`, `Effect`, `Renderer`, `Gamma`, `power::limit` (plafond de courant), `api::{LightView, LightPatch}` (JSON), `SharedState`. Testé sur l'hôte. |
| `crates/rgb-led/` | lib | Driver WS2812 via RMT (API `rmt-legacy`), N pixels (`set_pixels`), impulsions précalculées. |
| `crates/wifi/` | lib | Wi-Fi bloquant sur un driver réutilisable : `connect_sta` (15 s max) et `start_access_point`, plus le raccourci `wifi()` pour `hardware-check`. |
| `crates/storage/` | lib | NVS (espace `light`) : identifiants Wi-Fi. |
| `crates/http-server/` | lib | `GET /` page de pilotage (HTML embarqué, français), `GET/POST /api/light` (JSON `LightView` / `LightPatch`), `POST /connect` (identifiants Wi-Fi, callback fourni par le firmware). |

Fichiers racine : `Cargo.toml` (membres, versions, profils), `.cargo/config.toml` (cible, `ldproxy`,
runner `espflash`, `ESP_IDF_VERSION`), `rust-toolchain.toml`, `sdkconfig.defaults` (commun à tous
les binaires), `cfg.toml.example`, `Makefile`.

**Modèle de concurrence :** `SharedState = Arc<Mutex<LightState>>`. Les producteurs (handlers
HTTP, plus tard BLE et bouton) font `lock().apply(cmd)` ; la tâche lumière copie l'état à chaque
image, rend la trame, applique `power::limit` puis `set_pixels`. Le verrou n'est jamais tenu
pendant l'accès au driver. `LED_COUNT` et `MAX_MILLIAMPS` sont dans `light_task.rs` (1 LED et
500 mA tant que le ruban n'est pas câblé).

Branche distante `origin/feature/connect2Wifi` : son contenu utile (point d'accès, serveur HTTP)
a été revu, corrigé et intégré le 2026-10-04 dans `crates/wifi` et `crates/http-server` ; la
branche peut être supprimée. Le BLE de l'ancien `main.rs` a été retiré en attendant Improv Wi-Fi
(`BACKLOG.md` §2.4), qui reste le mode de provisioning principal prévu ; le portail HTTP en est
le repli.

## Chaîne de compilation

- Toolchain cible : `nightly-2025-01-01` + `rust-src` (fixé par `rust-toolchain.toml`). Pas besoin
  d'`espup` : la cible RISC-V est supportée par le nightly upstream. Les tests hôte passent par la
  toolchain **stable** (`cargo +stable`), qui ignore la section `[unstable] build-std` du
  `.cargo/config.toml`.
- Cible : `riscv32imc-esp-espidf`, `build-std = ["std", "panic_abort"]`, linker `ldproxy`.
- ESP-IDF **v5.3.2** compilé nativement par `esp-idf-sys` (`ESP_IDF_VERSION = tag:v5.3.2`),
  installé dans `~/.espressif` (`ESP_IDF_TOOLS_INSTALL_DIR = global`).
- Versions figées par `Cargo.lock` : `esp-idf-svc 0.51.0`, `esp-idf-hal 0.45.2` (feature
  `rmt-legacy`), `esp-idf-sys 0.36.1`, `esp32-nimble 0.10.2`, `embuild 0.33.5`.
- **`Cargo.lock` se régénère avec le nightly du projet** (`cargo update`), jamais avec
  `cargo +stable` : un lock résolu par un cargo récent tire des dépendances transitives qui exigent
  un rustc plus récent que nightly-2025-01-01. Garde-fous en place : `rust-version = "1.84"` dans
  le workspace et `resolver.incompatible-rust-versions = "fallback"` dans `.cargo/config.toml`.
  Exception connue : `ignore` est épinglé à 0.4.23 (0.4.30 utilise des let-chains sans déclarer
  sa `rust-version`) ; après un `cargo update`, vérifier qu'il n'est pas remonté.
- Un seul `sdkconfig.defaults` pour le workspace (esp-idf-sys n'est construit qu'une fois) ; il
  active NimBLE pour tous les binaires. Toute modification déclenche une recompilation complète
  d'ESP-IDF (long).
- `esp-idf-hal` et `esp-idf-svc` relaient l'environnement ESP-IDF via `links` : une crate n'a pas
  besoin de dépendre directement d'`esp-idf-sys`, mais chaque crate qui produit un binaire ou un
  exemple garde un `build.rs` appelant `embuild::espidf::sysenv::output()`.
- Outils installés par `make setup` ; `make doctor` vérifie l'installation et détecte la carte.
  La première compilation télécharge et compile ESP-IDF (plusieurs minutes, ~2 Go).

## Commandes : tout passe par le Makefile

**Convention du projet :** toute commande d'installation, compilation, test, flash ou moniteur
passe par le `Makefile` à la racine. Ne pas lancer ni documenter de `cargo` / `espflash` à la main ;
si une cible manque, l'ajouter au Makefile. `make help` liste les cibles.

```bash
make setup                                  # Ubuntu : paquets (sudo), toolchains, ldproxy, espflash, accès série
make doctor                                 # vérifie les outils et détecte la carte
make build   [CRATE=...] [RELEASE=1]        # cargo build -p CRATE
make build-all                              # tout le workspace, exemples compris
make check / make clippy / make fmt / make lint
make test                                   # tests hôte des crates sans dépendance ESP (light-core)
make run     [CRATE=...] [RELEASE=1] [PORT=/dev/ttyACM0]   # flash + moniteur (cargo run)
make flash / make monitor / make erase
make example CRATE=rgb-led EX=ws2812        # exemple d'une lib (EX=wifi nécessite cfg.toml)
make example CRATE=rgb-led EX=led_probe     # diagnostic LED : GPIO2 et GPIO8 en couleurs pleines
make monitor SECS=30                        # moniteur borné et non interactif (sessions sans terminal : agents, CI)
make example CRATE=rgb-led EX=ws2812 SECS=40   # idem pour run / example
make image   RELEASE=1                      # dist/light-flash-release.bin flashable seul
make clean
```

`CRATE` vaut `light-flash` par défaut ; valeurs possibles : `light-flash`, `hardware-check`,
`light-core`, `rgb-led`, `wifi`. Profils : debug `opt-level = "z"`, release `opt-level = "s"`.
`clippy` et `check` nécessitent qu'ESP-IDF ait déjà été construit une fois. Sans `cfg.toml` à la
racine, `build-all`, `check` et `clippy` excluent `hardware-check`.

Depuis une session sans terminal (Claude Code, CI), **toujours passer `SECS=<n>`** aux cibles
`run`, `example` et `monitor` : sans cela `espflash monitor` échoue (« Failed to initialize input
reader ») ou ne rend jamais la main. Si l'utilisateur vient d'être ajouté au groupe `dialout` sans
reconnexion, le Makefile passe automatiquement par `sg dialout -c`.

Les seuls tests automatisés sont ceux de `light-core` (`make test`). Les binaires ont
`harness = false` ; la validation du firmware se fait sur carte (`hardware-check`, puis `light-flash`).

## Identifiants Wi-Fi (`cfg.toml`)

`toml-cfg` lit `cfg.toml` **à la racine du workspace** (le parent de `target/`), dans la
**section portant le nom du package** : `[light-flash]`, `[hardware-check]`, `[wifi]` (exemple).
Modèle : `cfg.toml.example`. `cfg.toml` est ignoré par git. Pour `light-flash` les champs sont
facultatifs (identifiants de secours quand la NVS est vide, mot de passe du point d'accès) ; son
`build.rs` émet `rerun-if-changed` sur `cfg.toml`, donc sans ce fichier la crate se recompile à
chaque build (quelques secondes), c'est voulu pour détecter son apparition.
`firmware/hardware-check/build.rs` fait échouer le build si le fichier manque ou contient encore
les valeurs du modèle, et émet `rerun-if-changed` (pas de `cargo clean` nécessaire).

## Accès au port série

La machine de développement est sous **Ubuntu natif** (depuis octobre 2026). La carte apparaît en
`/dev/ttyACM0` avec le VID/PID Espressif `303a:1001` ; l'accès passe par le groupe `dialout`
(`make setup-serial`, puis se reconnecter). `make doctor` signale un port non accessible en écriture.
La liaison est l'USB-Serial-JTAG natif : le port se ré-énumère à chaque reset de la puce, ce
qu'espflash gère lui-même ; ne pas garder un `cat /dev/ttyACM0` ouvert pendant un flash.
Vérifié le 2026-10-04 : flash du firmware BLE et de l'exemple `ws2812` OK (puce rev v0.4, 4 Mo).

## Points d'attention

- `.devcontainer/Dockerfile` (ESP-IDF v4.4.4, nightly-2023-02-28) vient de `std-training` et ne
  correspond pas à la configuration réelle (v5.3.2, nightly-2025-01-01) : à réécrire
  (`BACKLOG.md` §5.5).
- Les handlers HTTP tournent dans la tâche httpd (pile 10 Ko) : y faire court, ne jamais y
  toucher au driver LED, ne jamais journaliser un mot de passe.
- `POST /connect` enregistre en NVS puis redémarre 2 s plus tard ; au boot suivant la lampe tente
  la station et retombe sur le point d'accès si la connexion échoue (15 s). La NVS survit aux
  flashs : `make erase` pour repartir sans identifiants. Le journal indique la source
  (« identifiants Wi-Fi : NVS » ou « cfg.toml »).
- Tester l'API depuis le PC : la lampe en station sur le réseau domestique (adresse dans le
  journal), puis `curl http://<ip>/api/light` ; en mode point d'accès le PC devrait quitter son
  propre Wi-Fi, préférer le téléphone.
- La LED embarquée n'a jamais été vue allumée par l'utilisateur (2026-10-04) alors que les trames
  partent : voir `BACKLOG.md` §6 et l'exemple `led_probe`.
