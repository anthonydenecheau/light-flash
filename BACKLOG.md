# BACKLOG — light-flash

Tâches restantes du projet, tenues à jour au fil des réalisations. Priorités : **P0** bloquant /
à faire en premier, **P1** important, **P2** souhaitable, **P3** idée. Nettoyé le 2026-10-04 :
les items réalisés ont été retirés ; ce qui a été fait et pourquoi se lit dans `CLAUDE.md`
(architecture, pièges), dans le manuel et dans l'historique git. La numérotation des sections est
conservée (renvois depuis `CLAUDE.md` et `HARDWARE.md`).

**État au 2026-10-04.** Fonctionnel et vérifié sur carte, LED embarquée seule : pilotage par
page et API, persistance, reconnexion Wi-Fi avec repli, mDNS et nom DHCP, Improv sur BLE, nom
par lampe, mode groupe, mise à jour par HTTP local, scènes, minuterie et horaires, manuel
(chapitres 1 et 2). **Prochaines priorités, dans l'ordre :** (1) matériel du ruban (§6),
(2) mémoire avec BLE actif (§4.3), (3) authentification de la page (§3), (4) mise à jour des
versions (§2.3), (5) CI (§5.6), (6) devcontainer (§5.5). Une question en attente du
propriétaire : batterie ou non (§1). Licence choisie le 2026-10-04 : MIT OR Apache-2.0.

**Décisions prises** (pour mémoire, détail dans `CLAUDE.md`) :

- Provisioning par **Improv Wi-Fi sur BLE** plutôt qu'un portail Wi-Fi seul : réutilise NimBLE,
  autorisation par bouton, applis existantes ; le point d'accès de secours reste le repli.
- **`std` / ESP-IDF** confirmé (BLE, Wi-Fi, HTTP, mDNS, NVS ensemble) ; pas de migration `no_std`.
- **`SharedState = Arc<Mutex<LightState>>`** plutôt qu'un canal : les producteurs lisent aussi
  l'état ; le verrou n'est jamais tenu pendant l'accès au driver.
- **Page embarquée sans framework**, servie gzip ; appli native, page hébergée hors de la lampe
  (contenu mixte https→http) et Matter écartés ; Home Assistant en complément.
- **Mode groupe par la page** (mDNS + CORS) ; synchronisation lampe à lampe seulement si cela ne
  suffit pas.
- **OTA par HTTP local** avec un client minimal sur `TcpStream` (celui d'ESP-IDF entraîne
  mbedTLS, ≈ 300 Ko) ; SHA-256 obligatoire, signature plus tard (§5.3).
- **Bouton poussoir sur l'applique** (GPIO9) pour les gestes BOOT ; RESET inutile, débrancher suffit.
- **Mise à jour des versions** dans un commit dédié, jamais mêlée à une fonctionnalité.

---

## 1. Décisions à prendre

- [ ] **P1 — Finir la spécification fonctionnelle.** Fixé : ruban WS2812B 144 LED, marche/arrêt,
  couleur, luminosité, trois effets, scènes et horaires, page et API HTTP, Improv, dernier état
  restauré au démarrage. Reste à fixer : luminosité maximale et budget de courant avec le ruban
  (§6.1), effets supplémentaires souhaités (§2.5), pilotage BLE direct ou non (§2.2).
- [ ] **P1 — Alimentation sur batterie ?** Le montage de `HARDWARE.md` est sur secteur et la
  carte n'a ni connecteur batterie ni jauge : un indicateur de charge exige d'abord un choix
  matériel. Si batterie : préciser type et capacité (144 LED à 30 % ≈ 2,6 A sous 5 V, 13 W), puis
  mesure par diviseur de tension sur ADC (GPIO0 ou GPIO1, ±10 %) ou, recommandé, jauge I2C
  MAX17048 sur le bus existant (GPIO8/GPIO10, adresse 0x36, ±2 %, vitesse de décharge donc temps
  restant). Logiciel : `GET /api/power` (`source`, `percent`, `voltage`, `current_ma`,
  `remaining_min`), jauge dans la page, masquée sans batterie. Sans batterie : afficher la
  consommation estimée du ruban (`power::estimate_ma`, déjà calculée à chaque trame).

