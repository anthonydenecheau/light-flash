# Matériel et prototype du ruban LED

Dernière mise à jour : 2026-10-04. **Aucun prototype construit à ce jour.** Ce document sert de
guide de montage ; la liste des tâches correspondante est dans `BACKLOG.md` §6.

## 1. Inventaire

| Élément | Détail |
|---|---|
| Carte | ESP32-C3-DevKit-RUST-1 (RISC-V, 4 Mo flash, USB-C natif, LED WS2812 embarquée sur GPIO2) |
| Ruban | WS2812B, 5 V, 1 m, 144 LED/m, adressable individuellement, PCB flexible, IP65 (gaine silicone) |
| Prototypage | breadboard + kit de fils Dupont |
| Machine de dev | Ubuntu natif, carte vue en `/dev/ttyACM0` |

## 2. Trois choses à savoir avant de brancher quoi que ce soit

### 2.1 Ce ruban consomme beaucoup

Une WS2812B tire jusqu'à 60 mA en blanc plein, plus environ 1 mA au repos. Pour 144 LED :

| Scénario | Courant |
|---|---|
| Éteint (veille des puces) | ≈ 0,15 A |
| Blanc 25 % | ≈ 2,2 A |
| Blanc 50 % | ≈ 4,3 A |
| Blanc 100 % | ≈ 8,6 A |

Conséquences :

- **Alimentation 5 V externe obligatoire** pour le ruban. Le port USB de la carte et sa broche 5V
  ne peuvent pas fournir ce courant.
- **Le courant du ruban ne traverse jamais la breadboard.** Breadboard et fils Dupont tiennent
  environ 1 A ; seuls les signaux (données et référence de masse) y passent. L'alimentation va
  directement de l'alim au ruban par du fil 18 AWG et des borniers.
- **Chaleur.** 144 LED/m sous gaine IP65 chauffe vite. Plafonner la luminosité dans le firmware
  (§7) et, pour l'applique finale, coller le ruban sur un profilé aluminium.
- **Chute de tension.** Sur 1 m à fort courant, le bout du ruban vire au jaune/rouge. Au-delà de
  3 A, injecter le 5 V aux deux extrémités.

### 2.2 La carte parle en 3,3 V, le ruban attend 3,5 V

Le WS2812B reconnaît un « 1 » à partir de 0,7 × VDD, soit 3,5 V sous 5 V. L'ESP32-C3 sort 3,3 V.
Ça fonctionne souvent avec un fil court, mais pas de façon fiable : prévoir un adaptateur de niveau
**74AHCT125** (alimenté en 5 V, accepte 3,3 V en entrée, se pose sur la breadboard).
Le 3,3 V ne peut pas endommager le ruban : on peut tenter sans adaptateur, au pire ça scintille.

### 2.3 Les masses doivent être reliées

GND de l'alimentation, du ruban et de la carte reliés ensemble. C'est l'oubli le plus fréquent ;
sans masse commune le ruban reste muet ou affiche n'importe quoi.

## 3. Liste d'achats

Rien à souder : borniers à vis, breadboard et éventuellement des WAGO suffisent.

| Composant | Référence / valeur | Rôle |
|---|---|---|
| Alimentation 5 V DC | 5 V / 8 à 10 A, jack 5,5 × 2,1 mm (bloc secteur « LED ») | alimente le ruban, et la carte au montage final |
| Adaptateur jack femelle vers bornier à vis | 5,5 × 2,1 mm | raccorder l'alimentation sans soudure |
| Adaptateur de niveau | 74AHCT125N (DIP-14) ou SN74HCT245N | 3,3 V vers 5 V sur la ligne données |
| Résistance | 330 à 470 Ω, 1/4 W | en série sur la donnée, près du ruban |
| Condensateur électrolytique | 1000 µF, 10 V ou plus | tampon aux bornes 5 V/GND du ruban |
| Fil souple | 18 AWG (≈ 0,8 mm²), rouge et noir | liaison alimentation vers ruban |
| Multimètre | n'importe lequel | vérifier 5 V, continuité des masses, absence de court-circuit |
| Optionnel : fusible | 10 A sur le 5 V | montage final |
| Optionnel : profilé aluminium + diffuseur | 1 m | dissipation et rendu |
| Optionnel : connecteurs WAGO 221 | | épissures sans soudure |

## 4. Repérage du ruban

- Le ruban a une **entrée (DIN)** et une **sortie (DOUT)** ; des flèches imprimées sur le PCB
  indiquent le sens des données. La carte se branche côté DIN.
- Le connecteur d'entrée porte en général trois fils : **rouge 5 V, blanc ou noir GND, vert
  données**. Vérifier sur l'étiquette ou le PCB, les couleurs varient selon les fabricants.
- Certains rubans IP65 ont en plus deux fils d'alimentation séparés (rouge/noir) : ils vont
  directement sur le bornier de l'alimentation.

