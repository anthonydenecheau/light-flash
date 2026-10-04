# light-flash — Manuel utilisateur

Version du 2026-10-04. Ce manuel s'enrichira d'un chapitre de résolution des problèmes
(chapitre 3).

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
3. La lampe est prête à être configurée : elle est visible en Bluetooth pendant cinq minutes
   après l'allumage (un appui bref sur **BOOT** la rend de nouveau visible à tout moment) et, tant
   qu'aucun Wi-Fi n'est enregistré, elle diffuse son propre réseau Wi-Fi de secours. Dans les
   deux cas elle porte son nom : **light-flash-** suivi de quatre caractères propres à chaque
   lampe (par exemple `light-flash-df60`) tant que vous ne l'avez pas renommée, puis le nom que
   vous lui donnez (chapitre 2).

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

1. Sur votre téléphone ou ordinateur, connectez-vous au réseau Wi-Fi portant le nom de la lampe
   (`light-flash-xxxx`), mot de passe **light-flash**.
2. Ouvrez http://192.168.71.1/ dans le navigateur. Si rien ne s'affiche après quelques secondes,
   vérifiez que vous êtes bien sur le réseau *light-flash* et retapez l'adresse.
3. En bas de la page, dans **Réseau Wi-Fi**, saisissez le nom du réseau de la maison et son mot
   de passe, puis **Enregistrer et redémarrer**.
4. La lampe redémarre et rejoint le réseau de la maison ; son réseau de secours disparaît.
   Reconnectez votre téléphone au Wi-Fi de la maison.

### 1.4 Retrouver la lampe sur le réseau de la maison

Depuis un appareil connecté au même Wi-Fi, ouvrez l'une de ces adresses dans le navigateur, dans
cet ordre jusqu'à ce que l'une réponde :

1. **http://light-flash-xxxx.local/** (ou **http://salon-1.local/** une fois la lampe renommée
   « Salon 1 ») : iPhone, iPad, Mac, Windows, Linux, et une partie des Android.
2. **http://light-flash-xxxx/** : fonctionne avec beaucoup de box, dont la Livebox, qui accepte
   aussi **http://light-flash-xxxx.home/**.
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
- Après une minute et demie sans réseau, elle rouvre son réseau de secours (à son nom) pour
  rester pilotable, et retente le réseau de la maison toutes les deux minutes.
- Après une coupure de courant, elle revient dans l'état où elle était, allumée ou éteinte.

### 1.8 Changer de réseau Wi-Fi

Trois possibilités :

- Depuis la page de la lampe, section **Réseau Wi-Fi** : saisir le nouveau réseau, la lampe
  redémarre dessus.
- Par Bluetooth : appui bref sur **BOOT**, puis la procédure du §1.3 avec le nouveau réseau.
- Appui long de 5 secondes sur **BOOT** pour tout effacer, puis reprendre au §1.2.

## 2. Utilisation de l'interface

La page de la lampe s'ouvre dans n'importe quel navigateur, sur téléphone comme sur ordinateur,
sans application à installer. Elle s'adapte au mode clair ou sombre de l'appareil et peut être
ajoutée à l'écran d'accueil pour s'ouvrir comme une application (§1.4).

### 2.1 En-tête

Le logo, le nom de la lampe et sa situation : point vert « En ligne sur <réseau> », point orange
« Point d'accès de secours », point gris « Connexion au Wi-Fi… » ou « Lampe injoignable » si la
page ne reçoit plus de réponse. À droite, deux boutons ronds ouvrent des fenêtres qui se
referment d'un toucher sur la croix ou en dehors :

- **Les barres de signal** (ou la ligne de situation) ouvrent la fenêtre *Réseau* (§2.3). Quatre
  barres vertes : excellent signal ; aucune : lampe sur son réseau de secours ou signal inconnu.
- **La flèche** ouvre la fenêtre *Mises à jour* (§2.6). Une pastille colorée sur le bouton
  signale qu'une nouvelle version est disponible ; la flèche tourne pendant une vérification ou
  une installation.

### 2.2 Lumière

