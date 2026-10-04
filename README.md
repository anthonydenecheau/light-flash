# light-flash

Applique lumineuse pilotée par une carte **ESP32-C3-DevKit-RUST-1** et un ruban **WS2812B 144 LED**.
Firmware en Rust (`std`, ESP-IDF v5.3.2).

| Document | Contenu |
|---|---|
| `CLAUDE.md` | Chaîne de compilation, conventions, pièges connus |
| `BACKLOG.md` | Préconisations et tâches : architecture, bugs, optimisations, déploiement |
| `HARDWARE.md` | Prototype du ruban : achats, câblage, sécurité, mise en route |
| `MANUEL.md` | Manuel utilisateur : mise en service, puis interface et dépannage |
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

## Première configuration du Wi-Fi

Deux méthodes, au choix :

- **Par Bluetooth (Improv Wi-Fi), recommandée** : ouvrir l'application Home Assistant, l'application
  Improv Wi-Fi (Android) ou https://www.improv-wifi.com/ dans Chrome ; la lampe apparaît sous le nom
  `light-flash`. Appuyer brièvement sur le bouton BOOT de la carte (la lampe respire en bleu pendant
  60 s), saisir le réseau et le mot de passe : la lampe clignote en bleu, puis passe au vert et
  l'application ouvre sa page. Rouge : mot de passe ou réseau incorrect, réessayer.
- **Par le point d'accès de secours** : sans Wi-Fi configuré, la lampe ouvre un réseau à son nom
  (`light-flash-xxxx` par défaut, mot de passe `light-flash`, modifiable dans `cfg.toml`). S'y
  connecter, ouvrir http://192.168.71.1/ et renseigner le réseau de la maison dans le formulaire ;
  la lampe redémarre sur ce réseau.

Un appui long (5 s) sur BOOT efface le Wi-Fi et l'état enregistrés, puis redémarre.

## Accéder à la lampe sur le réseau de la maison

Une fois la lampe sur votre Wi-Fi, trois façons d'ouvrir sa page depuis un téléphone ou un
ordinateur du même réseau, de la plus simple à la plus sûre :

1. **http://<nom>.local/** (`light-flash-xxxx.local` par défaut, `salon-1.local` après avoir
   nommé la lampe « Salon 1 » dans sa page) : la lampe s'annonce en mDNS. Fonctionne nativement
   sur iPhone, iPad, Mac, Windows 10 et plus, Linux. Sur Android, cela dépend de la version et du
   navigateur ; si l'adresse ne répond pas, passer au point 2 ou 3.
2. **http://<nom>/** : la lampe se présente à la box sous son nom. Beaucoup de box résolvent ce
   nom sur le réseau local ; vérifié sur Livebox, qui répond aussi à `<nom>.home`. Essayer
   `<nom>.lan` sur d'autres box.
3. **Adresse fixe** : dans l'interface de la box (Livebox : « Réseau », « DHCP », « Baux
   statiques » ; autres box : « réservation DHCP » ou « bail statique »), associer l'adresse MAC
   de la lampe à une adresse fixe, par exemple 192.168.1.50. L'adresse MAC et l'adresse courante
   s'affichent au démarrage dans `make monitor SECS=20`. Ouvrir ensuite `http://192.168.1.50/` et
   l'ajouter à l'écran d'accueil du téléphone (« Ajouter à l'écran d'accueil » dans le navigateur).
   C'est la méthode qui marche partout, Android compris.

Hors de la maison, ne jamais ouvrir de port vers la lampe sur la box : sa page n'a pas de mot de
passe. Deux options saines :

- **Domotique** : Home Assistant découvre la lampe (mDNS) et la pilote depuis son application,
  y compris à distance via Nabu Casa ou le VPN de Home Assistant. Intégration prévue au backlog.
- **VPN vers la maison** : Tailscale, WireGuard ou le VPN de la box ; une fois connecté, les
  adresses ci-dessus fonctionnent comme à la maison.

La première compilation télécharge et construit ESP-IDF (plusieurs minutes, environ 2 Go dans
`~/.espressif`).

## Attribution

`hardware-check`, `rgb-led` et `wifi` dérivent de [esp-rs/std-training](https://github.com/esp-rs/std-training)
(Ferrous Systems et Espressif, licence MIT OR Apache-2.0).