---

## 2. Architecture

### 2.1 Workspace

Fait. Notes de mise en œuvre (profils, `Cargo.lock` et nightly, `cfg.toml`, `build.rs`) dans
`CLAUDE.md`.

### 2.2 Domaine et périphériques

- [ ] **P2 — `LED_COUNT` et `MAX_MILLIAMPS`** (1 LED, 500 mA dans `light_task.rs`) à passer à
  144 et au budget de l'alimentation quand le ruban sera câblé (§6).
- [ ] **P2 — Pilotage BLE direct** : service GATT « light » (UUID 128 bits) : `power`, `color`
  (3 octets), `brightness`, `effect` (R/W/N), `status` (N), sur le même serveur NimBLE
  qu'Improv. Utile seulement pour piloter sans Wi-Fi ; à décider avec la spécification (§1).
- [ ] **P3 — Comportement après coupure secteur** : la lampe revient dans son dernier état, donc
  éteinte si elle l'était. Si l'applique est commandée par un interrupteur mural, prévoir une
  option « toujours allumer à la mise sous tension », réglable depuis la page.

### 2.3 Hygiène du dépôt

- [ ] **P1 — Mettre à jour les versions** (analyse du 2026-10-04). L'ensemble actuel est celui
  de `esp-rs/std-training` (connu bon) ; l'ensemble « dernier » est cohérent entre lui
  (`esp32-nimble 0.13` exige `esp-idf-svc 0.53.0`). L'ESP32-C3 est supporté partout.

  | Composant | Actuel | Dernier (2026-10-04) | Remarques |
  |---|---|---|---|
  | `esp-idf-svc` | 0.51.0 | 0.53.0 | MSRV 1.82 ; ESP-IDF 5.3 à 6.0 ; nouveaux variants `WifiEvent` ; API NVS modifiée ; wrappers NimBLE natifs ; corrige l'advertising BLE sur C3 avec ESP-IDF ≥ 5.5 |
  | `esp-idf-hal` | 0.45.2 | 0.47.0 | va avec svc 0.53 |
  | `esp-idf-sys` | 0.36.1 | 0.38.1 | ESP-IDF < 5.3 déprécié ; 5.4/5.5 supportés |
  | `esp32-nimble` | 0.10.2 | 0.13.0 | exige svc 0.53.0 |
  | `embuild` | 0.33.5 | 0.33.5 | à jour |
  | `toml-cfg` | 0.1.3 | 0.2.0 | mineur |
  | `rgb` | 0.8.53 | 0.8.53 | 0.8.92-rc est une pré-version, ne pas la prendre |
  | ESP-IDF | v5.3.2 | v5.5.x (6.0 trop récent) | recompilation complète |
  | nightly | 2025-01-01 | nightly récent, **daté** | aucune date imposée (MSRV 1.82) ; figer la date du jour de la mise à jour |

  Le nightly de janvier 2025 oblige déjà à brider des dépendances transitives (résolveur MSRV,
  `ignore` épinglé) ; chaque `cargo update` risque d'en réveiller d'autres : faire la mise à
  jour tôt. Ordre : un commit dédié qui passe crates, ESP-IDF v5.5 et nightly ensemble, corrige
  les ruptures d'API (`WifiEvent`, NVS, `embedded-svc`, voir §3), puis validation complète sur
  carte (Wi-Fi, BLE, OTA).
- [ ] **P2 — Verser les scripts de test PC dans le dépôt** (`tools/`) : client Improv `bleak`,
  seconde lampe simulée (zeroconf + mini API), scénarios Playwright (mode groupe, fenêtres,
  mises à jour), test minuterie/horaires par l'API. Ils vivent aujourd'hui dans le scratchpad de
  session de Claude Code et seront perdus avec elle ; les documenter dans `CLAUDE.md`.