- **Allumer / Éteindre** : le grand bouton. Coloré quand la lampe est allumée, gris sinon.
- **Couleur** : douze pastilles (deux blancs chauds, blanc, blanc froid, et huit couleurs) et une
  roue pour choisir n'importe quelle couleur. Choisir une couleur allume la lampe.
- **Luminosité** : le curseur, de 1 à 100 %. La lampe suit le doigt en temps réel.
- **Effet** : *Uni* (couleur fixe), *Respiration* (la luminosité monte et descend lentement),
  *Arc-en-ciel* (les couleurs défilent le long du ruban).

Tout ce qui est réglé est mémorisé par la lampe deux secondes plus tard : elle le retrouve après
une coupure de courant. Une autre personne qui ouvre la page voit les mêmes réglages, mis à jour
toutes les quelques secondes.

### 2.3 Réseau

Fenêtre ouverte depuis l'en-tête (§2.1) : le réseau Wi-Fi actuel, la qualité du signal (quatre
barres = excellent), l'adresse de la lampe, ses adresses faciles à retenir (`nom.local`, `nom`)
et les autres lampes découvertes (§2.5).

- **Changer de réseau Wi-Fi** : déplier, saisir le nouveau réseau et son mot de passe,
  *Enregistrer et redémarrer*. La lampe redémarre sur le nouveau réseau. Si elle ne le trouve pas,
  elle rouvre son réseau de secours au bout d'une minute et demie (§1.7).
- **Oublier le Wi-Fi…** : efface le réseau enregistré ; la lampe redémarre sur son réseau de
  secours, reprendre au §1.3. Une confirmation est demandée.

### 2.4 Lampe

- **Nom de la lampe** : par exemple « Salon 1 ». *Renommer et redémarrer* applique le nom
  partout : en Bluetooth, sur le réseau de secours, et dans les adresses, qui deviennent
  `salon-1.local` et `salon-1` (accents et espaces sont adaptés automatiquement). C'est ce qui
  permet de distinguer plusieurs lampes dans la maison.
- **Rendre visible en Bluetooth** : équivaut à l'appui bref sur **BOOT** : la lampe est visible
  cinq minutes et accepte une configuration pendant une minute. Utile quand le bouton n'est pas
  accessible. Le bouton indique « Bluetooth actif » tant que c'est le cas.
- **Redémarrer…** : redémarre la lampe sans rien effacer, après confirmation.
- La dernière ligne rappelle le modèle, le nombre de LED et la limite de courant, la durée depuis
  le dernier démarrage et la mémoire libre.

### 2.5 Plusieurs lampes

Chaque lampe a sa page et ses réglages. Donnez un nom différent à chacune (§2.4). Les lampes se
découvrent entre elles sur le réseau de la maison : la carte *Réseau* affiche les autres lampes
(« Autres lampes »), chacune cliquable pour ouvrir sa page.

Dès qu'une autre lampe est connue, un sélecteur apparaît en haut de la carte *Lumière* :

- **Cette lampe** : les réglages ne concernent que la lampe dont la page est ouverte.
- **Toutes les lampes** : chaque réglage (allumer, couleur, luminosité, effet) est envoyé en même
  temps à toutes les lampes découvertes. Si l'une ne répond pas, un message le signale.

La découverte prend jusqu'à une minute après l'allumage d'une lampe. Avec « Toutes les lampes »,
toucher une scène (§2.7) l'applique aussi à toutes ; les horaires (§2.8), eux, se règlent sur
chaque lampe. Pour un pilotage à la voix, utilisez un groupe de lumières dans Home Assistant.

### 2.6 Mettre à jour la lampe

