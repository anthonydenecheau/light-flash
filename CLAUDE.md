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
- `MANUEL.md` est le manuel **utilisateur** (pas développeur) : français, sans commande ni jargon,
  à tenir à jour quand un comportement visible change (couleurs de signalisation, boutons,
  adresses, procédure de configuration).

## Organisation du dépôt

**Workspace Cargo unique à la racine** (depuis le 2026-10-04) : un seul `target/`, un seul build
d'ESP-IDF, versions communes dans `[workspace.dependencies]`, `Cargo.lock` commité.

| Chemin | Type | Rôle |
|---|---|---|
| `firmware/light-flash/` | binaire | Firmware principal : tâche lumière (`light_task.rs`, 50 images/s, seule à parler au driver, rend les indications système si présentes), thread de persistance (`persistence.rs`, NVS après 2 s de calme), thread réseau (`network.rs` : possède le driver Wi-Fi, reconnexion avec backoff, repli point d'accès `light-flash` après 90 s, nouvel essai station toutes les 2 min, commandes `Connect` du provisioning), thread de provisioning (`provisioning.rs` : possède NimBLE, service Improv, bouton BOOT, reset usine), mDNS, serveur HTTP démarré avant le réseau, mise à jour OTA (`update.rs`), scènes et programmation (`automation.rs` : SNTP, fuseau POSIX, minuterie, horaires ; `tick` appelé chaque seconde par la boucle principale, pas de thread). Démarre dans le dernier état enregistré, sinon allumée en blanc chaud. |
| `firmware/hardware-check/` | binaire | Test de la carte : Wi-Fi STA + clignotement LED bleu/vert (rouge si échec Wi-Fi). |
| `crates/light-core/` | lib | Domaine **sans dépendance ESP** : `LightState` (+ `to_bytes`/`from_bytes` versionnés, `Display`), `LightCommand`, `Effect`, `Renderer`, `Gamma`, `power::limit` (plafond de courant), `api::{LightView, LightPatch}` (JSON), `persist::SaveScheduler` (écriture différée), `reconnect::Policy` (reconnexion Wi-Fi), `status::Indication` (signalisation lumineuse), `button::ButtonTracker` (appui court/long), `scenes::{Scene, SceneList}` (8 scènes, JSON), `schedule::{Schedule, Entry, Runner, SleepTimer}` (horaires hebdomadaires évalués sur une `LocalTime` injectée, minuterie), `SharedState`, `SharedIndication`. Testé sur l'hôte. |
| `crates/improv/` | lib | Protocole Improv Wi-Fi BLE **sans dépendance ESP** : UUID, paquets RPC (checksum), `Machine` (autorisation, provisioning, résultat). Testé sur l'hôte. |
| `crates/rgb-led/` | lib | Driver WS2812 via RMT (API `rmt-legacy`), N pixels (`set_pixels`), impulsions précalculées. |
| `crates/wifi/` | lib | Wi-Fi bloquant sur un driver réutilisable : `connect_sta` (15 s max) et `start_access_point`, plus le raccourci `wifi()` pour `hardware-check`. |
| `crates/storage/` | lib | NVS (espace `light`) : identifiants Wi-Fi, dernier état de la lampe (`light_state`, 7 octets versionnés), nom de la lampe, serveur de mises à jour, documents JSON (`scenes`, `schedule`, `tz`, 3 900 octets max chacun via `json`/`set_json`) ; `factory_reset` ; `SharedStorage` + `storage::lock`. |
| `crates/http-server/` | lib | `GET /` page de pilotage (HTML + CSS + JS vanilla en français, logo en data URI, **gzip préparé par `build.rs`**, ~17 Ko servis), `/icon-192.png`, `/manifest.json` (écran d'accueil), `GET /api/status` (`StatusView`), `GET /api/peers` (`PeerView`, mode groupe), `GET/POST /api/light` (`LightView` / `LightPatch`), `POST /connect`, `POST /api/name`, `POST /api/system/{restart,forget-wifi,ble}` (`SystemHooks`), `GET /api/update` + `POST /api/update/{check,install,url}` (`UpdateHooks`), `GET /api/automation` + `POST /api/scenes`, `/api/scenes/{apply,delete}`, `/api/schedule`, `/api/timer`, `/api/time` (trait `Automation`), et avec la feature `debug-hooks` du firmware `DebugHooks` (`/api/debug/wifi-disconnect`, `improv-authorize`, `ble-off`). Tout est passé via `HttpContext` ; CORS `*` sur les réponses JSON ; corps JSON limités à 1 Ko. |

Fichiers racine : `Cargo.toml` (membres, versions, profils), `.cargo/config.toml` (cible, `ldproxy`,
runner `espflash`, `ESP_IDF_VERSION`), `rust-toolchain.toml`, `sdkconfig.defaults` (commun à tous
les binaires), `cfg.toml.example`, `Makefile`.

**Modèle de concurrence :** `SharedState = Arc<Mutex<LightState>>`. Les producteurs (handlers
HTTP, plus tard BLE et bouton) font `lock().apply(cmd)` ; la tâche lumière copie l'état à chaque
image, rend la trame, applique `power::limit` puis `set_pixels`. Le verrou n'est jamais tenu
pendant l'accès au driver. `LED_COUNT` et `MAX_MILLIAMPS` sont dans `light_task.rs` (1 LED et
500 mA tant que le ruban n'est pas câblé).

**Provisioning Wi-Fi** : Improv Wi-Fi sur BLE (spécification https://www.improv-wifi.com/ble/,
protocole dans `crates/improv`), implémenté le 2026-10-04.
La lampe s'annonce en BLE sous `light-flash` avec le service Improv ; un appui court sur BOOT
(GPIO9) autorise la réception des identifiants pendant 60 s ; la lampe se connecte, enregistre en
NVS et renvoie `http://light-flash.local/` et `http://<ip>/`. Clients : appli Home Assistant,
appli Improv, improv-wifi.com. Le portail HTTP du point d'accès de secours reste le repli.
Branche distante `origin/feature/connect2Wifi` : intégrée et obsolète, peut être supprimée.

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
  active NimBLE pour tous les binaires et fixe le nom DHCP (`CONFIG_LWIP_LOCAL_HOSTNAME`). Toute
  modification déclenche une recompilation d'ESP-IDF.
- Composants ESP-IDF supplémentaires (registre Espressif, ex. `espressif/mdns`) : déclarés dans
  `firmware/light-flash/Cargo.toml` sous `[[package.metadata.esp-idf-sys.extra_components]]`,
  que esp-idf-sys lit grâce à `ESP_IDF_SYS_ROOT_CRATE = "light-flash"` dans `.cargo/config.toml`
  (workspace virtuel). Même effet : recompilation d'ESP-IDF.
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
make example CRATE=rgb-led EX=led_probe_original   # contre-épreuve : driver std-training d'origine sur GPIO2
make monitor SECS=30                        # moniteur borné et non interactif (sessions sans terminal : agents, CI)
make example CRATE=rgb-led EX=ws2812 SECS=40   # idem pour run / example
make image   RELEASE=1                      # dist/light-flash-release.bin flashable seul (câble)
make publish NOTES="..."                    # image release + manifest.json dans dist/update/ (OTA)
make serve-update                           # sert dist/update/ sur le port 8000
make clean
```

`CRATE` vaut `light-flash` par défaut ; valeurs possibles : `light-flash`, `hardware-check`,
`light-core`, `rgb-led`, `wifi`. Profils : debug `opt-level = "z"`, release `opt-level = "s"`.
`clippy` et `check` nécessitent qu'ESP-IDF ait déjà été construit une fois. Sans `cfg.toml` à la
racine, `build-all`, `check` et `clippy` excluent `hardware-check`.

Depuis une session sans terminal (Claude Code, CI), **toujours passer `SECS=<n>`** aux cibles
`run`, `example` et `monitor` : sans cela `espflash monitor` échoue (« Failed to initialize input
reader ») ou ne rend jamais la main. `SECS` ne borne que le moniteur : avec `SECS`, `run` et
`example` enchaînent un flash non borné puis un moniteur borné. **Ne jamais mettre un `timeout`
autour d'un flash** : l'application fait 1,8 Mo et son écriture prend plus de 15 s ; interrompue,
la carte boucle sur « Factory app partition is not bootable » jusqu'au prochain `make flash`
(constaté le 2026-10-04). Si l'utilisateur vient d'être ajouté au groupe `dialout` sans
reconnexion, le Makefile passe automatiquement par `sg dialout -c`.

Les seuls tests automatisés sont ceux de `light-core` (`make test`). Les binaires ont
`harness = false` ; la validation du firmware se fait sur carte (`hardware-check`, puis `light-flash`).

## Identifiants Wi-Fi (`cfg.toml`)

`toml-cfg` lit `cfg.toml` **à la racine du workspace** (le parent de `target/`), dans la
**section portant le nom du package** : `[light-flash]`, `[hardware-check]`, `[wifi]` (exemple).
Modèle : `cfg.toml.example`. `cfg.toml` est ignoré par git. Pour `light-flash` les champs sont
facultatifs (identifiants de secours quand la NVS est vide, mot de passe du point d'accès) ; son
`build.rs` émet `rerun-if-changed` sur `cfg.toml`, donc sans ce fichier la crate se recompile à
chaque build (quelques secondes), c'est voulu pour détecter son apparition. Cargo compare les
dates de modification : après avoir restauré un `cfg.toml` avec une date ancienne (`cp -p`,
`git checkout`), faire `touch cfg.toml`, sinon le firmware garde l'ancienne configuration.
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
  flashs : `make erase` pour repartir sans identifiants ni état. Le journal indique la source
  (« identifiants Wi-Fi : NVS » ou « cfg.toml ») et l'état restauré.
- Persistance de l'état : toute écriture NVS passe par `SaveScheduler` (2 s de calme, seulement si
  l'état diffère de l'enregistré) ; ne jamais écrire en NVS depuis un handler HTTP ou la tâche
  lumière. Changer le format de `LightState::to_bytes` impose d'incrémenter `FORMAT_VERSION`
  (un ancien blob est alors ignoré, pas migré).
- **Nom de la lampe** : `light_core::naming`. Nom affiché = NVS, sinon `light-flash-xxxx` (deux
  derniers octets de la MAC station) ; nom d'hôte dérivé (`hostname_from`, minuscules ASCII,
  tirets) utilisé pour mDNS (`<hôte>.local`), le nom DHCP (`esp_netif_set_hostname` dans
  `wifi::new_wifi`), le SSID du point d'accès de secours et le nom BLE ; le nom affiché sert
  d'instance mDNS, de nom Improv et de titre de page. Figé au démarrage : `POST /api/name`
  enregistre puis redémarre. Les deux chaînes sont `Box::leak`ées en `&'static str` dans `main`.
  Vérifié le 2026-10-04 : « Salon 1 » → `salon-1.local` ; la Livebox résout aussi `<hôte>.home`.
- **Mode groupe** : `discovery.rs` (thread propriétaire de `EspMdns`) interroge `_http._tcp` à
  10 s, 30 s puis chaque minute et retient les services au TXT `light-flash=1` (hors soi-même) ;
  `GET /api/peers` renvoie le cache. La page envoie alors ses commandes à chaque lampe par son
  adresse IP (pas `.local`, Android ne le résout pas toujours) ; pour éviter le pré-vol CORS, les
  POST partent en `text/plain` et toutes les réponses JSON portent `Access-Control-Allow-Origin: *`.
  Test sans seconde carte : `scratchpad/fake_peer.py` (zeroconf + mini API, à relancer depuis le
  dépôt si besoin : annonce `_http._tcp` avec `light-flash=1` sur le PC) et un script Playwright
  qui clique dans la vraie page ; Playwright et zeroconf sont installés sur le PC.
- **Mise à jour par le réseau (OTA)** : `partitions.csv` (deux emplacements de 1,94 Mo, pas de
  partition `factory`), `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`, `make flash` écrit le
  bootloader produit par esp-idf-sys et la table (changer la table impose `make erase`).
  `make publish` produit l'image *application seule* en release et `manifest.json` (version, url,
  sha256, size, notes) dans `dist/update/` ; `make serve-update` les sert sur le port 8000.
  Côté lampe, `update.rs` : vérification à 25 s, toutes les 6 h et sur demande, GET HTTP/1.1
  minimal sur `TcpStream` (pas le client ESP-IDF, qui entraîne mbedTLS : 300 Ko), écriture en
  flux via `EspOta`, SHA-256 (`sha2`) et taille vérifiés, redémarrage ; `main` confirme l'image
  (`mark_running_slot_valid`) dès que le réseau est opérationnel. Les tailles d'image sont le
  point de vigilance : les deux profils sont en LTO complet, une unité de génération de code,
  `opt-level` z (dev) ou s (release) ; debug ≈ 1,73 Mo pour 2,03 Mo d'emplacement, release
  ≈ 1,65 Mo ; `CONFIG_COMPILER_OPTIMIZATION_SIZE`, pas de bundle de certificats, pas de WPA2
  Entreprise ni d'IPv6. Éviter `#[serde(flatten)]` (plusieurs dizaines de Ko de code sur la
  cible). Les points d'entrée de debug sont derrière la feature `debug-hooks`
  (`make run RELEASE=1 FEATURES=debug-hooks` pour tester en release). **Piège :** le câble écrit
  toujours `ota_0` ; après une mise à jour OTA (image active en `ota_1`), un `make flash` seul
  laisserait démarrer l'ancienne image, d'où l'effacement d'`otadata` dans la cible `flash`
  (`espflash erase-parts otadata`). Vérifié le 2026-10-04 : 0.2.0 → 0.2.1
  en 29 s, redémarrage sur `ota_1`, confirmation, « firmware à jour ».
