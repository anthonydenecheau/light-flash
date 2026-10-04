# BACKLOG — light-flash

Préconisations issues de l'analyse initiale (branche `master` au commit `b3f2b42`, branche
`origin/feature/connect2Wifi`), tenues à jour au fil des réalisations. Priorités : **P0** bloquant /
à faire en premier, **P1** important, **P2** souhaitable, **P3** idée. Les items cochés gardent leur
texte comme trace des décisions.

**État au 2026-10-04 (relecture complète).** Fait et vérifié sur carte : workspace Cargo unique,
tâche lumière, API et page HTTP, persistance NVS, reconnexion Wi-Fi avec repli point d'accès,
mDNS et nom DHCP, provisioning Improv sur BLE, manuel utilisateur (chapitre 1). Prochaines
priorités, dans l'ordre : (1) matériel du ruban (§6, achats puis mise en route), (2) mémoire avec
BLE actif (§4.3), (3) mise à jour des versions (§2.3), (4) partitions OTA et CI (§5.3, §5.6),
(5) devcontainer (§5.5). Deux questions en attente du propriétaire : batterie ou non (§1) et
licence (§2.3).

---

## 1. Décisions à prendre (préalable à tout le reste)

- [ ] **P1 — Spécifier fonctionnellement l'applique.** Fixé le 2026-10-04 : ruban WS2812B 144 LED
  comme source lumineuse (§6) ; commandes marche/arrêt, couleur, luminosité, trois effets (uni,
  respiration, arc-en-ciel) ; pilotage par la page et l'API HTTP ; provisioning Improv (§2.4) ;
  BOOT réservé à l'autorisation Improv et au reset usine ; la lampe restaure son dernier état au
  démarrage (§2.2 pour l'option « toujours allumer »). Reste à fixer : luminosité maximale et
  budget de courant avec le ruban (§6.1), effets supplémentaires souhaités, pilotage BLE direct
  ou non (§2.2).
- [x] **Décidé le 2026-10-04 — Provisioning Wi-Fi par Improv Wi-Fi sur BLE** (détail en §2.4).
  `master` faisait du BLE, `feature/connect2Wifi` de l'AP Wi-Fi + HTTP ; faire tourner les deux
  radios en permanence sur un ESP32-C3 (≈400 Ko de SRAM) est possible mais serré. Le choix
  retenu réutilise la pile NimBLE déjà présente, stocke les identifiants en NVS, puis bascule en
  Wi-Fi STA pour l'API HTTP, mDNS et l'OTA. Le portail captif de la branche feature n'est
  conservé qu'en repli éventuel pour un téléphone sans BLE.
- [ ] **P1 — Alimentation sur batterie ?** (question du 2026-10-04). Le montage décrit dans
  `HARDWARE.md` est sur secteur et la carte n'a ni connecteur batterie ni jauge : un indicateur
  de charge exige d'abord un choix matériel. Si batterie : préciser type et capacité (un ruban de
  144 LED à 30 % tire ≈ 2,6 A sous 5 V, soit 13 W), puis mesure soit par diviseur de tension sur
  une entrée ADC (GPIO0 ou GPIO1, précision ±10 % sur le pourcentage), soit par jauge I2C
  MAX17048 sur le bus existant (GPIO8/GPIO10, adresse 0x36, pourcentage ±2 % et vitesse de
  décharge, donc temps restant fiable, recommandé). Côté logiciel : `GET /api/power`
  (`source`, `percent`, `voltage`, `current_ma`, `remaining_min`), jauge et temps restant dans la
  page, masqués sans batterie. Sans batterie : afficher la consommation estimée du ruban, déjà
  calculée à chaque trame par `power::estimate_ma`.
- [x] **Choix `std`/ESP-IDF confirmé** (2026-10-04) : BLE, Wi-Fi, HTTP, mDNS et NVS tournent
  ensemble sur ESP-IDF ; la ligne trompeuse vers `esp-wifi` a disparu de `REFERENCES.md`. Une
  migration `no_std` (`esp-hal`, `embassy`) n'est pas à l'ordre du jour.

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
- [x] Crates `http-server`, `storage` et `improv` créées (2026-10-04). Le BLE n'est pas une crate
  séparée : le protocole est dans `crates/improv` (sans ESP) et la glu NimBLE dans
  `firmware/light-flash/src/provisioning.rs`. Restent `partitions.csv`, `LICENSE`, `.github/`
  (§5, §2.3).

