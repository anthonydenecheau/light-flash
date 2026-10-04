# light-flash — Manuel utilisateur

Version du 2026-10-04. Ce manuel s'enrichira de deux chapitres : l'utilisation de l'interface
(chapitre 2) et la résolution des problèmes (chapitre 3).

> Stade actuel : la lampe est encore un prototype. La lumière est produite par la LED de la carte
> électronique ; le ruban de 144 LED sera raccordé ensuite. Les étapes ci-dessous resteront les
> mêmes.

## 1. Mise en service

### 1.1 Ce qu'il vous faut

- La lampe et son alimentation. Au stade prototype, un câble USB-C relié à un chargeur ou un
  ordinateur suffit. Avec le ruban, la lampe aura sa propre alimentation 5 V.
- Un réseau Wi-Fi de la maison en **2,4 GHz** (la lampe ne voit pas les réseaux 5 GHz) protégé en
  **WPA2**, ou mixte WPA2/WPA3. Sur la plupart des box, le réseau habituel convient.
- Pour la configuration par Bluetooth : un téléphone avec l'une de ces applications.
  - **Home Assistant** (iPhone et Android), gratuite, même sans installation de Home Assistant à
    la maison.
  - **Improv Wi-Fi** (Android).
  - Le site https://www.improv-wifi.com/ ouvert dans **Chrome** sur Android ou sur un ordinateur
    équipé du Bluetooth.
- À défaut de Bluetooth : n'importe quel téléphone ou ordinateur avec le Wi-Fi, pour la méthode de
  secours (§1.3).

### 1.2 Premier allumage

1. Branchez l'alimentation. La carte porte deux petits boutons : **RESET** et **BOOT**. Repérez
   **BOOT**, il servira à autoriser la configuration.
2. Au bout de deux secondes, la lampe s'allume en blanc chaud. Si elle a déjà servi, elle revient
   dans l'état où elle était avant d'être débranchée, allumée ou éteinte.
3. La lampe est prête à être configurée : elle est visible en Bluetooth sous le nom
   **light-flash** pendant cinq minutes après l'allumage (un appui bref sur **BOOT** la rend de
   nouveau visible à tout moment) et, tant qu'aucun Wi-Fi n'est enregistré, elle diffuse son
   propre réseau Wi-Fi de secours, lui aussi nommé **light-flash**.

### 1.3 Connexion au Wi-Fi de la maison

Deux méthodes, au choix. La première est la plus simple.

#### Par Bluetooth (recommandée)

1. Ouvrez l'application (Home Assistant, Improv Wi-Fi, ou improv-wifi.com dans Chrome). Dans
   Home Assistant, la lampe est proposée dans les notifications ou dans *Paramètres, Appareils et
   services, Ajouter une intégration, Improv*.
2. Choisissez **light-flash**. Si plusieurs lampes sont à portée, l'application peut demander à
   la lampe de se signaler : elle clignote alors en blanc pendant deux secondes.
3. L'application vous demande d'autoriser la configuration : **appuyez brièvement sur BOOT**.
   La lampe se met à **respirer en bleu** pendant une minute. Passé ce délai, il faut appuyer de
   nouveau.
4. Choisissez votre réseau Wi-Fi dans la liste que la lampe propose (les réseaux à portée, du
   plus fort au plus faible), saisissez son mot de passe, puis validez.
5. La lampe **clignote en bleu** pendant qu'elle se connecte, quelques secondes.
6. **Vert** : c'est réussi. Le réseau est mémorisé, et l'application vous propose d'ouvrir la page
   de la lampe. **Rouge** : le réseau est introuvable ou le mot de passe est faux ; la lampe
   revient en bleu respirant, corrigez et renvoyez sans rappuyer sur BOOT.

#### Par le réseau de secours

1. Sur votre téléphone ou ordinateur, connectez-vous au réseau Wi-Fi **light-flash**, mot de
   passe **light-flash**.
2. Ouvrez http://192.168.71.1/ dans le navigateur. Si rien ne s'affiche après quelques secondes,
   vérifiez que vous êtes bien sur le réseau *light-flash* et retapez l'adresse.
3. En bas de la page, dans **Réseau Wi-Fi**, saisissez le nom du réseau de la maison et son mot
   de passe, puis **Enregistrer et redémarrer**.
4. La lampe redémarre et rejoint le réseau de la maison ; son réseau de secours disparaît.
   Reconnectez votre téléphone au Wi-Fi de la maison.

### 1.4 Retrouver la lampe sur le réseau de la maison

Depuis un appareil connecté au même Wi-Fi, ouvrez l'une de ces adresses dans le navigateur, dans
cet ordre jusqu'à ce que l'une réponde :

1. **http://light-flash.local/** : iPhone, iPad, Mac, Windows, Linux, et une partie des Android.
2. **http://light-flash/** : fonctionne avec beaucoup de box, dont la Livebox, qui accepte aussi
   **http://light-flash.home/**.
3. **L'adresse numérique** de la lampe, par exemple http://192.168.1.37/. Elle est affichée par
   l'application Bluetooth à la fin de la configuration, et visible dans la liste des appareils de
   votre box. Pour qu'elle ne change jamais, réservez-la dans la box (chapitre 3 à venir).

Une fois la page ouverte, ajoutez-la à l'écran d'accueil du téléphone : *Partager, Ajouter à
l'écran d'accueil* sur iPhone, menu *⋮, Ajouter à l'écran d'accueil* dans Chrome.

### 1.5 Ce que disent les couleurs

| La lampe… | Signification |
|---|---|
| s'allume en blanc chaud ou dans sa dernière couleur | fonctionnement normal |
| respire en bleu | configuration autorisée, en attente du réseau et du mot de passe (une minute) |
| clignote en bleu | connexion au réseau en cours |
| passe au vert deux secondes | réseau enregistré |
| passe au rouge deux secondes | réseau introuvable ou mot de passe refusé |
| clignote en blanc deux secondes | elle se signale à la demande de l'application |

### 1.6 Les boutons de la carte

| Action | Effet |
|---|---|
| **BOOT**, appui bref | rend la lampe visible en Bluetooth pendant cinq minutes et autorise la configuration pendant une minute |
| **BOOT**, appui long de 5 secondes | efface le réseau Wi-Fi et les réglages enregistrés, puis redémarre (la lampe passe au rouge un instant) |
| **RESET** | redémarre la lampe sans rien effacer |

### 1.7 Et si le Wi-Fi tombe ?

La lampe continue d'éclairer : le Wi-Fi ne sert qu'au pilotage.

- Si la box redémarre ou le réseau disparaît, la lampe se reconnecte toute seule dès qu'il revient,
  en quelques secondes.
- Après une minute et demie sans réseau, elle rouvre son réseau de secours **light-flash** pour
  rester pilotable, et retente le réseau de la maison toutes les deux minutes.
- Après une coupure de courant, elle revient dans l'état où elle était, allumée ou éteinte.

### 1.8 Changer de réseau Wi-Fi

Trois possibilités :

- Depuis la page de la lampe, section **Réseau Wi-Fi** : saisir le nouveau réseau, la lampe
  redémarre dessus.
- Par Bluetooth : appui bref sur **BOOT**, puis la procédure du §1.3 avec le nouveau réseau.
- Appui long de 5 secondes sur **BOOT** pour tout effacer, puis reprendre au §1.2.

## 2. Utilisation de l'interface

À venir.

## 3. Résolution des problèmes

À venir.
