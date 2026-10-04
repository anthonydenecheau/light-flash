# BACKLOG — light-flash

Préconisations issues de l'analyse de la codebase (branche `master` au commit `b3f2b42` et branche
`origin/feature/connect2Wifi`). Priorités : **P0** bloquant / à faire en premier, **P1** important,
**P2** souhaitable, **P3** idée.

État des lieux en une phrase : le dépôt contient quatre crates indépendantes copiées du template
`esp-rs/std-training`, un serveur BLE de démonstration qui ne pilote aucune LED, et une branche
non mergée qui explore un portail Wi-Fi de provisioning. Il n'y a pas encore de « firmware d'applique ».

---

## 1. Décisions à prendre (préalable à tout le reste)

- [ ] **P0 — Spécifier fonctionnellement l'applique.** Rien dans le code ne dit ce que la lampe doit
  faire. Fixé le 2026-10-04 : ruban WS2812B 144 LED comme source lumineuse (§6), LED embarquée
  conservée comme voyant d'état, provisioning Improv (§2.4). Reste à fixer : commandes (on/off,
  couleur, luminosité, effets), canal de pilotage (BLE, Wi-Fi, bouton BOOT GPIO9), comportement au
  démarrage (restaurer le dernier état ?), luminosité maximale autorisée (§6.1).
- [x] **Décidé le 2026-10-04 — Provisioning Wi-Fi par Improv Wi-Fi sur BLE** (détail en §2.4).
  `master` faisait du BLE, `feature/connect2Wifi` de l'AP Wi-Fi + HTTP ; faire tourner les deux
  radios en permanence sur un ESP32-C3 (≈400 Ko de SRAM) est possible mais serré. Le choix
  retenu réutilise la pile NimBLE déjà présente, stocke les identifiants en NVS, puis bascule en
  Wi-Fi STA pour l'API HTTP, mDNS et l'OTA. Le portail captif de la branche feature n'est
  conservé qu'en repli éventuel pour un téléphone sans BLE.
- [ ] **P1 — Confirmer le choix `std`/ESP-IDF** plutôt qu'une migration `no_std` (`esp-hal` +
  `esp-wifi` + `embassy`, ce vers quoi pointe `REFERENCES.md`). Recommandation : rester sur ESP-IDF
  tant que BLE + Wi-Fi + HTTP + NVS + OTA sont nécessaires ; la coexistence BLE/Wi-Fi et l'OTA y sont
  matures. Supprimer la ligne trompeuse de `REFERENCES.md`.

---

## 2. Architecture et organisation des répertoires

### 2.1 Passer à un workspace Cargo unique (P0) — fait le 2026-10-04