- [ ] **P3 — Supprimer la branche distante** `origin/feature/connect2Wifi`, intégrée et obsolète
  (`git push origin --delete feature/connect2Wifi`).

### 2.4 Provisioning Improv sur BLE

Implémenté (`crates/improv`, `provisioning.rs`) ; spécification : https://www.improv-wifi.com/ble/.

- [ ] **P1 — Test avec un vrai téléphone** (appli Home Assistant ou Improv Wi-Fi) : scénario
  nominal avec appui réel sur BOOT, mauvais mot de passe, réinitialisation d'usine par appui
  long. Seul le client Python `bleak` depuis le PC a été exercé.
- [ ] **P3 — Commande Improv « Device name »** (`0x06`) pour nommer la lampe depuis l'appli, en
  plus de la page.
- [ ] **P3 — Coexistence** : avec BLE actif, l'association Wi-Fi au boot prend parfois plus de
  20 s au lieu de 4 ; `CONFIG_ESP_COEX_SW_COEXIST_ENABLE=y` est déjà actif, à surveiller.
- [ ] **P3 — Sans bouton accessible** : fenêtre d'autorisation automatique de 3 min après la
  mise sous tension tant que la lampe n'est pas provisionnée (`require_authorization`
  conditionnel). Moins sûr, mais c'est l'usage courant des objets connectés. Sans objet si le
  bouton de l'applique est câblé (§6).

### 2.5 Interface utilisateur

- [ ] **P2 — Programmation, suite** : transitions en fondu (allumage progressif le matin,
  extinction douce), horaires au lever et coucher du soleil (position à saisir, calcul local),
  rejouer le dernier horaire passé de la journée au démarrage, minuterie conservée au
  redémarrage, prochaine action affichée dans la page.
- [ ] **P3 — Mode groupe, suite** : état des autres lampes (allumée ou non), pilotage d'un
  sous-ensemble (cases à cocher), choix « Toutes » mémorisé entre deux visites.
- [ ] **P3 — Effets supplémentaires** et vitesse d'effet réglable depuis la page.

---

## 3. Corrections et sécurité

- [ ] **P1 — Authentification de la page** (manque face à l'état de l'art : Shelly, WLED,
  ESPHome). Aucune protection sur l'API, y compris `/api/system/*`, `/api/name` et la mise à
  jour. Plan : mot de passe optionnel défini depuis la page (haché en NVS), HTTP Basic ou cookie
  de session sur toutes les routes sauf la page elle-même et `/api/light` en lecture ; exempter
  le point d'accès de secours (déjà protégé par WPA2) pour ne pas bloquer la première
  configuration ; le mode groupe devra transmettre le même mot de passe aux autres lampes.
- [ ] **P2 — `rgb-led` : temps de reset** (> 50 µs à l'état bas) après la trame : deux envois
  rapprochés peuvent être lus comme une seule trame. Ajouter une impulsion basse finale ou un
  délai. À vérifier sur le ruban (§6).
- [ ] **P2 — `rgb-led` : `esp-idf-svc` et `log` en `[dependencies]`** alors que seuls les
  exemples les utilisent : passer en `[dev-dependencies]`.
- [ ] **P2 — `http-server` : `embedded-svc` en dépendance directe** (traits `Headers`, `Read`,
  `Write`) : vérifier si `esp_idf_svc::http` / `io` les ré-exportent en 0.53 et supprimer la
  dépendance lors de la mise à jour des versions (§2.3).
- [ ] **P2 — Portail captif** sur le point d'accès de secours : sans serveur DNS répondant à
  tout, le téléphone n'ouvre pas la page automatiquement, il faut saisir http://192.168.71.1/.
- [ ] **P3 — `wifi` : méthode d'authentification devinée** (`WPA2Personal` si mot de passe non
  vide) : WPA3-only non géré. Utiliser `WPA2WPA3Personal` ou la valeur renvoyée par le scan.
- [ ] **P3 — Sécurité BLE** : Improv ne prévoit pas d'appairage, la protection est l'appui sur
  BOOT. Un appairage avec bonding n'aurait de sens qu'avec un service de pilotage BLE (§2.2).
