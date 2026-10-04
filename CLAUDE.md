# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Objet du projet

Firmware Rust (`std`, ESP-IDF) pour une **applique lumineuse** pilotée par une carte
**ESP32-C3-DevKit-RUST-1** (RISC-V `riscv32imc`, 4 Mo flash, LED WS2812 embarquée sur **GPIO2**,
USB-Serial-JTAG natif exposé en `/dev/ttyACM0`).

Le projet est issu du template `esp-rs/std-training` (crates `hardware-check`, `rgb-led`, `wifi`
copiées puis adaptées). La langue de travail (commits, README, échanges) est le **français** ;
les messages de commit suivent la convention gitmoji (`:sparkles:`, `:bug:`, `:rocket:`…).

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

Il n'y a **pas de workspace Cargo à la racine** : chaque crate est autonome et possède ses propres
`.cargo/config.toml`, `rust-toolchain.toml`, `build.rs`, `sdkconfig.defaults` et son propre `target/`.
Toute commande `cargo` doit donc être lancée **depuis le répertoire de la crate concernée**.

| Crate | Type | Rôle |
|---|---|---|
| `light-flash/` | binaire | Firmware principal. Sur `master` : serveur BLE GATT (NimBLE via `esp32-nimble`) qui notifie un compteur, **sans pilotage de LED**. |
| `hardware-check/` | binaire | Test de la carte : connexion Wi-Fi STA + clignotement LED bleu/vert (rouge si échec Wi-Fi). |
| `common/lib/rgb-led/` | lib | Driver WS2812 via périphérique RMT (API `rmt-legacy` d'`esp-idf-hal`), un seul pixel. |
| `common/lib/wifi/` | lib | Helper bloquant de connexion Wi-Fi STA (scan → connect → attente DHCP). |

Branche distante `origin/feature/connect2Wifi` (non mergée) : ajoute `common/lib/wifi_ap/` (point
d'accès Wi-Fi) et `common/lib/http_server/` (page de saisie des identifiants Wi-Fi), et remplace le
`main.rs` BLE de `light-flash` par AP + serveur HTTP.

**Décision du 2026-10-04 :** le provisioning Wi-Fi se fait par **Improv Wi-Fi sur BLE** (protocole
ouvert, service GATT standard, clients existants : appli Home Assistant, improv-wifi.com), détaillé
dans `BACKLOG.md` §2.4. La branche SoftAP + portail n'est qu'un repli éventuel et ne doit pas être
mergée en l'état.

## Chaîne de compilation (identique dans toutes les crates)

- Toolchain : `nightly-2025-01-01` + composant `rust-src` (fixé par `rust-toolchain.toml`).
  Pas besoin d'`espup` : la cible RISC-V est supportée par le nightly upstream.
- Cible : `riscv32imc-esp-espidf`, `build-std = ["std", "panic_abort"]`, linker `ldproxy`.
- ESP-IDF **v5.3.2** compilé nativement par `esp-idf-sys` (`ESP_IDF_VERSION = tag:v5.3.2`),
  installé globalement dans `~/.espressif` (`ESP_IDF_TOOLS_INSTALL_DIR = global`).
- Versions épinglées : `esp-idf-sys 0.36`, `esp-idf-hal 0.45`, `embuild =0.33.0`.
  Attention : `light-flash` utilise `esp-idf-svc =0.51` alors que `hardware-check`, `rgb-led` et
  `wifi` sont sur `=0.50.1` (la branche feature les aligne sur 0.51).
- `sdkconfig.defaults` par crate. Celui de `light-flash` active NimBLE
  (`CONFIG_BT_ENABLED`, `CONFIG_BT_NIMBLE_ENABLED=y`, Bluedroid désactivé).
  Toute modification de `sdkconfig.defaults` déclenche une recompilation complète d'ESP-IDF (long).
- Pré-requis système et outils installés par `make setup` (paquets Ubuntu, nightly, `ldproxy`,
  `espflash`, `cargo-espflash`, accès série) ; `make doctor` vérifie l'installation et détecte la carte.
  La première compilation télécharge et compile ESP-IDF (plusieurs minutes, ~2 Go dans `~/.espressif`).

## Commandes : tout passe par le Makefile

**Convention du projet :** toute commande d'installation, compilation, test, flash ou moniteur
passe par le `Makefile` à la racine. Ne pas lancer ni documenter de `cargo` / `espflash` à la main ;
si une cible manque, l'ajouter au Makefile. `make help` liste les cibles.

```bash
make setup                                  # Ubuntu : paquets, nightly épinglé, ldproxy, espflash, accès série
make doctor                                 # vérifie les outils et détecte la carte
make build   [CRATE=...] [RELEASE=1]        # cargo build dans la crate
make check / make clippy / make fmt / make lint
make run     [CRATE=...] [RELEASE=1] [PORT=/dev/ttyACM0]   # flash + moniteur (cargo run)
make flash / make monitor / make erase
make example CRATE=rgb-led EX=ws2812        # exemple d'une lib (EX=wifi nécessite cfg.toml dans la crate)
make image   RELEASE=1                      # dist/light-flash-release.bin flashable seul
make test                                   # tests hôte (vide tant qu'aucune crate n'est testable)
make clean / make clean-all
```

`CRATE` vaut `light-flash` par défaut ; valeurs possibles : `light-flash`, `hardware-check`,
`rgb-led`, `wifi`. Tant qu'il n'y a pas de workspace, le Makefile fait `cd` dans la crate, et le
`rust-toolchain.toml` de chaque crate sélectionne le nightly. Profils : debug `opt-level = "z"`,
release `opt-level = "s"`. `clippy` nécessite qu'ESP-IDF ait déjà été construit une fois.

Il n'y a **pas de tests automatisés** : `harness = false` sur les binaires, aucun `#[test]`.
La validation se fait sur carte (`hardware-check`, puis `light-flash`).

## Identifiants Wi-Fi (`cfg.toml`)

`hardware-check` et l'exemple `wifi` lisent `wifi_ssid` / `wifi_psk` à la compilation via
`toml-cfg` depuis un fichier `cfg.toml` placé **dans le répertoire de la crate** (modèle :
`common/lib/wifi/cfg.toml.example`). `cfg.toml` est ignoré par git. `hardware-check/build.rs`
fait échouer le build si le fichier manque ou contient encore les valeurs du modèle.
Après modification de `cfg.toml`, un `cargo clean` peut être nécessaire pour que le changement soit pris en compte.

## Accès au port série

La machine de développement est sous **Ubuntu natif** (depuis octobre 2026 ; les instructions
`usbipd`/WSL2 et `chmod 777` du README datent de l'époque Windows et sont obsolètes). La carte
apparaît en `/dev/ttyACM0` avec le VID/PID Espressif `303a:1001` ; l'accès passe par le groupe
`dialout` (`sudo usermod -aG dialout $USER`, puis se reconnecter) ou une règle udev (voir
`BACKLOG.md` §5.1).

## Points d'attention

- `Cargo.lock` est ignoré par git (`**/Cargo.lock` dans `.gitignore`) : les builds ne sont pas
  reproductibles d'une machine à l'autre.
- `.devcontainer/Dockerfile` (ESP-IDF v4.4.4, nightly-2023-02-28) et `test.sh` viennent de
  `std-training` et ne correspondent pas à la configuration réelle du projet (v5.3.2, nightly-2025-01-01).
- `REFERENCES.md` pointe vers `esp-wifi` (écosystème `no_std`/`esp-hal`), qui n'est **pas** la pile
  utilisée ici (`std`/ESP-IDF).
- Dans `light-flash/src/main.rs`, le logger ESP n'est pas initialisé (`EspLogger::initialize_default()`
  absent) : utiliser `println!` ou l'initialiser avant d'utiliser `log::info!`.
- Le driver `rgb-led` n'est ni `Send` ni `Sync` de façon triviale et pilote un seul pixel ;
  pour le piloter depuis des callbacks BLE/HTTP, passer par un canal (`std::sync::mpsc`) vers une
  tâche dédiée plutôt que de partager le driver.