## 5. Câblage du prototype

```
Alim 5 V (+) ────┬────────────── fil rouge ruban (5 V)        [injection en bout de ruban si > 3 A]
                 │
            1000 µF (+ côté 5 V, − côté GND, au plus près du ruban)
                 │
Alim 5 V (−) ────┴──┬─────────── fil blanc/noir ruban (GND)
                    ├─────────── GND ESP32-C3                 masse commune obligatoire
                    └─────────── GND 74AHCT125 (broche 7)

74AHCT125 : VCC (broche 14) ── 5 V alimentation
            1OE (broche 1)  ── GND                           (active la sortie)
            1A  (broche 2)  ── ESP32-C3 GPIO3
            1Y  (broche 3)  ── R 330 Ω ── fil vert ruban (DIN)

ESP32-C3 : alimenté par USB pendant le développement.
```

Plan B sans 74AHCT125 : GPIO3, résistance 330 Ω, DIN, avec un fil court. Fonctionne souvent ; au
premier scintillement ou LED fantôme, passer à l'adaptateur de niveau.

## 6. Règles de sécurité

- Couper l'alimentation avant toute modification de câblage.
- **Ne jamais alimenter la carte par sa broche 5V tant que l'USB est branché** : risque de renvoyer
  du courant vers le PC. Au montage final, la carte sera alimentée par sa broche 5V depuis
  l'alimentation du ruban, USB débranché.
- Respecter la polarité du condensateur électrolytique (la bande marquée est le −).
- Ordre de branchement : GND, puis 5 V, puis données. Jamais de données sur un ruban non alimenté.
- Fil de données court (< 30 cm) ; au-delà, le torsader avec un fil de masse.
- Vérifier au multimètre, hors tension, l'absence de court-circuit entre 5 V et GND avant la
  première mise sous tension.

## 7. Affectation des broches

| Fonction | GPIO | Périphérique |
|---|---|---|
| LED embarquée (voyant : provisioning, erreurs) | GPIO2 | RMT TX canal 0 |
| Données ruban 144 LED | GPIO3 | RMT TX canal 1 |
| Bouton BOOT (autorisation Improv, reset usine) | GPIO9 | entrée, pull-up |
| Réservés : I2C des capteurs embarqués (SHTC3, ICM-42670-P) | GPIO8 (SCL), GPIO10 (SDA) | |
| Réservés : USB | GPIO18, GPIO19 | |
| Réservés : UART0 | GPIO20 (RX), GPIO21 (TX) | |

L'ESP32-C3 dispose de deux canaux RMT en émission : un pour le voyant, un pour le ruban.
Une trame de 144 LED dure ≈ 4,3 ms, ce qui laisse largement 60 images/s pour les effets.
Le plafond de courant se calcule dans le firmware par trame : par LED, (r + g + b) / 765 × 60 mA,
plus 1 mA de veille ; la luminosité globale est réduite pour rester sous un budget configurable
(par exemple 5 A). C'est la protection principale de l'alimentation et du ruban.

## 8. Mise en route pas à pas

1. **Sans ruban.** Étendre le driver `rgb-led` à N pixels et le valider sur le voyant embarqué
   avec N = 1 (`make example CRATE=rgb-led EX=ws2812`).
2. **Câblage hors tension** selon §5. Au multimètre : pas de court-circuit 5 V/GND, continuité des
   trois masses.
3. **Mise sous tension sans la carte.** Mesurer 5 V aux bornes du ruban. Le ruban reste éteint ou
   affiche des couleurs aléatoires, c'est normal sans données.
4. **Premier allumage.** Exemple `strip_test` : tout éteint, puis les 8 premières LED en rouge
   faible (10, 0, 0). Si elles s'allument, la chaîne de données est bonne.
5. **144 LED.** Chenillard d'une LED sur toute la longueur (vérifie que les 144 répondent et le
   sens), puis arc-en-ciel à 25 %.
6. **Puissance.** Rampe de blanc par paliers de 10 % en touchant régulièrement le ruban et
   l'alimentation. Noter le palier au-delà duquel le bout du ruban jaunit et décider de l'injection
   en bout.

## 9. Montage final (plus tard)

Perfboard ou petite PCB avec borniers, fusible 10 A sur le 5 V, profilé aluminium avec diffuseur,
boîtier pour l'alimentation, carte alimentée par sa broche 5V, USB débranché.

## 10. Références

- Adafruit NeoPixel Überguide, bonnes pratiques (résistance, condensateur, adaptateur de niveau) :
  https://learn.adafruit.com/adafruit-neopixel-uberguide/best-practices
- Fiche technique WS2812B (timings, niveaux logiques) :
  https://cdn-shop.adafruit.com/datasheets/WS2812B.pdf
- Carte ESP32-C3-DevKit-RUST-1, schéma et brochage :
  https://github.com/esp-rs/esp-rust-board