- [ ] **P3 — `rgb-led` : feature `rmt-legacy`** dépréciée dans ESP-IDF 5.x ; migrer vers le
  nouveau driver RMT quand `esp-idf-hal` l'exposera pleinement.

---

## 4. Optimisations

### 4.1 Temps de compilation

Rien d'ouvert hors CI (§5.6). Rappel : toute modification de `sdkconfig.defaults` reconstruit
ESP-IDF, regrouper les changements.

### 4.2 Taille de l'image

Tailles et réductions appliquées dans `CLAUDE.md` (OTA) ; debug ≈ 1,73 Mo, release ≈ 1,65 Mo
pour 2,03 Mo d'emplacement.

- [ ] **P2 — Journal moins bavard en release** (`CONFIG_LOG_DEFAULT_LEVEL_WARN=y`, journaux
  NimBLE réduits) : image plus petite et moins de trafic série ; demande un `sdkconfig` distinct
  par profil ou une variable d'environnement dans le Makefile.

### 4.3 Exécution

- [ ] **P1 — Mémoire avec BLE actif** : tas libre ≈ 23 Ko au démarrage avec BLE (mesuré le
  2026-10-04, tous services lancés), ≈ 77 Ko une fois le BLE coupé (5 min). Déjà fait : une
  connexion, rôles central/observateur coupés, tampons NimBLE et Wi-Fi réduits, BLE arrêté
  après 5 min. Pistes : ne démarrer le BLE au boot que si aucun identifiant Wi-Fi n'existe (sinon
  seulement sur appui BOOT), réduire les piles des threads (`provision` 16 Ko, `network` 12 Ko)
  après mesure du pic (`uxTaskGetStackHighWaterMark`), journaliser le minimum de tas atteint
  (`esp_get_minimum_free_heap_size`).
- [ ] **P3 — Économie d'énergie** : `wifi.set_ps(...)` (modem sleep) si la latence HTTP reste
  acceptable ; sans intérêt tant que l'applique est sur secteur.
- [ ] **P3 — Tick FreeRTOS** : `CONFIG_FREERTOS_HZ=1000` (commenté dans `sdkconfig.defaults`) si
  des effets ont besoin de `sleep` < 10 ms.

---

## 5. Déploiement et flash

### 5.1 Poste de développement

Fait (`make setup`, `make doctor`, convention « tout passe par le Makefile »).

### 5.2 Commandes de flash

- [ ] **P3 — Runner `cargo run`** (`.cargo/config.toml`) : ajouter `--chip esp32c3` et le
  bootloader d'esp-idf-sys pour l'aligner sur `make flash` ; n'affecte que `make example`.

### 5.3 OTA

- [ ] **P2 — Signature des images** (`CONFIG_SECURE_SIGNED_APPS_NO_SECURE_BOOT=y`, clé dans le
  dépôt de publication seulement) : refuser un `.bin` qui ne vient pas de vous. Manque face à
  l'état de l'art, à faire avec l'authentification de la page (§3).
- [ ] **P3 — Débit de téléchargement** : ≈ 60 Ko/s avec BLE actif ; couper le BLE pendant le
  téléchargement ou agrandir les tampons Wi-Fi pour viser 300 Ko/s.

### 5.4 Provisioning et persistance

Fait. L'intégration Home Assistant est une idée (§7).

### 5.5 Devcontainer

- [ ] **P1 — Réécrire `.devcontainer/Dockerfile`** : base Debian bookworm, `nightly-2025-01-01`,
  `ldproxy`, `espflash`, pré-clone d'ESP-IDF **v5.3.2** dans `~/.espressif` (le Dockerfile actuel
  installe v4.4.4 et nightly-2023-02-28, jamais utilisés puisque `esp-idf-sys` retélécharge
  v5.3.2). Épingler le tag de l'image dans `devcontainer.json` au lieu de `latest`. À faire
  après la mise à jour des versions (§2.3) pour ne pas le réécrire deux fois.