Arborescence réelle au 2026-10-04 (en italique : à venir) :

```
light-flash/                      # racine = workspace
├── Cargo.toml                    # [workspace] members, [workspace.dependencies], [profile.*]
├── Cargo.lock                    # commité
├── .cargo/config.toml            # target, ldproxy, runner espflash, ESP_IDF_VERSION, résolveur MSRV
├── rust-toolchain.toml           # nightly-2025-01-01 + rust-src
├── sdkconfig.defaults            # NimBLE périphérique, nom DHCP, tampons Wi-Fi, piles
├── cfg.toml.example              # identifiants de secours et mot de passe du point d'accès
├── Makefile                      # point d'entrée unique
├── CLAUDE.md  BACKLOG.md  README.md  HARDWARE.md  MANUEL.md  REFERENCES.md   (LICENSE à venir)
├── firmware/
│   ├── light-flash/              # main.rs (câblage), light_task, persistence, network, provisioning
│   └── hardware-check/           # recette carte (Wi-Fi + LED), conservé tel quel
├── crates/
│   ├── light-core/               # domaine sans ESP : état, commandes, effets, gamma, puissance,
│   │                             #   API JSON, persistance différée, reconnexion, indications, bouton
│   ├── improv/                   # protocole Improv Wi-Fi BLE sans ESP
│   ├── rgb-led/                  # driver WS2812 (RMT), N pixels, exemples de diagnostic
│   ├── wifi/                     # station et point d'accès sur un driver partagé
│   ├── http-server/              # page de pilotage, API JSON, formulaire Wi-Fi, hooks debug
│   └── storage/                  # NVS : identifiants Wi-Fi, état de la lampe, reset usine
├── .devcontainer/                # obsolète, à réécrire (§5.5)
└── .github/workflows/            # à venir (§5.6)
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
  51 tests sur l'hôte via `make test` au 2026-10-04 (avec API, persistance, reconnexion,
  indications, bouton).
- [x] **`light-core` branché dans `firmware/light-flash`** (2026-10-04) : tâche lumière
  (`light_task.rs`) propriétaire du driver, 50 images/s, `power::limit` avant envoi, écriture sur
  les LED seulement quand la trame change. Modèle retenu : `SharedState = Arc<Mutex<LightState>>`
  plutôt qu'un canal `mpsc`, car les producteurs ont aussi besoin de lire l'état (réponse JSON) ;
  le verrou n'est jamais tenu pendant l'accès au driver, les callbacks restent courts.
- [x] **Persistance différée de `LightState` en NVS** (2026-10-04) : `SaveScheduler` dans
  `light-core` (2 s de calme, écriture seulement si différent, retentative après échec, 6 tests),
  blob versionné de 7 octets, thread `persistence` dans `light-flash`, restauration au démarrage.
  Vérifié sur carte : état restauré après reset, une rafale de 20 commandes = 1 écriture.
- [ ] **P3 — Comportement après coupure secteur** : aujourd'hui la lampe revient dans son dernier
  état, donc éteinte si elle l'était. Si l'applique est commandée par un interrupteur mural,
  prévoir une option « toujours allumer à la mise sous tension » (réglable depuis la page).
- [ ] **P2 — `LED_COUNT` et `MAX_MILLIAMPS`** (1 LED, 500 mA) à passer à 144 et au budget de
  l'alimentation quand le ruban sera câblé (§6).
- [ ] **P2 — Pilotage BLE direct** : service GATT « light » (UUID 128 bits custom) : `power`
  (u8, R/W/N), `color` (3 octets RGB, R/W/N), `brightness` (u8, R/W/N), `effect` (u8, R/W/N),
  `status` (N), exposé par le même serveur NimBLE que le service Improv (§2.4). Utile seulement
  pour piloter sans Wi-Fi ; aujourd'hui tout passe par HTTP. À décider avec la spécification (§1).

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
- [x] `feature/connect2Wifi` revue et intégrée dans `master` (2026-10-04, §3) ; la branche
  distante peut être supprimée (`git push origin --delete feature/connect2Wifi`).

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
| Current State | `…8001` | READ, NOTIFY | `0x01` autorisation requise, `0x02` autorisé, `0x03` connexion en cours, `0x04` provisionné (valeurs vérifiées sur la spec le 2026-10-04) |
| Error State | `…8002` | READ, NOTIFY | `0x00` aucun, `0x01` paquet RPC invalide, `0x02` commande inconnue, `0x03` connexion impossible, `0x04` non autorisé, `0x05` nom d'hôte invalide, `0xFF` inconnu |
| RPC Command | `…8003` | WRITE | `[cmd][len][data…][checksum]`, checksum = somme des octets précédents modulo 256 |
| RPC Result | `…8004` | READ, NOTIFY | `[cmd][len][chaînes préfixées par leur longueur][checksum]` ; pour `0x01`, une URL à ouvrir (peut être vide) |

L'advertising porte le nom de l'appareil, l'UUID du service, et un *service data* sur l'UUID 16 bits
`0x4677` contenant `[état courant][capabilities][4 octets réservés]`, ce qui permet aux applis
d'afficher l'état avant de se connecter.

Commandes RPC : `0x01` Send Wi-Fi settings, data = `[len ssid][ssid][len psk][psk]` ;
`0x02` Identify, sans data (faire clignoter la lampe pour la reconnaître parmi plusieurs) ;
`0x03` Device info (réponse : firmware, version, matériel, nom) ; `0x04` Scan Wi-Fi (réponse :
triplets SSID, RSSI, sécurité) ; `0x05`/`0x06` nom d'hôte / nom d'appareil. Capabilities :
bit 0 identify, bit 1 device info, bit 2 scan, bit 3 hostname, bit 4 device name.

**Séquence cible**

1. Boot sans identifiants en NVS → advertising `light-flash`, état `0x01`, LED bleue clignotante.
2. Appui court sur BOOT (GPIO9) → état `0x02`, LED bleue fixe ; retour à `0x01` après 60 s sans
   commande. Toute commande `0x01` reçue en état `0x01` est refusée avec Error State `0x04`.
3. Réception `0x01` → vérification du checksum et des longueurs (SSID ≤ 32, PSK ≤ 64) → état
   `0x03` → tentative STA avec timeout de 15 s.
4. Succès : écriture SSID/PSK en NVS, état `0x04`, RPC Result avec `http://light-flash.local/`
   et l'adresse IP, LED verte 2 s puis retour à l'état lampe. Échec : Error State `0x03`, retour à
   `0x02`, LED rouge 2 s puis bleue fixe ; rien n'est écrit en NVS.
