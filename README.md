# light-flash

Applique lumineuse pilotée par une carte **ESP32-C3-DevKit-RUST-1** et un ruban **WS2812B 144 LED**.
Firmware en Rust (`std`, ESP-IDF v5.3.2).

| Document | Contenu |
|---|---|
| `CLAUDE.md` | Chaîne de compilation, conventions, pièges connus |
| `BACKLOG.md` | Préconisations et tâches : architecture, bugs, optimisations, déploiement |
| `HARDWARE.md` | Prototype du ruban : achats, câblage, sécurité, mise en route |
| `REFERENCES.md` | Liens utiles |

## Organisation

```
Cargo.toml                workspace : membres, versions communes, profils
.cargo/config.toml        cible riscv32imc-esp-espidf, ldproxy, runner espflash, ESP-IDF v5.3.2
rust-toolchain.toml       nightly épinglé + rust-src
sdkconfig.defaults        configuration ESP-IDF (NimBLE, tailles de pile)
cfg.toml.example          modèle d'identifiants Wi-Fi, à copier en cfg.toml (ignoré par git)
Makefile                  point d'entrée unique de toutes les commandes
firmware/light-flash      firmware principal
firmware/hardware-check   test de la carte : Wi-Fi + LED embarquée
crates/light-core         domaine (état, commandes, rendu, puissance, API JSON), sans dépendance ESP
crates/rgb-led            driver WS2812 via RMT (N pixels)
crates/wifi               Wi-Fi station ou point d'accès
crates/storage            NVS : identifiants Wi-Fi
crates/http-server        page de pilotage, API JSON, formulaire Wi-Fi
```

## Démarrage (Ubuntu)

Toute commande passe par le Makefile : `make help` liste les cibles.

```bash
make setup                        # paquets, toolchains, ldproxy, espflash, accès série (sudo demandé)
make doctor                       # vérifie l'installation et détecte la carte
cp cfg.toml.example cfg.toml      # puis renseigner wifi_ssid / wifi_psk (hardware-check, exemple wifi)
make run                          # flash light-flash + moniteur série
make run   CRATE=hardware-check   # test de la carte (nécessite cfg.toml)
make test                         # tests hôte de light-core
```

Au premier démarrage, la lampe ouvre le point d'accès Wi-Fi `light-flash` (mot de passe
`light-flash`, modifiable dans `cfg.toml`). S'y connecter et ouvrir http://192.168.71.1/ : la page
permet d'allumer, choisir couleur, luminosité et effet, et d'enregistrer le réseau Wi-Fi de la
maison. La lampe redémarre alors sur ce réseau ; son adresse s'affiche dans le moniteur série.

La première compilation télécharge et construit ESP-IDF (plusieurs minutes, environ 2 Go dans
`~/.espressif`).

## Attribution

`hardware-check`, `rgb-led` et `wifi` dérivent de [esp-rs/std-training](https://github.com/esp-rs/std-training)
(Ferrous Systems et Espressif, licence MIT OR Apache-2.0).