- [ ] **P2 — Simulation Wokwi** (`wokwi.toml` + `diagram.json` avec ESP32-C3 + WS2812) pour
  tester la logique LED sans carte ; l'extension est déjà listée dans `devcontainer.json`.

### 5.6 Intégration continue

- [ ] **P1 — GitHub Actions** : job Ubuntu qui n'appelle que des cibles du Makefile
  (`make setup-system setup-rust`, `make lint`, `make test`, `make build RELEASE=1`,
  `make image RELEASE=1`), cache `~/.espressif` + `~/.cargo` + `target/`, publie `dist/*.bin`
  en artefact et consigne la taille des images. Premier run 15–30 min (build ESP-IDF), les
  suivants quelques minutes.
- [ ] **P2 — Release** : sur tag `vX.Y.Z`, attacher l'image mergée, l'image application seule
  avec son `manifest.json` (`make publish`) et `partitions.csv` à la release GitHub.

---

## 6. Matériel : prototype du ruban LED

Guide de montage complet dans **`HARDWARE.md`** (contraintes électriques, achats, repérage du
ruban, câblage, sécurité, broches, mise en route). Disponible : ruban WS2812B 5 V, 1 m, 144 LED,
IP65 ; carte ESP32-C3-DevKit-RUST-1 ; breadboard et fils Dupont. Aucun prototype construit.
Broches : voyant GPIO2 (RMT canal 0), ruban GPIO3 (RMT canal 1), bouton GPIO9. Le driver est déjà
N pixels et le plafond de courant (`power::limit`) est en place.

- [ ] **P0 — Achats** : alimentation 5 V 8 à 10 A avec jack, adaptateur jack vers bornier,
  74AHCT125N, résistance 330 Ω, condensateur 1000 µF, fil 18 AWG, multimètre
  (`HARDWARE.md` §3).
- [ ] **P1 — Bouton poussoir de l'applique** entre GPIO9 et GND (`HARDWARE.md` §7.1) : reprend
  les gestes BOOT (appui bref : Bluetooth et autorisation ; appui long : réinitialisation).

### 6.1 Plan de mise en route

- [ ] **P0 — Étape 1, câblage hors tension** selon `HARDWARE.md` §5 ; au multimètre, pas de
  court-circuit 5 V/GND, continuité des masses ; sous tension sans la carte, 5 V aux bornes du
  ruban.
- [ ] **P0 — Étape 2, premier allumage** : écrire l'exemple `strip_test` dans `rgb-led` (tout
  éteint, puis les 8 premières LED en rouge faible). Si elles s'allument, la chaîne de données
  est bonne.
- [ ] **P0 — Étape 3, 144 LED** : chenillard d'une LED sur toute la longueur (les 144 répondent,
  sens), puis arc-en-ciel à 25 % ; passer `LED_COUNT` à 144 dans le firmware (§2.2).
- [ ] **P1 — Étape 4, puissance** : rampe de blanc par paliers de 10 % en touchant régulièrement
  le ruban et l'alimentation ; noter le palier au-delà duquel le bout du ruban jaunit et décider
  de l'injection en bout ; fixer `MAX_MILLIAMPS`.
- [ ] **P2 — Montage final** : perfboard ou petite PCB avec borniers, fusible 10 A, profilé
  aluminium, boîtier pour l'alimentation ; carte alimentée par sa broche 5V, USB débranché.

---

## 7. Idées (P3)

- Capteurs embarqués de la DevKit-RUST-1 (IMU ICM-42670-P et SHTC3 sur I2C, SDA GPIO10 /
  SCL GPIO8) : « tap » pour allumer, inclinaison pour varier l'intensité, couleur fonction de la
  température.
- Intégration domotique : Home Assistant via MQTT (`esp_idf_svc::mqtt`) ou API de type ESPHome ;
  aujourd'hui seulement la découverte mDNS et la page.
- Bouton BOOT comme interrupteur physique ou cycle d'effets : en conflit avec son usage actuel
  (autorisation Improv, reset usine) ; prévoir plutôt un second bouton sur l'applique.