5. Après provisioning, l'advertising Improv s'arrête ; le service « light » (§2.2) reste disponible
   en BLE pour le pilotage. *Non fait : l'advertising reste actif (voir P2 ci-dessous), pas de
   service de pilotage BLE.*
6. Re-provisioning : appui long 5 s sur BOOT efface la NVS et redémarre (*fait*) ; N échecs
   consécutifs de reconnexion STA rouvrent aussi la fenêtre Improv sans effacer les identifiants
   (*non nécessaire : l'advertising est permanent, un appui court sur BOOT suffit*).

**Implémentation** — fait le 2026-10-04

- [x] `crates/improv` : protocole (paquets RPC avec checksum, Wi-Fi settings, identify, device
  info, résultats) et machine à états (autorisation par bouton avec expiration à 60 s,
  provisioning, succès/échec), 15 tests hôte. Aucune crate existante sur crates.io.
- [x] `firmware/light-flash/src/provisioning.rs` : thread propriétaire de NimBLE (`esp32-nimble`),
  service GATT Improv (cinq caractéristiques), advertising avec *service data* `0x4677` mis à jour
  à chaque changement d'état (nom dans la réponse de scan), bouton BOOT (appui court =
  autorisation, appui long 5 s = réinitialisation d'usine), indications lumineuses
  (`light_core::status`), demande de connexion au thread réseau (`NetworkHandle::connect`),
  enregistrement NVS, URL renvoyées : `http://light-flash.local/` et `http://<ip>/`.
- [x] `network.rs` : canal de commandes `Connect` ; en cas de succès le thread réseau adopte les
  identifiants (`Policy::set_has_credentials`), sinon il relance le point d'accès s'il y était.
- [x] Capacités annoncées : identify, device info, scan Wi-Fi (`0x07`).
- [x] Sécurité : autorisation par BOOT obligatoire (`require_authorization: true`), le mot de passe
  n'est jamais journalisé. Pas d'appairage BLE (Improv ne le prévoit pas).
- [x] Debug : `POST /api/debug/improv-authorize` simule l'appui sur BOOT (build debug seulement).
- [x] **Scan Wi-Fi** (`0x04`, 2026-10-04) : commande `Scan` du thread réseau (`wifi::scan`,
  dédoublonnage par SSID, tri par signal, 20 réseaux maximum), un résultat RPC par réseau
  (SSID, RSSI, YES/NO) puis un résultat vide ; le point d'accès de secours tourne en mode mixte
  (point d'accès + station inactive) car le scan exige une interface station.
- [x] **BLE arrêté quand il ne sert pas** (2026-10-04) : `BLEDevice::deinit` 5 min après
  l'allumage, le dernier appui BOOT ou le dernier provisioning ; un appui BOOT relance la pile
  (`BLEDevice::init` puis `take`, services conservés par esp32-nimble). Vérifié : tas libre de
  56 Ko BLE actif à 107 Ko BLE coupé ; après rallumage, device info, scan et provisioning complet
  fonctionnent. Premier essai avec `take()` seul : erreur 0x1E puis plantage dans
  `ble_gatts_reset`, corrigé par l'appel explicite à `init()`.
- [ ] **P3 — Coexistence** : avec BLE actif, l'association Wi-Fi au boot prend parfois plus de
  20 s au lieu de 4 ; `CONFIG_ESP_COEX_SW_COEXIST_ENABLE=y` est déjà actif, à surveiller.
- [ ] **P1 — Accès aux fonctions BOOT et RESET pour l'utilisateur final** (question du
  2026-10-04). Sur l'applique finale, les boutons de la carte seront dans le boîtier. Décision à
  prendre, recommandation : (a) un **bouton poussoir sur l'applique**, câblé entre GPIO9 et la
  masse (la carte a déjà la résistance de rappel), qui reprend les deux gestes : appui bref =
  visible en Bluetooth + autorisation, appui long = réinitialisation ; (b) dans la **page**, trois
  actions une fois sur le réseau : « Rendre visible en Bluetooth 5 min », « Oublier le Wi-Fi »
  (avec confirmation) et « Redémarrer » ; (c) RESET n'a pas besoin d'être accessible, débrancher
  suffit. Sans bouton accessible, alternative : fenêtre d'autorisation automatique de 3 min après
  la mise sous tension tant que la lampe n'est pas provisionnée (`require_authorization`
  conditionnel) ; moins sûr, mais c'est l'usage courant des objets connectés.