Les nouvelles versions du logiciel de la lampe se distribuent depuis un petit serveur de fichiers
de la maison (un ordinateur ou un NAS), sans câble. Une seule fois, ouvrez la fenêtre *Mises à
jour* (la flèche de l'en-tête, §2.1), saisissez l'adresse de ce serveur, par exemple
`http://192.168.1.30:8000`, et *Enregistrer* : la lampe vérifie aussitôt.

La fenêtre indique la version installée et, dans un encadré, où en est la lampe : « La lampe est à
jour », « Mise à jour x.y.z disponible », « Vérification impossible » avec la raison (serveur
éteint, adresse erronée) ou « Aucun serveur de mises à jour ». Ensuite, la lampe vérifie toute
seule au démarrage puis toutes les six heures, et à chaque fois que vous appuyez sur *Vérifier
maintenant* ; au bout de quelques secondes un message annonce le résultat, y compris quand il n'y
a rien de nouveau : « Aucune mise à jour : la lampe est à jour ». Quand une version plus récente
est publiée, une pastille apparaît sur la flèche de l'en-tête et un bandeau coloré en haut de la
page : « Mise à jour x.y.z disponible », avec le descriptif. Appuyez sur **Mettre à jour**, confirmez : la barre de progression suit le
téléchargement, puis la lampe vérifie l'image reçue et redémarre. Comptez une minute ; la lampe
continue d'éclairer pendant le téléchargement et s'éteint le temps du redémarrage. La page se
reconnecte d'elle-même et affiche la nouvelle version.

Si quelque chose se passe mal pendant la mise à jour (coupure du Wi-Fi, fichier abîmé), rien
n'est perdu : la lampe garde la version précédente et le bandeau propose de réessayer. Si une
nouvelle version ne parvient pas à démarrer, la lampe revient d'elle-même à l'ancienne.

### 2.7 Scènes

Une scène est un réglage complet, couleur, luminosité et effet, mémorisé sous un nom et rappelé
d'un geste. La lampe en propose quatre au départ : *Lecture*, *Soirée*, *Veilleuse* et *Fête*.
Dans la carte *Scènes* :

- **Toucher une scène** l'applique et allume la lampe. Avec « Toutes les lampes » (§2.5), elle
  est envoyée à toutes les lampes découvertes.
- **Créer une scène** : réglez la lumière à votre goût dans la carte *Lumière*, saisissez un nom
  (24 caractères au plus) et touchez *Mémoriser*. Un nom déjà utilisé remplace la scène
  existante, ce qui permet de la retoucher. Huit scènes au maximum.
- **Supprimer** : la croix à droite du nom, après confirmation. Une scène utilisée par un
  horaire ne peut pas être supprimée tant que l'horaire existe.

Les scènes sont propres à chaque lampe et conservées quand elle est débranchée.

### 2.8 Programmation

La carte *Programmation* regroupe la minuterie, les horaires et le fuseau horaire.

**Minuterie d'extinction.** *15 min*, *30 min* ou *1 h* : la lampe s'éteint d'elle-même à
l'échéance, le temps restant s'affiche en face de « Minuterie d'extinction », *Annuler* l'arrête.
La minuterie est oubliée si la lampe est débranchée.

**Horaires.** La lampe peut, à heure fixe et certains jours de la semaine, s'allumer, s'éteindre
ou appliquer une scène : par exemple *Lecture* en semaine à 7 h 30 et extinction tous les jours à
23 h. Dépliez *Ajouter un horaire*, touchez les jours voulus (en couleur quand ils sont
sélectionnés), choisissez l'heure et l'action, puis *Ajouter*. Dans la liste, la case à gauche
suspend un horaire sans le supprimer, la croix le supprime. Huit horaires au maximum.

Les horaires ont besoin de l'heure. La lampe la reçoit d'Internet par le Wi-Fi de la maison dans
la minute qui suit sa connexion ; l'heure courante s'affiche en face de « Horaires ». Si vous
lisez « heure non reçue », la lampe n'a pas accès à Internet (point d'accès de secours, box
coupée) : les horaires ne se déclenchent pas, la minuterie fonctionne toujours. Un horaire
survenu pendant que la lampe était débranchée n'est pas rattrapé au rallumage.

**Fuseau horaire.** Réglé d'avance sur la France métropolitaine, changement d'heure compris.
Ailleurs : dépliez *Fuseau horaire*, choisissez la région puis *Enregistrer*. Pour un fuseau
absent de la liste, saisissez sa chaîne POSIX, que l'on trouve par exemple dans
https://github.com/nayarsystems/posix_tz_db/blob/master/zones.csv (colonne de droite).

## 3. Résolution des problèmes

À venir.