Réalisé : workspace racine (`Cargo.toml`, `.cargo/config.toml`, `rust-toolchain.toml`,
`sdkconfig.defaults` uniques), crates déplacées dans `firmware/` et `crates/`, versions alignées
(`esp-idf-svc 0.51.0`, `esp-idf-hal 0.45.2`, `esp-idf-sys 0.36.1`, `esp32-nimble 0.10.2`),
`Cargo.lock` commité, Makefile adapté (`-p CRATE`, `build-all`, `test` sur l'hôte).

- [x] **Build cible vérifié le 2026-10-04** : `make build-all` OK (ESP-IDF v5.3.2 compilé en
  ~3 min après téléchargement, ~3,7 Go dans `~/.espressif`), `light-flash` flashé, exemple
  `ws2812` flashé et journal de démarrage lu via `make example ... SECS=35`.
- [ ] **P2** — Les crates `ble`, `http-server`, `storage` de l'arborescence ci-dessous seront créées
  quand leur code existera ; `partitions.csv` et `.github/` relèvent du §5.

Arborescence cible :

```
light-flash/                      # racine = workspace
├── Cargo.toml                    # [workspace] members, [workspace.dependencies], [profile.*]
├── Cargo.lock                    # COMMITÉ (voir 2.3)
├── .cargo/config.toml            # unique : target, ldproxy, runner espflash, ESP_IDF_VERSION…
├── rust-toolchain.toml           # unique : nightly-2025-01-01 + rust-src
├── sdkconfig.defaults            # unique : union BT NimBLE + Wi-Fi + LWIP hostname
├── partitions.csv                # table de partitions OTA (voir 5.3)
├── CLAUDE.md  BACKLOG.md  README.md  LICENSE
├── firmware/
│   ├── light-flash/              # binaire principal (main.rs mince : câblage des tâches)
│   └── hardware-check/           # binaire de recette carte (conservé tel quel)
├── crates/
│   ├── light-core/               # DOMAINE, sans dépendance ESP : état de la lampe, commandes,
│   │                             #   effets, correction gamma. Testable sur l'hôte.
│   ├── rgb-led/                  # driver WS2812 (RMT), N pixels
│   ├── wifi/                     # STA + AP + reconnexion (fusion de wifi et wifi_ap)
│   ├── ble/                      # service GATT « light » (esp32-nimble)
│   ├── http-server/              # API HTTP + page de provisioning
│   └── storage/                  # NVS : identifiants Wi-Fi, dernier état lampe
├── .devcontainer/                # réécrit (voir 5.5)
└── .github/workflows/            # CI build + clippy (voir 5.6)
```

Notes de mise en œuvre :
- Les profils ne sont lus qu'à la racine ; les versions sont centralisées dans
  `[workspace.dependencies]` (caret, figées par `Cargo.lock`).
- Un seul `sdkconfig.defaults` pour tout le workspace : esp-idf-sys n'est construit qu'une fois,
  donc `hardware-check` embarque aussi la configuration NimBLE.
- `toml-cfg` lit `cfg.toml` **à la racine du workspace** (parent de `target/`), pas dans la crate.
- Chaque crate qui produit un binaire ou un exemple garde un `build.rs` avec
  `embuild::espidf::sysenv::output()` ; `esp-idf-hal` et `esp-idf-svc` relaient l'environnement
  ESP-IDF via `links`, une dépendance directe à `esp-idf-sys` n'est pas nécessaire.
- Les tests hôte utilisent `cargo +stable`, qui ignore `[unstable] build-std` : pas besoin d'un
  workspace séparé pour `light-core`.
- `Cargo.lock` doit être résolu par le nightly du projet. Un lock généré par cargo stable 1.97 a
  tiré `uuid 1.27`, `globset 0.4.20`, `home 0.5.12`, `ignore 0.4.33` (rustc ≥ 1.88). Correctifs :
  `rust-version = "1.84"` + `resolver.incompatible-rust-versions = "fallback"`, et `ignore`
  épinglé à 0.4.23 (sa `rust-version` n'est pas déclarée correctement).

### 2.2 Séparer domaine et périphériques (P1)

- [x] `light-core` créé le 2026-10-04 (`crates/light-core`) : `LightCommand`, `LightState`,
  `Effect` (Solid, Breathe, Rainbow), `Renderer` (horloge + rendu d'une trame), `Gamma`,
  `hsv_to_rgb`, et `power::{estimate_ma, limit}` pour le plafond de courant. Zéro dépendance ESP ;
  23 tests sur l'hôte via `make test`.
- [ ] **P1 — Brancher `light-core` dans `firmware/light-flash`** : tâche « lumière » propriétaire
  du driver, canal `mpsc`, boucle de rendu à 50 images/s, `power::limit` avant envoi au ruban,
  persistance différée de `LightState` en NVS.
- Modèle de concurrence : **une tâche « lumière » possède le driver WS2812** et consomme des
  `LightCommand` via `std::sync::mpsc` (ou `crossbeam-channel`). BLE, HTTP et bouton n'envoient que
  des commandes. Résout proprement le fait que `WS2812RMT` ne peut pas être partagé, et garde les
  callbacks NimBLE courts (ils tournent dans la tâche hôte NimBLE : jamais de transfert RMT bloquant
  dedans).
- Service GATT « light » proposé (UUID 128 bits custom) : `power` (u8, R/W/N), `color` (3 octets
  RGB, R/W/N), `brightness` (u8, R/W/N), `effect` (u8, R/W/N), `status` (N). Le provisioning Wi-Fi
  n'y figure pas : il passe par le service Improv standard (§2.4), exposé par le même serveur
  NimBLE. Nom d'advertising `light-flash`, pas « ESP32 Server ».

### 2.3 Hygiène du dépôt (P1)

- [x] `Cargo.lock` commité (2026-10-04), `**/Cargo.lock` retiré du `.gitignore`.
- [x] `.devcontainer/test.sh` supprimé (2026-10-04).
- [ ] **Choisir une licence** (décision à prendre par le propriétaire du projet) et ajouter
  `LICENSE`. L'attribution à `std-training` (MIT OR Apache-2.0) figure dans le README depuis le
  2026-10-04 et les champs `authors` d'origine sont conservés.
- [x] `README.md` et `REFERENCES.md` réécrits (2026-10-04) : organisation, démarrage via Makefile,
  liens réellement utiles.
- [x] `make lint` = `cargo fmt --check` + `cargo clippy -D warnings` sur tout le workspace ;
  configuration rustfmt/clippy par défaut, pas de fichier dédié nécessaire.
- [ ] **P1 — Mettre à jour les versions, après le premier build cible vérifié** (analyse du
  2026-10-04). L'ensemble actuel est identique à la référence `esp-rs/std-training` (connu bon) ;
  l'ensemble « dernier » est cohérent entre lui (`esp32-nimble 0.13` exige `esp-idf-svc 0.53.0`).
  La carte ESP32-C3 est supportée par toutes les versions ci-dessous, elle n'impose rien.

  | Composant | Actuel | Dernier (2026-10-04) | Remarques |
  |---|---|---|---|
  | `esp-idf-svc` | 0.51.0 | 0.53.0 (2026-09-25) | MSRV 1.82 ; ESP-IDF 5.3 à 6.0 ; nouveaux variants `WifiEvent` à traiter ; API NVS modifiée ; wrappers NimBLE natifs (GAP/GATT) ; corrige l'advertising BLE sur C3 avec ESP-IDF ≥ 5.5 |
  | `esp-idf-hal` | 0.45.2 | 0.47.0 | va avec svc 0.53 |
  | `esp-idf-sys` | 0.36.1 | 0.38.1 (2026-09-16) | ESP-IDF < 5.3 déprécié ; 5.4/5.5 supportés, 6.0 « compatibilité de base » |
  | `esp32-nimble` | 0.10.2 | 0.13.0 | exige svc 0.53.0 |
  | `embuild` | 0.33.5 | 0.33.5 | à jour |
  | `toml-cfg` | 0.1.3 | 0.2.0 | mineur |
  | `rgb` | 0.8.53 | 0.8.53 | 0.8.92-rc est une pré-version, ne pas la prendre |
  | ESP-IDF | v5.3.2 | v5.5.x (6.0 trop récent) | recompilation complète (15–30 min) |
  | nightly | 2025-01-01 | nightly récent, **daté** | aucune date imposée par les crates (MSRV 1.82 satisfait) ; choisir la date du jour de la mise à jour et la figer |
  | `espflash` | 4.6.0 | 4.6.0 | options du Makefile vérifiées (`--chip`, `save-image --merge`, `monitor --non-interactive`) |

  Constat du 2026-10-04 : le nightly de janvier 2025 oblige déjà à brider des dépendances
  transitives (résolveur MSRV, `ignore` épinglé) ; chaque `cargo update` futur risque d'en
  réveiller d'autres. Argument supplémentaire pour faire la mise à jour tôt.

  Ordre : 1) `make build-all` + `hardware-check` sur carte avec l'ensemble actuel pour valider
  l'environnement ; 2) un commit dédié qui passe crates, ESP-IDF v5.5 et nightly ensemble, puis
  corrige les ruptures d'API (`WifiEvent`, NVS) ; 3) re-valider sur carte. Ne pas mélanger cette
  mise à jour avec une fonctionnalité.
- [ ] Ne pas merger `feature/connect2Wifi` en l'état (décision §1) ; en extraire uniquement
  l'alignement sur `esp-idf-svc 0.51`, puis garder la branche comme référence pour un éventuel
  repli SoftAP. Ne pas laisser deux `main.rs` incompatibles vivre en parallèle.

### 2.4 Provisioning Wi-Fi : Improv Wi-Fi sur BLE (décision du 2026-10-04)

Protocole ouvert publié par les auteurs d'ESPHome (spécification : https://www.improv-wifi.com/ble/).
Retenu parce qu'il réutilise la pile NimBLE déjà présente sur `master`, qu'il prévoit nativement
une autorisation par bouton physique, et qu'il est déjà supporté côté client : appli Home Assistant
(iOS et Android), appli Improv Wi-Fi (Android), page improv-wifi.com (Web Bluetooth, Chrome Android
et desktop). **Aucune appli mobile à écrire.** La spécification en ligne fait foi pour les valeurs
ci-dessous.

**Service GATT Improv** (UUID de service `00467768-6228-2272-4663-277478268000`)

| Caractéristique | UUID (suffixe) | Propriétés | Contenu |
|---|---|---|---|
| Capabilities | `…8005` | READ | bit 0 = commande « Identify » supportée |
| Current State | `…8001` | READ, NOTIFY | `0x02` autorisation requise, `0x03` autorisé, `0x04` connexion en cours, `0x05` provisionné |
| Error State | `…8002` | READ, NOTIFY | `0x00` aucun, `0x01` paquet RPC invalide, `0x02` commande inconnue, `0x03` connexion impossible, `0x04` non autorisé, `0xFF` inconnu |
| RPC Command | `…8003` | WRITE | `[cmd][len][data…][checksum]`, checksum = somme des octets précédents modulo 256 |
| RPC Result | `…8004` | READ, NOTIFY | `[cmd][len][chaînes préfixées par leur longueur]` ; pour `0x01`, une URL à ouvrir (peut être vide) |

L'advertising porte le nom de l'appareil, l'UUID du service, et un *service data* sur l'UUID 16 bits
`0x4677` contenant `[état courant][capabilities][4 octets réservés]`, ce qui permet aux applis
d'afficher l'état avant de se connecter.

Commandes RPC : `0x01` Send Wi-Fi settings, data = `[len ssid][ssid][len psk][psk]` ;
`0x02` Identify, sans data (faire clignoter la lampe pour la reconnaître parmi plusieurs).

**Séquence cible**

1. Boot sans identifiants en NVS → advertising `light-flash`, état `0x02`, LED bleue clignotante.
2. Appui court sur BOOT (GPIO9) → état `0x03`, LED bleue fixe ; retour à `0x02` après 60 s sans
   commande. Toute commande `0x01` reçue en état `0x02` est refusée avec Error State `0x04`.
3. Réception `0x01` → vérification du checksum et des longueurs (SSID ≤ 32, PSK ≤ 64) → état
   `0x04` → tentative STA avec timeout de 15 s.
4. Succès : écriture SSID/PSK en NVS, état `0x05`, RPC Result avec `http://light.local/` (ou l'IP),
   LED verte 2 s puis retour à l'état lampe. Échec : Error State `0x03`, retour à `0x03`, LED rouge
   2 s puis bleue fixe ; rien n'est écrit en NVS.
5. Après provisioning, l'advertising Improv s'arrête ; le service « light » (§2.2) reste disponible
   en BLE pour le pilotage.
6. Re-provisioning : appui long 5 s sur BOOT efface la NVS et redémarre ; N échecs consécutifs de
   reconnexion STA (ex. 10) rouvrent aussi la fenêtre Improv sans effacer les identifiants.

**Implémentation** (crate `crates/improv`)

- [ ] **P0** — Vérifier sur crates.io s'il existe une implémentation Rust réutilisable du protocole ;
  sinon l'écrire : le parseur de paquets et la machine à états tiennent en quelques centaines de
  lignes, **sans dépendance ESP**, donc testables sur l'hôte (§2.2).
- [ ] **P0** — Côté BLE avec `esp32-nimble` : un service, cinq caractéristiques, `on_write` sur
  RPC Command. Le callback ne fait que parser et pousser une `ProvisionCommand` sur un canal
  `mpsc` ; la tentative de connexion (plusieurs secondes) tourne dans une tâche dédiée qui met à
  jour Current State / Error State / RPC Result et notifie. Jamais de Wi-Fi bloquant dans un
  callback NimBLE.
- [ ] **P0** — Côté Wi-Fi : réutiliser `crates/wifi` avec `Some(nvs)` et distinguer dans l'erreur
  retournée réseau introuvable / authentification refusée / pas de bail DHCP, pour le log et pour
  choisir entre réessayer ou abandonner.
- [ ] **P1** — Bouton BOOT : `PinDriver::input(gpio9)` avec pull-up, interruption ou polling
  50 ms, anti-rebond, distinction appui court / appui long.
- [ ] **P1** — `sdkconfig.defaults` : `CONFIG_BT_NIMBLE_NVS_PERSIST=y` (mémoriser les appairages),
  `CONFIG_ESP_COEX_SW_COEXIST_ENABLE=y` (BLE et Wi-Fi actifs simultanément pendant l'étape 3).
- [ ] **P1** — Sécurité : mot de passe jamais loggé ni renvoyé ; appairage BLE LE Secure
  Connections avec bonding (Improv ne chiffre pas au niveau applicatif, c'est l'appairage BLE qui
  protège le canal) ; fenêtre d'autorisation courte ; advertising Improv coupé une fois provisionné.
- [ ] **P2** — Après succès, démarrer mDNS (`light.local`, composant `espressif/mdns`, voir §3) pour
  que l'URL renvoyée dans RPC Result soit utilisable.

**Tests**

- Hôte : parseur (checksum faux, longueurs incohérentes, paquet tronqué, SSID vide) et machine à
  états (commande hors fenêtre d'autorisation, timeout, double provisioning).
- Carte, avec l'appli Home Assistant : scénario nominal, mauvais mot de passe, SSID inexistant,
  timeout d'autorisation, Identify, reset usine par appui long, reboot avec identifiants en NVS.

---

## 3. Corrections de bugs

### `firmware/light-flash/src/main.rs` (BLE)

- [ ] **P0 — Aucun pilotage de LED.** Le firmware « applique » ne fait que notifier un compteur.
  La caractéristique est `READ | NOTIFY` : impossible d'écrire dessus, donc impossible de piloter
  quoi que ce soit. Ajouter `WRITE` + `on_write`.
- [ ] **P1 — Logger non initialisé.** `log` est en dépendance mais `EspLogger::initialize_default()`
  n'est jamais appelé ; le code utilise `println!`. Initialiser le logger et basculer sur `log::info!`.
- [ ] **P1 — `unwrap()` dans le callback `on_connect`** (`update_conn_params(...).unwrap()`) :
  un échec de négociation fait paniquer la tâche NimBLE → reboot. Logger l'erreur et continuer.
- [ ] **P2 — `main()` retourne `()`** : passer à `anyhow::Result<()>` comme les autres crates.
- [x] **Dépendances inutiles** (fait le 2026-10-04) : `heapless` et la dépendance directe à
  `esp-idf-sys` retirées, `link_patches` appelé via `esp_idf_svc::sys`, section `[features]` vide
  supprimée.
- [ ] **P2 — Sécurité BLE** : aucune authentification ; n'importe qui à portée peut se connecter.
  Au minimum un appairage « Just Works » avec bonding (`ble_device.security().set_auth(...)`) et
  `CONFIG_BT_NIMBLE_NVS_PERSIST=y` pour mémoriser les appairages.

### `crates/wifi/src/lib.rs`

- [ ] **P1 — `expect()` sur la conversion SSID / mot de passe** : un SSID > 32 caractères ou un mot
  de passe > 64 caractères fait paniquer au lieu de retourner une erreur. Remplacer par
  `.map_err(...)?`.
- [ ] **P1 — Aucune reconnexion.** Après une coupure de l'AP, la lampe reste déconnectée jusqu'au
  reboot. S'abonner à `WifiEvent::StaDisconnected` sur l'`EspSystemEventLoop` et relancer
  `connect()` avec backoff.
- [ ] **P2 — `EspWifi::new(modem, sysloop, None)`** : passer la partition NVS (`Some(nvs)`) pour que
  l'IDF conserve la calibration RF et les derniers paramètres (démarrage Wi-Fi plus rapide).
- [ ] **P3 — Méthode d'auth devinée** (`WPA2Personal` si mot de passe non vide) : WPA3-only non géré.
  Utiliser `AuthMethod::WPA2WPA3Personal` ou la valeur renvoyée par le scan.

### `crates/rgb-led/src/lib.rs`

- [ ] **P0 — Un seul pixel.** Le ruban cible compte 144 LED (§6), soit 3456 paires d'impulsions
  par trame : ajouter `set_pixels(&[RGB8])` avec `VariableLengthSignal` (le driver RMT envoie les
  items par morceaux au-delà de sa mémoire interne), ou adopter la crate `ws2812-esp32-rmt-driver`
  + trait `smart-leds`, qui donne accès aux effets de l'écosystème `smart-leds`. Prévoir deux
  instances : canal 0 pour la LED de statut, canal 1 pour le ruban.
- [ ] **P2 — Pas de temps de reset** (> 50 µs à l'état bas) après la trame : deux `set_pixel`
  rapprochés peuvent être interprétés comme une seule trame. Ajouter une pulse basse finale ou
  un délai.
- [ ] **P2 — `esp-idf-svc` et `log` en `[dependencies]`** alors que seuls les exemples les
  utilisent : passer en `[dev-dependencies]`.
- [ ] **P3 — Feature `rmt-legacy`** : l'API RMT legacy est dépréciée dans ESP-IDF 5.x ; prévoir la
  migration vers le nouveau driver RMT quand `esp-idf-hal` l'exposera pleinement.

### `firmware/hardware-check/`

- [x] **Build qui échoue sur un clone frais** (fait le 2026-10-04) : `cfg.toml.example` est à la
  racine du workspace, là où `toml-cfg` lit `cfg.toml` ; `build.rs` vérifie ce chemin avec un
  message explicite.
- [x] **Modification de `cfg.toml` non détectée** (fait le 2026-10-04) : `build.rs` émet
  `cargo:rerun-if-changed` sur `cfg.toml`, plus besoin de `cargo clean`.

### Branche `feature/connect2Wifi`

Points à traiter **seulement si** le repli SoftAP est finalement conservé (§1, §2.4) ; sinon la
branche peut être archivée telle quelle.

- [ ] **P0 — Fuite des identifiants** : le handler `/connect` logge SSID et mot de passe en clair
  (`info!("Credentials: …")`) **et les renvoie dans la réponse HTTP**. Supprimer les deux.
- [ ] **P0 — Provisioning incomplet** : les identifiants reçus ne sont ni stockés en NVS ni utilisés
  pour basculer en STA. Le portail ne sert à rien en l'état.
- [ ] **P1 — Mot de passe AP codé en dur** (`password123`) dans `wifi_ap/src/lib.rs`.
  Générer un mot de passe par carte (dérivé de l'adresse MAC) ou le lire depuis `cfg.toml`/NVS.
- [ ] **P1 — Code mort dans `main.rs`** : `server_thread.join()` sur un thread qui `park()` en
  boucle ne retourne jamais ; la boucle « Still running » qui suit est inatteignable. Garder
  `server` vivant dans `main` et boucler avec `sleep`.
- [ ] **P2 — `RUST_BACKTRACE=1` dans `http_server/sdkconfig.defaults`** : ce n'est pas une clé
  sdkconfig, la ligne est ignorée.
- [ ] **P2 — Hostname « not fixed »** (commits `eb569ad`, `208f530`) : `CONFIG_LWIP_LOCAL_HOSTNAME`
  n'affecte que le nom DHCP. Pour `light.local` il faut mDNS : ajouter le composant
  `espressif/mdns` via `[package.metadata.esp-idf-sys.extra_components]` (il n'est plus livré avec
  ESP-IDF 5.x) et utiliser `esp_idf_svc::mdns::EspMdns`.
- [ ] **P2 — `embedded-svc` en dépendance directe** : passer par les ré-exports
  `esp_idf_svc::http` / `esp_idf_svc::io` pour ne pas épingler une version séparée.

---

## 4. Optimisations

### 4.1 Temps de compilation

- [x] **Workspace** (§2.1, fait le 2026-10-04) : passe de 4 compilations ESP-IDF à 1.
- [ ] **P1 — Limiter les modifications de `sdkconfig.defaults`** : chaque changement reconstruit
  ESP-IDF. Regrouper les changements et les faire tôt.
- [ ] **P2 — CI** : mettre en cache `~/.espressif` et `target/` (voir 5.6).

### 4.2 Taille du binaire et flash

- [ ] **P1 — `CONFIG_COMPILER_OPTIMIZATION_SIZE=y`** : par défaut ESP-IDF compile son code C en
  `-Og` ; `-Os` réduit sensiblement la partie IDF du binaire.
- [ ] **P1 — Désactiver les composants IDF inutiles** dans `sdkconfig.defaults` :
  `CONFIG_MBEDTLS_CERTIFICATE_BUNDLE=n` tant qu'il n'y a pas de TLS sortant (≈ 60–80 Ko),
  `CONFIG_BT_NIMBLE_ROLE_CENTRAL=n`, `CONFIG_BT_NIMBLE_ROLE_OBSERVER=n` (la lampe est périphérique
  seulement), `CONFIG_LOG_DEFAULT_LEVEL_WARN=y` en release.
- [ ] **P2 — Profil release Rust** : ajouter `lto = "fat"`, `codegen-units = 1`, `strip = true`
  (n'affecte que la partie Rust ; `panic = "abort"` est déjà imposé par `build-std`).
- [ ] **P2 — Vérifier la taille** avec `espflash save-image` + `ls -l` ou `cargo espflash
  save-image --chip esp32c3` et consigner une valeur de référence par version.

### 4.3 Exécution

- [ ] **P1 — Démarrage Wi-Fi** : le `scan()` complet avant `connect()` coûte 2–3 s à chaque boot.
  Le rendre optionnel (ne scanner que si la connexion directe échoue) ou mémoriser le canal en NVS.
- [ ] **P1 — Driver LED** : calculer les quatre `Pulse` une fois dans `new()` plutôt qu'à chaque
  `set_pixel` ; remplacer `2_u32.pow(i)` par `1u32 << i` ; ne pas réinterroger `counter_clock()`.
- [ ] **P1 — Coexistence BLE / Wi-Fi** : si les deux radios tournent, activer la coexistence
  logicielle (`CONFIG_ESP_COEX_SW_COEXIST_ENABLE=y`, vérifier le nom exact dans `menuconfig` pour
  la v5.3.2) et surveiller le heap libre (`esp_idf_svc::sys::esp_get_free_heap_size()`) dans un log
  périodique en debug.
- [ ] **P2 — Usure de la flash** : ne pas écrire l'état en NVS à chaque commande BLE ; différer
  l'écriture (ex. 2 s après la dernière modification).
- [ ] **P2 — Rendu couleur** : appliquer une correction gamma (table 256 entrées) avant envoi aux
  WS2812 pour une luminosité perçue linéaire.
- [ ] **P3 — Économie d'énergie** : `wifi.set_ps(...)` (modem sleep) si la latence BLE/HTTP reste
  acceptable ; sans intérêt tant que l'applique est alimentée par USB.
- [ ] **P3 — Tick FreeRTOS** : `CONFIG_FREERTOS_HZ=1000` (commenté dans `sdkconfig.defaults`) si
  des effets ont besoin de `sleep` < 10 ms.

---

## 5. Déploiement et flash

### 5.1 Poste de développement (état constaté : aucune toolchain ESP installée sur cette machine)

Convention adoptée le 2026-10-04 : **toute commande d'installation, compilation, test, flash ou
moniteur passe par le `Makefile`** à la racine (`make help`). Ne pas documenter de `cargo` /
`espflash` à la main ailleurs ; ajouter une cible quand il en manque une.

- [x] **`make setup`** installe tout en une fois : paquets Ubuntu, nightly épinglé (lu dans
  `rust-toolchain.toml`), `ldproxy`, `espflash`, `cargo-espflash`, groupe `dialout` et règle udev
  sur le VID/PID `303a:1001`. `espup` n'est **pas** nécessaire (RISC-V uniquement).
  `make doctor` vérifie l'installation et détecte la carte.
- [x] **`make setup` terminé le 2026-10-04** : paquets, nightly (rustfmt, clippy), stable,
  `ldproxy`, `espflash 4.6.0`, `cargo-espflash 4.6.0`, groupe `dialout` + règle udev. Reste la
  reconnexion de session pour que `dialout` soit effectif (le Makefile contourne via `sg`).
- [x] Makefile adapté au workspace (2026-10-04) : `-p CRATE`, `build-all`, `make test` sur les
  crates hôte, PATH de `~/.cargo/bin` exporté pour les shells non interactifs.
- [ ] **P2 — README** : la machine de développement est désormais sous **Ubuntu natif** (plus de
  Windows/WSL2). Retirer les instructions `usbipd` et `chmod 777` du README, ou les reléguer dans
  une annexe « historique WSL2 ». Sous Ubuntu la carte apparaît directement en `/dev/ttyACM0` ;
  seuls le groupe `dialout` ou la règle udev ci-dessus sont nécessaires.

### 5.2 Commandes de flash

Cibles disponibles : `make run` (flash + moniteur), `make flash`, `make monitor`, `make erase`,
`make image`, avec `RELEASE=1`, `PORT=/dev/ttyACM0` et `CRATE=...` en option.

- [ ] **P1 — Runner enrichi** dans `.cargo/config.toml` :
  `runner = "espflash flash --monitor --chip esp32c3 --partition-table partitions.csv"`
  (le `--chip` évite l'auto-détection, la table de partitions prépare l'OTA). Ajouter
  `--partition-table` aussi dans les cibles `flash` et `image` du Makefile.
- [x] **Image distribuable** : `make image RELEASE=1` produit `dist/light-flash-release.bin`
  (bootloader + partitions + application fusionnés), flashable avec `espflash write-bin 0x0 <bin>`
  ou esp-web-flash sans toolchain Rust sur la machine cible. À brancher en CI (§5.6).
- [x] **Effacement complet** avant un changement de table de partitions : `make erase`.
- [x] **Moniteur seul** : `make monitor` (`--elf` passé pour symboliser les backtraces) ;
  `SECS=<n>` borne la durée et passe en mode non interactif (obligatoire sans terminal).
- [ ] **P2 — Bootloader cohérent** : espflash 4.6 flashe son propre bootloader de 2e étage
  (ESP-IDF v6.1-beta, visible dans le journal de démarrage) alors que l'application est bâtie
  sur v5.3.2. Ça fonctionne, mais pour un firmware distribué, passer `--bootloader` avec celui
  produit par esp-idf-sys (`target/.../esp-idf-sys-*/out/build/bootloader/bootloader.bin`) dans
  les cibles `flash` et `image`.

### 5.3 Table de partitions et OTA

- [ ] **P1 — `partitions.csv` avec deux slots OTA** (flash 4 Mo). Par défaut `espflash` utilise une
  seule partition `factory` : impossible de mettre à jour sans câble.
  ```csv
  # Name,    Type, SubType,  Offset,   Size
  nvs,       data, nvs,      0x9000,   0x6000
  otadata,   data, ota,      0xf000,   0x2000
  phy_init,  data, phy,      0x11000,  0x1000
  ota_0,     app,  ota_0,    0x20000,  0x180000
  ota_1,     app,  ota_1,    0x1A0000, 0x180000
  storage,   data, spiffs,   0x320000, 0xE0000
  ```
- [ ] **P2 — OTA par HTTP** avec `esp_idf_svc::ota::EspOta` : la lampe télécharge l'image depuis
  une URL fournie (serveur local pendant le dev), écrit dans le slot inactif, redémarre, confirme
  (`mark_running_slot_valid`) après un boot sain, sinon rollback automatique
  (`CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`).
- [ ] **P3 — Signature des images** (`CONFIG_SECURE_SIGNED_APPS_NO_SECURE_BOOT=y`) une fois l'OTA
  en place.

### 5.4 Provisioning et persistance

- [ ] **P0 — Stockage NVS** (`EspNvs::new(nvs_partition, "light", true)`) pour SSID/PSK et le
  dernier état de la lampe. Remplace `cfg.toml` compilé en dur pour le firmware final
  (`cfg.toml` reste pratique pour `hardware-check`).
- [ ] **P1 — Séquence de boot** : lire NVS → si identifiants présents, STA + reconnexion
  automatique ; sinon mode provisioning Improv (§2.4) signalé par une LED bleue clignotante.
- [ ] **P2 — Reset usine** : appui long 5 s sur BOOT (GPIO9) efface la NVS et redémarre (§2.4,
  étape 6).

### 5.5 Devcontainer

- [ ] **P1 — Réécrire `.devcontainer/Dockerfile`** pour correspondre au projet : base Debian
  bookworm, `nightly-2025-01-01`, `ldproxy`, `espflash`, pré-clone d'ESP-IDF **v5.3.2** dans
  `~/.espressif` (le Dockerfile actuel installe v4.4.4 et nightly-2023-02-28, jamais utilisés
  puisque `esp-idf-sys` retélécharge v5.3.2). Épingler le tag de l'image dans `devcontainer.json`
  au lieu de `latest`.
- [ ] **P2 — Simulation Wokwi** (`wokwi.toml` + `diagram.json` avec ESP32-C3 + WS2812) pour tester
  la logique LED et le Wi-Fi sans carte ; l'extension est déjà listée dans `devcontainer.json`.

### 5.6 Intégration continue

- [ ] **P1 — GitHub Actions** : job Ubuntu qui n'appelle que des cibles du Makefile
  (`make setup-system setup-rust`, `make lint`, `make build RELEASE=1`, `make image RELEASE=1`),
  met en cache `~/.espressif` + `~/.cargo` + `target/`, et publie `dist/*.bin` en artefact.
  Le premier run dure 15–30 min (build ESP-IDF), les suivants quelques minutes grâce au cache.
- [ ] **P2 — Release** : sur tag `vX.Y.Z`, attacher l'image mergée et `partitions.csv` à la release
  GitHub ; versionner `light-flash` via `env!("CARGO_PKG_VERSION")` exposé dans la caractéristique
  BLE `status` et sur `/api/status`.

---

## 6. Matériel : prototype du ruban LED

Guide de montage complet (contraintes électriques, liste d'achats, repérage du ruban, câblage,
règles de sécurité, affectation des broches, mise en route) dans **`HARDWARE.md`**.
Matériel disponible au 2026-10-04 : ruban WS2812B 5 V, 1 m, 144 LED/m, IP65 ; carte
ESP32-C3-DevKit-RUST-1 ; breadboard et fils Dupont. Aucun prototype construit. Broches retenues :
voyant GPIO2 (RMT canal 0), ruban GPIO3 (RMT canal 1), bouton BOOT GPIO9.

- [ ] **P0 — Achats** : alimentation 5 V 8 à 10 A avec jack, adaptateur jack vers bornier,
  74AHCT125N, résistance 330 Ω, condensateur 1000 µF, fil 18 AWG, multimètre
  (liste détaillée dans `HARDWARE.md` §3).

### 6.1 Plan de mise en route

- [ ] **P0 — Étape 0, sans ruban** : étendre `rgb-led` à N pixels (§3) et valider sur la LED
  embarquée avec N = 1.
- [ ] **P0 — Étape 1, câblage hors tension** : monter selon `HARDWARE.md` §5 ; au multimètre, vérifier l'absence de
  court-circuit entre 5 V et GND et la continuité des masses ; mettre sous tension sans la carte
  et mesurer 5 V aux bornes du ruban.
- [ ] **P0 — Étape 2, premier allumage** : exemple `strip_test` dans `rgb-led` : tout éteint, puis
  les 8 premières LED en rouge faible (10, 0, 0). Si elles s'allument, la chaîne données est bonne.
- [ ] **P0 — Étape 3, 144 LED** : chenillard d'une LED sur toute la longueur (vérifie que les 144
  répondent et le sens), puis arc-en-ciel à 25 %.
- [ ] **P1 — Étape 4, puissance** : rampe de blanc par paliers de 10 % en touchant régulièrement
  le ruban et l'alimentation ; noter le palier au-delà duquel le bout du ruban jaunit (chute de
  tension) et décider de l'injection en bout.
- [ ] **P1 — Plafond de courant dans le firmware** : estimer le courant d'une trame (par LED :
  (r + g + b) / 765 × 60 mA, plus 1 mA de veille) et réduire la luminosité globale pour rester sous
  un budget configurable (ex. 5 A), comme `setMaxPowerInVoltsAndMilliamps` de FastLED. C'est la
  protection principale de l'alimentation et du ruban ; à placer dans `light-core`, donc testable
  sur l'hôte.
- [ ] **P2 — Montage final** : perfboard ou petite PCB avec borniers, fusible 10 A, profilé
  aluminium, boîtier pour l'alimentation ; carte alimentée par sa broche 5V, USB débranché.

---

## 7. Idées (P3)

- Bouton BOOT (GPIO9) comme interrupteur physique / cycle d'effets.
- Capteurs embarqués de la DevKit-RUST-1 (IMU ICM-42670-P et SHTC3 sur I2C, SDA GPIO10 / SCL GPIO8) :
  « tap » pour allumer, inclinaison pour varier l'intensité, couleur fonction de la température.
- Intégration domotique : Home Assistant via MQTT (`esp_idf_svc::mqtt`) ou ESPHome-like API.