- [ ] **P1 — Nommer chaque applique** (question du 2026-10-04, plusieurs lampes). Aujourd'hui le
  nom `light-flash` est fixe partout (BLE, mDNS, DHCP, point d'accès) : deux lampes entreraient en
  conflit (mDNS renomme en `light-flash-2`, le nom DHCP devient ambigu). À faire : (1) suffixe
  unique par défaut dérivé de l'adresse MAC (`light-flash-df60`) ; (2) nom choisi par l'utilisateur
  (« salon 1 ») stocké en NVS, modifiable depuis la page et par la commande Improv Device name
  (`0x06`), normalisé pour l'hôte mDNS et DHCP (`salon-1`, minuscules, lettres, chiffres, tirets,
  `esp_netif_set_hostname` à chaud) et affiché tel quel dans le titre de la page et le nom BLE.
- [ ] **P2 — Piloter plusieurs appliques ensemble** (question du 2026-10-04). Chaque lampe a sa
  page et ses réglages ; rien ne les relie. Options : (a) **Home Assistant** : groupe de lumières,
  zéro code côté lampe, c'est la voie naturelle pour qui a une domotique ; (b) **groupe dans la
  page** : la lampe découvre ses semblables par mDNS (`_http._tcp`), les expose sur `/api/peers`,
  et la page envoie la même commande à toutes (nécessite les en-têtes CORS sur l'API) ; (c)
  synchronisation lampe à lampe (une « maîtresse » relaie ses changements aux autres par HTTP ou
  ESP-NOW). Recommandation : (a) documenté, puis (b) pour l'usage sans domotique ; (c) seulement
  si (b) ne suffit pas.

**Tests**

- [x] Hôte : parseur (checksum faux, longueurs incohérentes, paquet tronqué, SSID vide, UTF-8
  invalide) et machine à états (commande hors fenêtre d'autorisation, expiration, échec puis
  nouvel essai, double provisioning).
- [x] Carte, depuis le PC avec un client Python `bleak` (2026-10-04) : découverte avec service
  data `01 03 00 00 00 00`, device info, identify, refus sans autorisation (`0x04`), paquet
  invalide (`0x01`), puis autorisation simulée → Wi-Fi settings → état provisioning →
  résultat `http://light-flash.local/` + `http://192.168.1.37/` → provisionné en 6,4 s.
- [ ] Carte, avec l'appli Home Assistant ou Improv sur téléphone : scénario nominal avec appui
  réel sur BOOT, mauvais mot de passe, réinitialisation d'usine par appui long.

---

## 3. Corrections de bugs

### `firmware/light-flash/src/main.rs`

Le `main.rs` de démonstration BLE a été remplacé le 2026-10-04 (tâche lumière + Wi-Fi + HTTP) ;
ses défauts (aucun pilotage de LED, logger non initialisé, `unwrap()` dans un callback NimBLE,
`main()` sans `Result`) disparaissent avec lui. Le BLE réintroduit par Improv (`provisioning.rs`)
n'a aucun `unwrap()` dans les callbacks : `on_write` ne fait que relayer vers un canal.
- [x] **Dépendances inutiles** (fait le 2026-10-04) : `heapless` et la dépendance directe à
  `esp-idf-sys` retirées, `link_patches` appelé via `esp_idf_svc::sys`, section `[features]` vide
  supprimée.
- [ ] **P3 — Sécurité BLE** : Improv ne prévoit pas d'appairage, la protection est l'appui sur
  BOOT. Un appairage avec bonding (`ble_device.security().set_auth(...)`,
  `CONFIG_BT_NIMBLE_NVS_PERSIST=y`) n'aurait de sens qu'avec un service de pilotage BLE (§2.2).
- [ ] **P2 — Sécurité HTTP** : aucune authentification sur `/api/light` ni `/connect`. Acceptable
  sur le point d'accès de secours (mot de passe WPA2) et sur un réseau domestique ; prévoir au
  minimum un jeton si la lampe est exposée au-delà.

### `crates/wifi/src/lib.rs`

- [x] **`expect()` sur la conversion SSID / mot de passe** : remplacés par des erreurs (2026-10-04).
- [x] **Aucune reconnexion** (fait le 2026-10-04) : thread réseau dans `light-flash` piloté par
  `light_core::reconnect::Policy` (backoff 2 s → 30 s, repli point d'accès après 90 s de
  déconnexion, nouvel essai station toutes les 2 min, retour en station dès succès ; 6 tests
  hôte). Les événements `StaDisconnected` sont journalisés avec leur raison. Vérifié sur carte :
  déconnexion forcée → API joignable en moins de 6 s ; SSID inexistant → tentatives à 0,7 / 21 /
  43 / 70 s, point d'accès à 92 s, nouvel essai station à 212 s, point d'accès relancé à 229 s.
- [x] **`EspWifi::new(modem, sysloop, None)`** : `light-flash` passe désormais `Some(nvs)`
  (2026-10-04) ; `hardware-check` reste sans NVS via le raccourci `wifi()`.
- [ ] **P3 — Méthode d'auth devinée** (`WPA2Personal` si mot de passe non vide) : WPA3-only non géré.
  Utiliser `AuthMethod::WPA2WPA3Personal` ou la valeur renvoyée par le scan.

### `crates/rgb-led/src/lib.rs`

- [x] **Un seul pixel** : `set_pixels(&[RGB8])` avec `VariableLengthSignal` et impulsions
  précalculées (2026-10-04). Validé sur la LED embarquée ; à valider sur 144 LED une fois le ruban
  câblé (§6). Alternative toujours ouverte : `ws2812-esp32-rmt-driver` + `smart-leds`.
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

### Branche `feature/connect2Wifi` — revue et intégration (2026-10-04)

Revue de `common/lib/http_server`, `common/lib/wifi_ap` et du `main.rs` de la branche, puis
intégration dans `master` sous `crates/http-server` et `crates/wifi`. La branche peut être supprimée.

- [x] **Fuite des identifiants** : `/connect` journalisait SSID et mot de passe et renvoyait le
  mot de passe dans la réponse. Corrigé : seul le SSID est journalisé, la réponse est un JSON neutre.
- [x] **Provisioning incomplet** : rien n'était stocké ni appliqué. Corrigé : validation des
  longueurs (`WifiCredentials::new`), écriture NVS (`crates/storage`), redémarrage 2 s plus tard,
  station au boot suivant avec repli point d'accès.
- [x] **Mot de passe AP codé en dur** (`password123`) : lu dans `cfg.toml` (`ap_password`,
  8 caractères minimum, vide = ouvert avec avertissement).
- [x] **Code mort dans `main.rs`** (`join()` d'un thread qui `park()`) : `main` garde le serveur
  vivant et boucle avec `sleep`, en journalisant le tas libre toutes les minutes.
- [x] **`RUST_BACKTRACE=1` dans un `sdkconfig.defaults`** : disparu avec le `sdkconfig` unique.
- [x] **Réponses sans `Content-Type`**, erreurs JSON renvoyées en 200, corps non borné
  (`MAX_LEN` 128 trop juste pour un SSID + mot de passe) : `Content-Type` sur toutes les réponses,
  400/413 explicites, corps borné à 512 octets, `deny_unknown_fields`.
- [x] **Logo PNG de 134 Ko embarqué** en fond de page : retiré (flash et bande passante du point
  d'accès) ; page de pilotage de 5 Ko, sans ressource externe, en français.
- [x] **`wifi_ap` : `EspWifi::wrap_all` + netif DHCP client sur un point d'accès** : remplacé par
  une configuration AP simple sur le driver partagé (`start_access_point`), même driver réutilisé
  pour la station (`connect_sta`), ce qui permet le repli sans recréer le modem.
- [x] **`scan()` systématique avant `connect()`** dans `wifi` : supprimé (2 à 3 s de boot
  gagnées) ; `expect()` sur les conversions SSID/mot de passe remplacés par des erreurs.
- [x] **Vérification sur carte et depuis le PC** (2026-10-04, lampe en station sur le réseau
  domestique) : `GET /` 200 en 5 Ko, `GET/POST /api/light` corrects, sept cas d'erreur en 400/413
  avec message, 20 requêtes en 0,8 s, `/connect` → NVS → redémarrage → reconnexion en 12 s ;
  après `make erase` la source d'identifiants est `cfg.toml`, puis la NVS après `/connect`
  (vérifié en retirant `cfg.toml` le temps d'un flash).
- [x] **Hostname** (fait le 2026-10-04) : `light-flash.local` par mDNS (composant
  `espressif/mdns`, `EspMdns`) et `light-flash` par DHCP (`CONFIG_LWIP_LOCAL_HOSTNAME`), voir §5.4.
- [ ] **P2 — `embedded-svc` en dépendance directe** de `http-server` (traits `Headers`, `Read`,
  `Write`) : vérifier si `esp_idf_svc::http` / `esp_idf_svc::io` les ré-exportent en 0.53 et
  supprimer la dépendance lors de la mise à jour.
- [ ] **P2 — Portail captif** : sans serveur DNS répondant à tout, le téléphone n'ouvre pas la
  page automatiquement ; il faut saisir http://192.168.71.1/. À ajouter si le repli SoftAP reste
  le mode d'entrée principal.

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
- [x] **Rôles NimBLE** : `CONFIG_BT_NIMBLE_ROLE_CENTRAL=n`, `CONFIG_BT_NIMBLE_ROLE_OBSERVER=n`
  (2026-10-04, la lampe est périphérique seulement).
- [ ] **P1 — Désactiver les autres composants IDF inutiles** dans `sdkconfig.defaults` :
  `CONFIG_MBEDTLS_CERTIFICATE_BUNDLE=n` tant qu'il n'y a pas de TLS sortant (≈ 60–80 Ko de
  flash), `CONFIG_LOG_DEFAULT_LEVEL_WARN=y` en release.
- [ ] **P2 — Profil release Rust** : ajouter `lto = "fat"`, `codegen-units = 1`, `strip = true`
  (n'affecte que la partie Rust ; `panic = "abort"` est déjà imposé par `build-std`).
- [x] **Taille mesurée** (2026-10-04, `make image`) : 1,75 Mo en debug, 1,51 Mo en release avec
  BLE + Wi-Fi + HTTP + mDNS. À consigner à chaque version (CI, §5.6).

### 4.3 Exécution

- [x] **Démarrage Wi-Fi** : le `scan()` avant `connect()` a été supprimé (2026-10-04).
- [x] **Driver LED** : impulsions calculées une fois dans `new()`, `1 << i`, `counter_clock()`
  lu une fois (2026-10-04).
- [x] **Coexistence BLE / Wi-Fi** : `CONFIG_ESP_COEX_SW_COEXIST_ENABLE=y` est actif par défaut
  (vérifié dans le sdkconfig généré) ; le tas libre est journalisé chaque minute.
- [ ] **P1 — Mémoire avec BLE actif** : 40 Ko de tas libre le 2026-10-04 avec NimBLE par défaut
  (173 Ko sans BLE). Première passe de réduction dans `sdkconfig.defaults` (une connexion, rôles
  central/observateur coupés, tampons NimBLE et Wi-Fi réduits, pile principale 12 Ko) : 58 Ko
  libres après 60 s, BLE et Wi-Fi vérifiés. Seconde passe (2026-10-04) : BLE arrêté 5 min après
  l'allumage ou le dernier appui BOOT (§2.4), ce qui rend sa mémoire en exploitation normale.
  Reste : réduire les piles des threads (`provision` 16 Ko, `network` 12 Ko) après mesure du pic.
- [x] **Usure de la flash** : écriture différée de 2 s et seulement si l'état change
  (`SaveScheduler`, 2026-10-04).
- [x] **Rendu couleur** : correction gamma 2,2 (table de 256 entrées, `light_core::color::Gamma`)
  appliquée par le `Renderer` (2026-10-04).
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
- [x] **README** réécrit sans les instructions WSL2 (2026-10-04) : Ubuntu natif, carte en
  `/dev/ttyACM0`, accès par le groupe `dialout` ou la règle udev.

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
  seule partition `factory` : impossible de mettre à jour sans câble. Attention : avec BLE,
  Wi-Fi, HTTP et mDNS, l'application fait 1,75 Mo en debug (2026-10-04) ; mesurer la taille en
  release avant de fixer la taille des slots (1,5 Mo chacun ne suffirait pas en debug).
  ```csv
  # Name,    Type, SubType,  Offset,   Size
  nvs,       data, nvs,      0x9000,   0x6000
  otadata,   data, ota,      0xf000,   0x2000
  phy_init,  data, phy,      0x11000,  0x1000
  ota_0,     app,  ota_0,    0x20000,  0x1C0000
  ota_1,     app,  ota_1,    0x1E0000, 0x1C0000
  storage,   data, spiffs,   0x3A0000, 0x60000
  ```
  Slots de 1,75 Mo : l'image release fait 1,51 Mo le 2026-10-04 (BLE + Wi-Fi + HTTP + mDNS), la
  debug 1,75 Mo ; l'OTA se fera en release.
- [ ] **P2 — OTA par HTTP** avec `esp_idf_svc::ota::EspOta` : la lampe télécharge l'image depuis
  une URL fournie (serveur local pendant le dev), écrit dans le slot inactif, redémarre, confirme
  (`mark_running_slot_valid`) après un boot sain, sinon rollback automatique
  (`CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`).
- [ ] **P3 — Signature des images** (`CONFIG_SECURE_SIGNED_APPS_NO_SECURE_BOOT=y`) une fois l'OTA
  en place.

### 5.4 Provisioning et persistance

- [x] **Stockage NVS** des identifiants Wi-Fi (`crates/storage`, espace `light`, 2026-10-04).
  `cfg.toml` ne sert plus que d'identifiants de secours et de mot de passe du point d'accès.
- [x] **Stockage NVS du dernier état de la lampe** (2026-10-04, voir §2.2).
- [x] **Séquence de boot** (2026-10-04) : NVS → sinon `cfg.toml` → station, reconnexion
  automatique et repli point d'accès gérés par le thread réseau ; Improv BLE actif en permanence,
  autorisation par BOOT ; signalisation par la lampe (`light_core::status`).
- [x] **Reset usine** : appui long 5 s sur BOOT (GPIO9) efface identifiants et état, puis
  redémarre (2026-10-04, `provisioning.rs`).
- [ ] **P1 — Accès à la page depuis le téléphone sur le réseau domestique** (question du
  2026-10-04). Aujourd'hui l'adresse n'est visible que dans le journal série. Options, par ordre
  de recommandation :
  1. [x] **mDNS + DNS-SD** (fait le 2026-10-04) : `http://light-flash.local/` et annonce
     `_http._tcp` (composant `espressif/mdns` déclaré dans `firmware/light-flash/Cargo.toml`,
     `ESP_IDF_SYS_ROOT_CRATE` dans `.cargo/config.toml`, `EspMdns` dans `main.rs`). Vérifié depuis
     le PC. Natif sur iPhone, Mac, Windows 10+, Linux ; inégal sur Android.
  2. [x] **Nom DHCP `light-flash`** (fait le 2026-10-04, `CONFIG_LWIP_LOCAL_HOSTNAME`) : la Livebox
     résout `light-flash` et `light-flash.home` vers la lampe, vérifié avec `dig`.
  3. [x] **Réservation DHCP dans la box** : documentée dans le README (section « Accéder à la
     lampe »), avec la méthode Livebox.
  4. [x] **Improv (§2.4, fait le 2026-10-04)** : à la fin du provisioning, la lampe renvoie
     `http://light-flash.local/` et `http://<ip>/`, l'appli du téléphone les ouvre directement.
  5. [ ] **P3 — Page installable (PWA)** : manifeste + icône pour un raccourci « application »
     sur le téléphone ; ne résout pas l'adresse, à combiner avec 1 à 3. En attendant, le manuel
     explique « Ajouter à l'écran d'accueil ».
  6. [x] **Domotique et accès distant** : documentés dans le README (Home Assistant via mDNS,
     VPN ; jamais d'ouverture de port). L'intégration Home Assistant elle-même reste à faire (§7).

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
  GitHub. La version (`CARGO_PKG_VERSION`) est déjà renvoyée par Improv (device info) ; l'exposer
  aussi sur `/api/status` avec le tas libre et l'état réseau.

---

## 6. Matériel : prototype du ruban LED

Guide de montage complet (contraintes électriques, liste d'achats, repérage du ruban, câblage,
règles de sécurité, affectation des broches, mise en route) dans **`HARDWARE.md`**.
Matériel disponible au 2026-10-04 : ruban WS2812B 5 V, 1 m, 144 LED/m, IP65 ; carte
ESP32-C3-DevKit-RUST-1 ; breadboard et fils Dupont. Aucun prototype construit. Broches retenues :
voyant GPIO2 (RMT canal 0), ruban GPIO3 (RMT canal 1), bouton BOOT GPIO9.

- [x] **LED embarquée invisible — résolu le 2026-10-04 : défaut matériel de la première carte.**
  Même firmware, même driver : rien sur la première carte malgré `led_probe` (GPIO2 et GPIO8) et
  `led_probe_original` (driver std-training mot pour mot, RMT 40 MHz) ; tout s'allume sur une
  seconde ESP32-C3-DevKit-RUST-1. Les deux exemples restent dans `crates/rgb-led/examples` comme
  outils de diagnostic. La luminosité par défaut est passée de 96 à 160 (après gamma 2,2, 96 ne
  donnait que 12 %).
- [ ] **P0 — Achats** : alimentation 5 V 8 à 10 A avec jack, adaptateur jack vers bornier,
  74AHCT125N, résistance 330 Ω, condensateur 1000 µF, fil 18 AWG, multimètre
  (liste détaillée dans `HARDWARE.md` §3).

### 6.1 Plan de mise en route

- [x] **Étape 0, sans ruban** : `rgb-led` étendu à N pixels et validé sur la LED embarquée
  (2026-10-04). Pour le ruban : `LED_COUNT` et `MAX_MILLIAMPS` dans `light_task.rs`, broche GPIO3.
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
- [x] **Plafond de courant dans le firmware** : `light_core::power::limit` (par LED :
  (r + g + b) / 765 × 60 mA, plus 1 mA de veille), appliqué à chaque trame avant envoi, testé sur
  l'hôte (2026-10-04). Le budget (`MAX_MILLIAMPS`, 500 mA aujourd'hui) sera fixé avec
  l'alimentation du ruban.
- [ ] **P2 — Montage final** : perfboard ou petite PCB avec borniers, fusible 10 A, profilé
  aluminium, boîtier pour l'alimentation ; carte alimentée par sa broche 5V, USB débranché.

---

## 7. Idées (P3)

- Bouton BOOT (GPIO9) comme interrupteur physique / cycle d'effets : en conflit avec son usage
  actuel (autorisation Improv, reset usine) ; prévoir plutôt un bouton dédié sur l'applique.
- Capteurs embarqués de la DevKit-RUST-1 (IMU ICM-42670-P et SHTC3 sur I2C, SDA GPIO10 / SCL GPIO8) :
  « tap » pour allumer, inclinaison pour varier l'intensité, couleur fonction de la température.
- Intégration domotique : Home Assistant via MQTT (`esp_idf_svc::mqtt`) ou ESPHome-like API.