- Page de pilotage : un seul fichier `crates/http-server/src/static/index.html`, placeholder
  `__LOGO__` remplacé par `build.rs` (data URI de `logo-160.jpg`) puis gzippé (≈ 17 Ko servis) ;
  `make build` suffit après modification. Cartes Lumière, Scènes, Programmation, Lampe ; Réseau
  et Mises à jour sont deux `<dialog>` (feuille en bas sur mobile, centrée au-delà de 600 px)
  ouverts par les boutons de l'en-tête. « Vérifier maintenant » attend la fin de la vérification
  (changement de `last_check_s`) pour annoncer le résultat, y compris « à jour ». Rendu vérifié avec Chrome sans tête :
  `google-chrome --headless=new --screenshot=... --window-size=420,1180 --virtual-time-budget=8000
  [--force-dark-mode] http://<nom>.local/` (ne pas utiliser `WebContentsForceDark`, qui inverse
  les couleurs au lieu d'honorer `prefers-color-scheme`). Source du logo : `assets/logo-1024.jpg`.
- BLE : seul le thread `provision` touche à NimBLE ; les callbacks `on_write` ne font que
  relayer les paquets sur un canal. Le GATT est créé une fois ; la pile est arrêtée
  (`BLEDevice::deinit`) 5 min après l'allumage, le dernier appui BOOT ou le dernier provisioning
  pour rendre sa mémoire, et relancée au prochain appui par `BLEDevice::init()` **puis**
  `take()` : `take()` seul ne réinitialise pas (initialisation paresseuse unique) et le premier
  `advertising.start()` plante alors dans `ble_gatts_reset` (constaté le 2026-10-04). esp32-nimble
  ré-enregistre les services au redémarrage, le GATT n'est créé qu'une fois. Le scan Wi-Fi
  d'Improv passe par le thread réseau ; le point d'accès de secours tourne en mode mixte pour que
  le scan reste possible. Hooks debug : `/api/debug/ble-off` coupe le BLE tout de suite,
  `/api/debug/improv-authorize` simule l'appui BOOT (et rallume le BLE). Tester sans téléphone :
  client Python `bleak` (script à verser dans le dépôt, `BACKLOG.md` §2.3), `rfkill unblock
  bluetooth` sur le PC puis `rfkill block` après, et `curl -X POST
  http://<ip>/api/debug/improv-authorize` à la place de l'appui sur BOOT.
- Réseau : seul le thread `network` touche au driver Wi-Fi. La logique de décision est dans
  `light_core::reconnect::Policy` (testée sur l'hôte) ; `network.rs` ne fait qu'exécuter les
  actions (`connect_sta`, `start_access_point`) et journaliser. Pour tester la reconnexion sans
  couper la box : `curl -X POST http://<ip>/api/debug/wifi-disconnect` (firmware compilé avec `FEATURES=debug-hooks`).
- Tester l'API depuis le PC : la lampe en station sur le réseau domestique (adresse dans le
  journal), puis `curl http://<ip>/api/light` ; en mode point d'accès le PC devrait quitter son
  propre Wi-Fi, préférer le téléphone.
- La première carte avait une LED embarquée défectueuse (remplacée le 2026-10-04 par une carte
  identique). Les exemples `led_probe` et `led_probe_original` de `rgb-led` servent de diagnostic
  si le doute revient.
- **Scènes et programmation** : `automation.rs` charge `scenes`/`schedule`/`tz` (JSON en NVS,
  valeurs par défaut sinon : 4 scènes, pas d'horaire, `CET-1CEST,M3.5.0,M10.5.0/3`), applique le
  fuseau par `setenv("TZ")` + `tzset`, démarre `EspSntp` dès que la station est connectée
  (`pool.ntp.org`, resynchronisation horaire) et, à chaque `tick` (1 s depuis `main`), lit
  `time()`/`localtime_r` (heure considérée reçue si epoch > 1,7 G), évalue `schedule::Runner`
  (une fois par minute, pas de rattrapage) et `SleepTimer`, puis applique les commandes à
  `SharedState`. Les scènes sont appliquées par la page via `POST /api/light` (donc aussi en mode
  groupe) ; `POST /api/scenes` sans couleur/luminosité/effet mémorise l'état courant. Supprimer
  une scène référencée par un horaire est refusé. Vérifié sur carte le 2026-10-04 : heure reçue
  14 s après le démarrage, UTC0 ↔ Paris, minuterie 1 min, horaire « scène 2 » à l'heure dite,
  scène conservée après reflash.
