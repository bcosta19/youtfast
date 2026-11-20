# Guide d'utilisation de YouTFast

[Retour à la présentation](README.md) · [Installer YouTFast](install.md) · [Détails des plateformes](../../distribution.md)

[English](../../user-guide.md) · [Português (Brasil)](../pt-BR/user-guide.md) · [Español](../es/user-guide.md) ·
**Français** · [Deutsch](../de/user-guide.md) · [Italiano](../it/user-guide.md)

*Ceci est une traduction du [document original en anglais](../../user-guide.md) ; en cas de divergence, la version anglaise fait foi.*

## Prérequis

| Quoi | Pourquoi | Arch | Debian/Ubuntu |
|---|---|---|---|
| libmpv 0.42+ | Lecture, liée à la compilation | `mpv>=0.42.0` | `libmpv-dev` (vérifiez la version) |
| yt-dlp | Les flux que lit mpv et les nouvelles tentatives de lecture | `yt-dlp` | `yt-dlp` (gardez-le à jour) |
| Deno / EJS de yt-dlp correspondant | Défis d'extraction de YouTube | `deno` et le paquet EJS de la distribution | Voir la [configuration de yt-dlp](https://github.com/yt-dlp/yt-dlp/wiki/EJS) |
| Rust 1.98 | Fixé dans `rust-toolchain.toml` | `rustup` | `rustup` |
| python-secretstorage | Facultatif : connexion avec la session d'un navigateur basé sur Chromium | `python-secretstorage` | `python3-secretstorage` |

mpv 0.42 ou plus récent doit être compilé avec Lua (c'est le cas des paquets
habituels) : il résout les liens YouTube via son script `ytdl_hook` et attend
que les flux de yt-dlp deviennent disponibles. Les anciens hooks peuvent les
ouvrir trop tôt et recevoir une erreur HTTP 403. Gardez yt-dlp et son
environnement d'exécution JS dans le `PATH`. Vérifiez la version du **paquet
libmpv** installé, et non la version de son API client ni celle d'un exécutable
mpv installé séparément. Si votre distribution ne fournit qu'une libmpv plus
ancienne, mettez-la à jour depuis une source de confiance ou utilisez une
version Flatpak contenant mpv 0.42+.
L'import depuis le navigateur nécessite aussi `python3` avec le module `yt_dlp` ;
une installation de yt-dlp sous forme d'exécutable seul ne fournit pas ce module
Python.

## Lancement

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"   # open a link
cargo run --locked --release -- "rust egui"                      # or search
```

| Touche | Action |
|---|---|
| Espace, K | Lecture ou pause |
| ←, → | Reculer ou avancer de 5 s |
| J, L | Reculer ou avancer de 10 s |
| Ctrl+← / → (Option sur macOS) | Chapitre précédent / suivant |
| Origine, Fin ; 0–9 | Début / fin ; position par dizaines de pour cent |
| ↑, ↓ ; M | Défilement vertical de la page hors du lecteur ; volume ±5 % dans le lecteur ; couper le son (les Shorts utilisent ↑/↓ pour changer de vidéo) |
| C ; <, > ; virgule, point | Sous-titres ; vitesse ; image précédente / suivante en pause |
| Maj+N, Maj+P | Suivant / précédent dans la source de lecture |
| I ; T ; Maj+A | Mini-lecteur ; mode cinéma ; mode audio seul |
| F, double-clic | Plein écran |
| / ; ? | Activer la recherche ; ouvrir l'aide des raccourcis |
| Alt+← / → ; boutons précédent / suivant de la souris | Naviguer en arrière / en avant (touches Alt sous Linux/Windows) |
| Ctrl/Cmd+R | Actualiser la page actuelle |
| Échap | Fermer un menu, l'aide ou un compte à rebours, quitter le plein écran ou revenir en arrière |

Les champs de texte et les menus conservent leurs propres touches. Les boutons
ayant le focus gèrent Espace, et un curseur de volume ayant le focus conserve
ses touches fléchées. Hors du lecteur, ↑/↓ font défiler verticalement la page
active et J/K déplacent la sélection d'élément (`g`/`G` vont en haut ou en
bas). Maintenez le clic sur la vidéo pour la lire temporairement à 2× ;
relâcher rétablit la vitesse précédente sans modifier la vitesse enregistrée. La
vitesse choisie avec `<`/`>` ou dans le menu **Vitesse de lecture** du lecteur
est mémorisée entre les vidéos et les redémarrages ; activez **Mémoriser pour
cette chaîne** dans ce menu pour enregistrer une vitesse pour la chaîne en
cours (la vitesse de la chaîne prévaut sur la vitesse globale, jusqu'à 500
chaînes, et peut être effacée dans **Paramètres → Lecture → Vitesses de lecture
par chaîne**). Les directs au bord du direct restent à 1× ; si un direct est en
retrait par rapport au bord, les commandes de vitesse et l'appui long 2× restent
disponibles pour le rattraper et reviennent à 1× au bord du direct, sans écraser
les vitesses enregistrées. Le menu du lecteur propose aussi la lecture en
boucle, la taille et l'arrière-plan des sous-titres, la copie d'un lien avec le
minutage et l'ouverture de la vidéo dans un navigateur.

La recherche utilise directement l'interface InnerTube de YouTube, avec des
résultats séparés pour les vidéos, les chaînes et les playlists. Les filtres
vidéo portent sur la date, la durée, le direct et le tri.
Précédent/suivant restaure la page, la requête, les filtres, la sélection et la
position de défilement. Les vignettes comportent des liens vers la chaîne, un
menu et des actions rapides À regarder plus tard/file d'attente. La file
d'attente peut être réordonnée ou réduite ; le mini-lecteur est une petite
fenêtre vidéo 16:9 sans bordure dans le coin inférieur droit (cliquez pour
revenir à la page de la vidéo, ou survolez-la pour afficher les commandes
lecture/pause et fermer). La lecture d'une playlist continue au-delà de la page
chargée initialement. L'historique de l'appareil permet de filtrer et de
supprimer des entrées individuelles.

Dans une fenêtre large, Paramètres affiche ses groupes dans un index à côté
d'eux : cliquez sur un groupe pour y faire défiler la page. Une ligne avec un
interrupteur peut être cliquée n'importe où, pas seulement sur l'interrupteur.

Paramètres → Volet latéral choisit quels boutons du volet latéral sont affichés
et leur ordre (l'interrupteur pour masquer, les flèches pour déplacer, Rétablir
les valeurs par défaut pour annuler). Compte et Paramètres ne peuvent pas être
masqués. Vidéos « J'aime » et Playlists n'apparaissent toujours que lorsque vous
êtes connecté. Les touches numériques 1 à 9 ouvrent les boutons dans l'ordre où
le volet latéral les affiche.

Paramètres → Lecture → Mini-lecteur pendant le défilement garde une vidéo
visible dans le mini-lecteur en coin lorsque vous la dépassez en faisant
défiler la page pour lire les commentaires. Cette option est désactivée par
défaut. Revenir en haut fait réapparaître le lecteur intégré ; cliquer sur le
mini-lecteur ramène à la vidéo en haut de la page. Fermer le mini-lecteur met la
lecture en pause.

Paramètres → Lecture → Tampon vidéo permet de choisir entre le cache de
démultiplexage standard de mpv (**Grand — 150 Mio / 50 Mio**, par défaut, qui
met plusieurs minutes en mémoire tampon pour les connexions instables et la
navigation instantanée) et **Petit — 50 Mio / 20 Mio** (pour maintenir
l'utilisation de la RAM autour de ~200 Mo et consommer moins de données en
changeant vite de vidéo).
Le mode audio seul (`Shift+A` ou le bouton casque dans les commandes du lecteur)
diffuse et décode
uniquement la piste audio (`ba/b` et `vid=no`) de la vidéo actuelle, et revient
à la vidéo standard à l'ouverture d'une autre vidéo ou au redémarrage de
l'application.

La page de la vidéo affiche aussi la description (les horodatages permettent de
se déplacer), les chapitres et les commentaires, chargés via InnerTube. Les
commentaires et les réponses affichent aussi les pièces jointes images et GIF/WebP
animés. Les pièces jointes utilisent le cache de vignettes à taille limitée,
sont décodées en arrière-plan et ne s'animent que lorsqu'elles sont visibles.
Les pages de chaîne ont des onglets — vidéos,
Shorts, direct, playlists et à propos — ainsi qu'une recherche dans les vidéos
de la chaîne.

Les pages de chaîne incluent une bannière et le tri des vidéos. Les playlists
affichent leur couverture, leurs métadonnées et l'élément en cours de lecture.
Les Shorts utilisent un affichage en portrait avec des commandes
suivant/précédent et la navigation à la molette. L'interface démarre en anglais.
Choisissez English, Português (Brasil), Español, Français, Deutsch ou Italiano
dans Paramètres → Apparence → Langue ; le changement s'applique immédiatement et
est conservé d'une session à l'autre, y compris pour les notifications sur le
bureau. Les anciens paramètres utilisent aussi l'anglais par défaut. Les titres
des vidéos, les commentaires et les préférences de sous-titres conservent leurs
propres langues.

La page de la vidéo peut afficher une transcription synchronisée dans laquelle
on peut chercher, ainsi que des aperçus en vignettes sur la barre de
progression, lorsque YouTube fournit les métadonnées publiques nécessaires. Ces
requêtes facultatives démarrent à la demande, fonctionnent sans compte et
utilisent des téléchargements limités. L'absence de sous-titres ou de
storyboards, ou une restriction en amont, peut rendre la fonctionnalité
indisponible ; la lecture continue de passer par libmpv.

Pour des aperçus synthétiques de l'interface, compilez avec `--features demo` et lancez :

```sh
cargo run --locked --features demo -- --demo --demo-view watch --demo-size 1280x800
cargo run --locked --features demo -- --demo --demo-view home --demo-light
cargo run --locked --features demo -- --demo --demo-view settings --demo-language pt-BR
cargo run --locked --features demo -- --demo --demo-view theater --demo-maximized
```

Le mode démo ouvre une fenêtre isolée sans comptes, fichiers utilisateur,
réseau, lecture ni enregistrement auprès des contrôles multimédias du bureau. Il
affiche des états prédéfinis, pas chaque interaction avec le backend.
`--demo-capture target/example.png` capture cette fenêtre puis la ferme.
`--demo-maximized` démarre l'aperçu en fenêtre agrandie. Dans la vue de
lecture, `--demo-aspect 4:3` (ou `16:9` ou `21:9`) étire l'image synthétique
vers un autre format ; `9:16` utilise l'affichage de repli en portrait. Les vues
`watch`, `theater` et `fullscreen` montrent ces dispositions. Les raccourcis de
lecture suivent
[les raccourcis publiés par YouTube](https://support.google.com/youtube/answer/7631406),
avec les adaptations pour les listes et les Shorts décrites ci-dessus.

Les touches et panneaux multimédias du bureau (MPRIS, donc `playerctl` fonctionne
aussi) contrôlent la lecture, et un second lancement transmet un lien ou une
recherche à la fenêtre déjà ouverte au lieu d'en ouvrir une autre.

## Diffuser sur la TV

Le sélecteur de TV prend en charge trois méthodes. Les TV YouTube compatibles
DIAL/Lounge utilisent la connexion réseau native et ne nécessitent aucun outil
auxiliaire. Cela inclut les modèles Samsung, LG, Roku, Fire TV et Android/Google
TV qui exposent YouTube via DIAL ; lorsqu'une TV propose à la fois DIAL et
Google Cast, le sélecteur conserve les deux options de connexion. Les adresses
IP locales permettent de distinguer des TV portant le même nom. Tout appareil
dont l'application YouTube affiche **Associer avec un code TV** (Apple TV,
consoles de jeu, TV sur un autre réseau ou derrière l'isolation d'un réseau
invité) peut plutôt être associé avec ce code. La prise en charge de Google Cast
est facultative et utilise
[`catt`](https://github.com/skorokithakis/catt) installé séparément dans le `PATH`.
Par exemple, si vous utilisez déjà pipx, installez-le avec `pipx install catt`.
Suivez les instructions d'installation de catt pour votre système
d'exploitation, vérifiez `catt --version` dans l'environnement qui lance
YouTFast, puis redémarrez YouTFast si vous avez modifié le `PATH`. YouTFast
n'installe jamais l'outil auxiliaire automatiquement.
L'outil et ses dépendances Python ne sont pas inclus dans les archives publiées.
Le Flatpak ne peut pas utiliser un outil arbitraire de l'hôte ; cette
intégration n'est pas encore validée pour le paquet Flatpak.

1. Allumez la TV ou le Chromecast et connectez-le au réseau local de l'ordinateur.
   L'isolation d'un réseau invité et le blocage du multicast peuvent empêcher la
   découverte ; la recherche est envoyée sur chaque interface réseau privée,
   donc les VPN et les ponts de conteneurs ne masquent pas le réseau de la TV.
   Sous Windows, le pare-feu peut bloquer les réponses des TV à une recherche
   sur tout le réseau : autorisez YouTFast lorsque Windows le demande, ou
   utilisez les options par IP ou par code TV ci-dessous. Cette version accepte
   les adresses IP privées/lien-local. Google Cast utilise le port 8009 ;
   YouTube DIAL utilise le point d'accès annoncé par la TV.
2. Ouvrez une vidéo YouTube dans YouTFast et cliquez sur **Diffuser sur la TV**
   à côté de **Audio seul** dans les commandes du lecteur, ou utilisez le bouton
   de la barre supérieure.
   La découverte ne s'exécute que lorsque vous ouvrez le sélecteur pour la
   première fois ou appuyez sur **Rechercher des TV** ; il n'y a pas d'analyse du
   réseau en arrière-plan.
   Les TV YouTube sont découvertes via SSDP et des points d'accès DIAL vérifiés ;
   les annonces AirPlay et Google Cast sont aussi examinées pour vérifier la
   prise en charge de YouTube DIAL. Si une TV n'apparaît pas, saisissez son IP
   sous **Rechercher une TV YouTube par IP** et appuyez sur **Rechercher**. La
   recherche manuelle demande à cette adresse son point d'accès DIAL, puis
   essaie les chemins courants sur les ports 8080, 8008 et 8060.
   Aucune application n'est lancée pendant la découverte ou la recherche par IP.
   Pour associer avec un code, ouvrez YouTube sur la TV, puis **Paramètres →
   Associer avec un code TV**, saisissez le code à 12 chiffres sous **Associer
   une TV avec un code** et appuyez sur **Associer la TV**. Le code n'est
   utilisé qu'une fois et n'est pas conservé ; la TV associée est jointe via les
   serveurs de YouTube et reste dans la liste jusqu'à la fermeture de YouTFast.
3. Choisissez l'appareil. YouTFast envoie le lien canonique de la vidéo et la
   position locale arrondie à la seconde inférieure. Les vidéos en direct
   demandent la position du direct.
   Le lecteur local continue la lecture jusqu'à ce que la TV signale que la
   vidéo demandée est en cours de lecture, en attendant jusqu'à une minute pour
   les publicités et le sélecteur de compte de la TV. Avec DIAL, choisir la TV
   rejoint son application YouTube si elle est déjà ouverte, ou l'ouvre, et
   établit une connexion Lounge en mémoire ; une connexion inactive est
   renouvelée automatiquement. Si la TV vous demande de vous connecter, de
   choisir un compte ou de regarder en tant qu'invité, faites votre choix sur la
   TV ; YouTFast ne gère jamais cette connexion.
   Une confirmation tardive ne met jamais en pause une autre vidéo locale, y
   compris une lecture nouvellement ouverte de la même vidéo.
4. Utilisez les commandes TV explicites du panneau : lecture, pause,
   reculer/avancer de dix secondes, baisser/monter le volume de dix points de
   pourcentage, et **Arrêter sur la TV**. Les commandes sont envoyées une par
   une ; le panneau ne synchronise pas en continu l'état de la TV.
5. Fermer le panneau ne fait que le masquer. Rouvrez-le depuis la barre
   supérieure pour contrôler le même appareil. **Déconnecter sans arrêter la
   TV**, ou quitter YouTFast, laisse la lecture se poursuivre sur la TV et
   abandonne la session de contrôle en mémoire. **Arrêter sur la TV** demande à
   un récepteur Cast de s'arrêter, ou ferme l'application YouTube DIAL ; une TV
   qui ne permet pas de la fermer, ou une TV associée avec un code, arrête la
   vidéo. Aucune de ces actions ne relance automatiquement le lecteur local.

Le panneau est une télécommande séparée : les commandes du lecteur local, les
raccourcis clavier et les touches multimédias du bureau continuent de contrôler
l'ordinateur. Le point sur le bouton Cast signifie qu'un appareil est retenu,
pas que la lecture est toujours en cours. Une confirmation initiale réussie
appartient au passé ; les changements côté TV ne sont pas interrogés. Un délai
dépassé ou une erreur peut survenir après le démarrage sur la TV : vérifiez la
TV avant de renvoyer. Le panneau conserve un appareil dont l'état est incertain
pour que vous puissiez quand même l'arrêter ou vous déconnecter.

Les appareils Google Cast sont le Chromecast, le Google TV Streamer, ainsi que
les TV Google TV, Android TV, LG, Samsung et autres compatibles avec Cast
activé. La prise en charge dépend du modèle exact et du micrologiciel, pas de la
marque. Les enceintes Google connues (Home, Nest Mini, Nest Audio, Chromecast
Audio) sont exclues de la liste ; d'autres appareils Cast uniquement audio
peuvent encore apparaître et ne sont pas des cibles vidéo. Une TV qui ne
communique pas son volume (par exemple, avec le son sur un récepteur HDMI-ARC)
garde le volume sur sa propre télécommande.
Les appareils DIAL nécessitent une application YouTube fonctionnelle exposant un
identifiant d'écran et l'accès à YouTube Lounge. AirPlay ne sert que d'indice de
découverte ; aucun média n'est envoyé via AirPlay, Miracast ou DLNA. La
validation sous Windows/macOS natifs et en Flatpak est en attente. L'API Lounge
n'est pas documentée et peut changer indépendamment de YouTFast ; ses références
de protocole sont les projets ytcast et casttube.

L'application YouTube et le compte de la TV gèrent la lecture, y compris les
publicités et les restrictions d'accès. YouTFast ne transfère pas sa session de
compte, les cookies du navigateur ni la bibliothèque locale. Avec **Ignorer les
sponsors (SponsorBlock)** activé, YouTFast vérifie aussi la position de la vidéo
envoyée sur la TV et envoie des déplacements à distance via DIAL/Lounge ou
Google Cast (`catt`). Il utilise les mêmes catégories automatiques que la
lecture locale : sponsors, autopromotion et rappels d'interaction. Laissez
YouTFast ouvert et connecté ; se déconnecter ou désactiver SponsorBlock arrête
les déplacements automatiques suivants.
Cela s'applique à la vidéo explicitement envoyée, pas aux vidéos suivantes
choisies par la lecture automatique de la TV. Les directs, la lecture en pause
et l'absence de données de position ne donnent lieu à aucun saut. Les
vérifications ont lieu environ toutes les deux secondes à proximité d'un segment
et jusqu'à dix secondes d'intervalle lorsque le suivant est loin.
L'interrogation de la position de la TV et le démarrage de l'outil auxiliaire
peuvent retarder un saut ; les deux dernières secondes d'un segment ne sont pas
touchées. Les événements publicitaires de Lounge suspendent les sauts de
sponsors ; `catt` n'expose pas d'indicateur de publicité séparé et fiable, donc
Cast s'appuie sur l'état de lecture du récepteur et sa capacité de déplacement.
Cela ne bloque pas les publicités YouTube.
La validation sur des TV/Chromecast physiques et sur les plateformes natives
reste en attente.

Les paramètres locaux de sous-titres, la lecture automatique et la file
d'attente locales et les règles de synchronisation de l'historique du compte ne
sont pas transférés à la TV ;
YouTube sur la TV peut enregistrer son propre historique. Les résultats de
découverte et la TV sélectionnée ne sont conservés qu'en mémoire. La sortie
brute et les erreurs de l'outil auxiliaire ne sont jamais affichées ni
journalisées, y compris avec la journalisation détaillée.

## Votre bibliothèque, avec ou sans compte

**Accueil** affiche les nouveautés de vos abonnements — le flux du compte ou les
chaînes RSS de l'appareil — regroupées par jour et paginées : faites défiler
jusqu'à la fin pour en charger davantage. Les Shorts se trouvent sur une étagère
au-dessus de la liste : ceux du compte sont marqués par YouTube, ceux des
chaînes de l'appareil proviennent de leurs onglets Shorts.

**Sans compte**, YouTFast conserve les abonnements, l'historique de visionnage
(avec l'endroit où chaque vidéo a été laissée, pour la reprendre) et À regarder
plus tard sur cet ordinateur, dans `~/.local/share/youtfast/library.json`, et
ses paramètres (qualité vidéo, volume, vitesse de lecture) dans `~/.config/youtfast/settings.json`.
Le flux d'accueil provient du RSS de chaque chaîne suivie. Importez une
bibliothèque existante depuis
[Google Takeout](https://takeout.google.com) : choisissez YouTube et YouTube
Music, avec JSON comme format d'historique, et importez `subscriptions.csv` et
`watch-history.json`. Rien de tout cela ne touche à un compte Google. Les
imports acceptent des fichiers jusqu'à 64 Mio et 20 000 abonnements uniques, et
conservent les 1 000 entrées d'historique uniques les plus récentes. L'analyse
et la fusion s'effectuent en arrière-plan.
Si la bibliothèque change pendant la fusion, relancez l'import ; ces
modifications plus récentes ne sont jamais remplacées par un instantané
d'import plus ancien.

Dans **Paramètres → Historique**, **Sauvegarder la bibliothèque** exporte vos
abonnements de l'appareil, l'historique de visionnage (avec les positions de
reprise) et À regarder plus tard dans un fichier JSON versionné
(`youtfast-library.json`) via **Exporter la bibliothèque (JSON)**, ou fusionne
une sauvegarde via **Importer une sauvegarde** selon les mêmes règles de fusion
en arrière-plan (limite de 64 Mio, jusqu'à 20 000 abonnements uniques, les
1 000 entrées d'historique les plus récentes et 5 000 vidéos À regarder plus
tard). Les comptes et sessions ne sont jamais exportés. **Exporter les
abonnements** enregistre vos abonnements de l'appareil au format OPML 2.0
(`youtfast-subscriptions.opml`) avec l'URL du flux RSS YouTube de chaque chaîne
via **Exporter en OPML**.

**History sync** (synchronisation de l'historique) dans Paramètres → Historique
s'applique lorsque vous êtes connecté :

- **Separate** (séparé, par défaut) garde séparées les listes Compte et Cet
  appareil.
- **Merged in YouTFast** (fusionné dans YouTFast) ouvre Historique sur **Tout**,
  en combinant les deux listes sans vidéos en double. **Compte** et **Cet
  appareil** restent disponibles comme filtres.
  La recherche filtre la liste combinée ; les positions de reprise locales et
  les mentions « regardée » ont la priorité. Rien n'est envoyé à YouTube.
- **Merged and sent to YouTube** (fusionné et envoyé à YouTube) signale aussi
  les vidéos dont la lecture démarre réellement ici, après deux secondes de
  lecture active, au compte actif au début de la lecture. Les Shorts passés
  rapidement ne sont pas signalés. Cela envoie des rapports de lecture à Google
  depuis votre compte ; Google peut considérer cela comme un accès automatisé.
  Les rapports en échec n'interrompent pas la lecture. La liste du compte est
  actualisée la prochaine fois que Historique est ouvert après un rapport réussi.

La liste fusionnée conserve les en-têtes de jour de YouTube. Comme l'historique
du compte comporte des libellés de jour plutôt que des heures de visionnage
exactes, l'ordre entre les deux sources est approximatif ; les libellés de jour
non reconnus conservent l'ordre du compte. Le bouton de suppression n'est
disponible que pour les vidéos conservées sur cet appareil et ne supprime que
cette entrée locale.
L'historique du compte reste en lecture seule pour la suppression. Ce paramètre
n'a aucun effet lorsque vous n'êtes pas connecté.

Un fichier de bibliothèque ou de paramètres illisible est sauvegardé avant que
les valeurs par défaut puissent être enregistrées. Un échec de lecture ou de
sauvegarde arrête le démarrage plutôt que d'écraser l'original. Les
enregistrements à la fermeture attendent ceux en cours et empêchent des
instantanés plus anciens en attente de remplacer l'état final.

**Avec vos comptes YouTube**, chaque compte est nommé et conservé dans le
trousseau sous sa propre entrée : ajoutez-en plusieurs, passez de l'un à
l'autre depuis la page du compte ou avec `a`, et associez une connexion par
navigateur à un profil (`Profile 1`, `Default`, …). Le flux, les abonnements,
l'historique, À regarder plus tard, les vidéos « J'aime » et les playlists
proviennent du compte. YouTube ne propose pas de connexion pour les
applications comme celle-ci, donc YouTFast utilise la session qu'un navigateur
possède déjà :

- **La session de votre navigateur** : YouTFast lit les cookies YouTube du
  navigateur via yt-dlp, et les relit chaque fois qu'ils expirent.
- **Un fichier cookies.txt** : exporté depuis une fenêtre de navigation privée,
  ce qui laisse la session propre du navigateur en dehors.

La session est conservée uniquement dans le trousseau du système (Secret
Service, Trousseau d'accès,
Gestionnaire d'identifiants), jamais dans un fichier ni dans le journal.
**Utiliser un compte de cette façon peut être considéré par Google comme un
accès automatisé** : le compte pourrait devoir se vérifier, être déconnecté ou
suspendu. Un compte secondaire est le choix le plus sûr. S'abonner et la liste
À regarder plus tard de l'appareil agissent localement. Évaluer une vidéo ou
modifier une playlist du compte depuis la page de la vidéo modifie le compte
sélectionné, avec annulation si YouTube refuse. La synchronisation de
l'historique peut en plus signaler les lectures démarrées à ce compte lorsque
vous choisissez explicitement **Merged and sent to YouTube** ; par défaut,
aucun rapport de lecture n'est envoyé.

Les changements de compte vident immédiatement les listes privées et les caches
et ferment les menus ouverts. Les écritures en attente se terminent sur leur
session d'origine avant le changement, et les réponses tardives ne peuvent pas
mettre à jour l'interface du nouveau compte. Une expiration de session ou un
échec de connexion écarte aussi les réponses privées du même lot en attente.
Aucun cache client n'est écrit sur le disque ; seule l'entrée du trousseau du
compte sélectionné peut restaurer sa session.

Les journaux utilisent des catégories de diagnostic fixes plutôt que des noms
de compte, des identifiants de vidéo, des profils de navigateur, des URL ou du
texte d'erreur externe. Ce masquage s'applique aussi aux dépendances et à
`--verbose` / `RUST_LOG` ; une journalisation plus détaillée ne désactive pas la
protection de la vie privée. Les téléchargements de vignettes sont partagés
entre l'affichage et le cache disque, limités à 4 Mio et quatre millions de
pixels chacun, avec au plus trois tentatives automatiques en cas d'échec
temporaire. Une réponse HTTP 429 respecte
`Retry-After` (secondes ou date HTTP) ; les échecs d'accès renouvellent la page
de commentaires d'origine une fois par commentaire sans joindre les cookies du
compte aux téléchargements d'images. Les images supprimées, les liens inchangés
ou expirés, les données non prises en charge et les limites atteintes affichent
une commande pour réessayer. Un élément de cache corrompu peut être remplacé
indépendamment des autres images.
Le cache disque est réduit en cours d'utilisation à 2 000 fichiers et 128 Mio ;
les réponses `no-store` ne sont conservées que pour l'affichage en cours.

Les diagnostics de toute l'application sont conservés localement dans
`$XDG_STATE_HOME/youtfast/logs` (normalement `~/.local/state/youtfast/logs` sous Linux).
`youtfast.log` est renouvelé vers `youtfast.log.1` ; chacun est limité à 2 Mio.
Ces fichiers couvrent toute l'application, y compris les catégories d'échec de
téléchargement et de décodage d'images. Ils ne sont pas envoyés. Sous Unix, le
dossier n'est accessible qu'à son propriétaire (0700) et les fichiers journaux
sont en 0600. Le même filtre de confidentialité s'applique avant stderr et les
fichiers, y compris pour les diagnostics des dépendances. `--verbose` ajoute les
catégories de mode d'animation (en cache/en flux) ; il n'y a pas
d'enregistrement par image. La journalisation démarre lorsque ce binaire
possède la prochaine instance en cours d'exécution ; recompiler ne met pas à
jour un processus existant. Pour suivre le journal actuel sous Linux (y compris
le renouvellement) :

```sh
tail -F "${XDG_STATE_HOME:-$HOME/.local/state}/youtfast/logs/youtfast.log"
```

Sur les plateformes sans dossier d'état XDG, les journaux utilisent le
sous-dossier `logs` du dossier local de données d'application de la plateforme
pour YouTFast. Si l'ouverture ou l'écriture du fichier échoue, les diagnostics
continuent sur stderr.

La lecture des GIF/WebP des commentaires utilise un processus de décodage et une
texture réutilisée par pièce jointe visible, avec au plus deux pièces jointes
actives. Les images sont réduites pour tenir dans 320×200 en conservant leurs
proportions. Les petites animations sont mises en cache jusqu'à environ 8 Mio ;
les plus longues continuent séquentiellement au lieu de conserver chaque image à
la taille source ou de rejeter tout le clip au-delà d'un seuil de nombre
d'images. Les réservations contrôlées pour les images, le canevas et les
textures partagent un budget de 64 Mio ; la gestion interne du décodeur, des
bibliothèques et du pilote ainsi que le cache de vignettes ordinaire constituent
de la mémoire de processus supplémentaire.
Une image dont le décodage prend plus de 200 ms arrête le décodage automatique
suivant et conserve un aperçu ; il s'agit d'une limite de travail observée, pas
d'une interruption d'un décodeur natif au milieu d'une image. Les animations
hors écran sont libérées, les commandes de pause arrêtent les demandes d'images,
et
les nombres de boucles finis de la source sont respectés. Les échecs de décodage
après une image réussie conservent cette image comme aperçu. Des blocages
externes, des médias supprimés ou une source inutilisable peuvent tout de même
empêcher un aperçu ou la lecture.

## Alertes de nouvelles vidéos

Les notifications de YouTube n'ont pas d'API, donc YouTFast vérifie plutôt vos
abonnements à intervalles réguliers (toutes les 15 minutes par défaut ; 5, 15,
30 ou 60 dans Paramètres). La cloche de la barre supérieure ouvre une liste
déroulante sans quitter la page actuelle ; vous pouvez y lancer une nouvelle
vérification ou la vider. Ouvrir la cloche marque les alertes actuelles comme
vues, mais les garde visibles pendant cette consultation.
Les vidéos trouvées par une vérification pendant que la liste est ouverte y
apparaissent et sont aussi marquées comme vues.
L'entrée Notifications du volet latéral ouvre toujours la page complète. Les
nouvelles vidéos peuvent aussi apparaître sous forme de notifications sur le
bureau. Les vérifications comme les notifications sur le bureau peuvent être
désactivées dans Paramètres ; le premier lancement ne fait que démarrer le
minuteur, pour qu'une nouvelle installation ne vous submerge pas.

## Police de l'interface

Paramètres → Apparence → Police de l'interface. **Système** (par défaut) affiche
YouTFast avec la police d'interface de votre bureau : San Francisco sur macOS,
Segoe UI sur Windows et, sous Linux, la police définie par votre bureau (la
police d'interface de GNOME, le réglage de GTK ou de KDE), sinon celle que
désigne `system-ui` dans fontconfig.
**Inter** est la police fournie avec YouTFast, identique sur tous les
ordinateurs. Le changement s'applique immédiatement. Les autres écritures
(arabe, chinois, devanagari, ...) sont affichées dans tous les cas avec les
polices installées sur l'ordinateur.

Sous Linux, le menu liste ensuite les polices installées sur l'ordinateur
(celles qui, selon fontconfig, peuvent afficher du texte anglais) : tapez dans
le champ du menu pour réduire la liste, et choisissez-en une pour afficher
l'interface avec. Le texte en gras et en graisse moyenne utilise les graisses
propres à la famille lorsqu'elle en dispose. Si une police choisie est
supprimée par la suite, YouTFast utilise Inter et indique quelle police
manque ; choisissez-en une autre dans le même menu. Windows et macOS ne
proposent que Système et Inter.

Si la police du système est introuvable ou illisible, YouTFast utilise Inter et
le groupe Apparence le signale. **Signaler ceci** ouvre votre navigateur sur une
nouvelle issue GitHub déjà remplie avec la version de YouTFast, votre système
d'exploitation, la session de bureau, la police définie par votre bureau et ce
qu'affiche `fc-match system-ui` ; vous la relisez,
ajoutez la police qu'utilise votre bureau et décidez de l'envoyer ou non.
YouTFast n'envoie rien de lui-même. Le temps du lecteur conserve les chiffres à
chasse fixe d'Inter quel que soit le choix, pour qu'il ne tremble pas pendant le
décompte.

## Thèmes

Paramètres → Apparence. **Automatique** suit en direct le thème du bureau
Omarchy sous Linux, et est sombre ailleurs. **Sombre** et **Clair** sont les
couleurs propres à YouTFast. Les huit palettes partagées (Catppuccin, Nord, Rosé
Pine, Tokyo Night, et d'autres) sont installées dans `~/.config/youtfast/themes`
au premier lancement sous forme de simples fichiers JSON que vous pouvez
modifier, copier ou supprimer ; ajoutez-y les vôtres dans le même format (une
`base` plus des couleurs nommées, voir
[fastframe-theme](https://github.com/crmne/fastframe)). Une compilation depuis
les sources n'active pas le hook Omarchy (`contrib/omarchy/youtfast-theme`). Les
paquets Linux natifs l'incluent pour une installation facultative. Le Flatpak
n'active pas les hooks de l'hôte et ne lit pas implicitement les thèmes de
l'hôte. Le suivi natif du thème fonctionne aussi sans le hook lorsque les
fichiers de thème sont accessibles.

## À propos et mises à jour

Paramètres → À propos affiche la version de YouTFast installée, la licence et le
journal des modifications inclus, et vérifie les GitHub Releases pour trouver de
nouvelles versions stables au démarrage et toutes les 12 heures tant que
l'application est ouverte :

- **Notifier les nouvelles versions** (activé par défaut) envoie une
  notification système sur le bureau lorsqu'une nouvelle version est publiée et
  met en évidence Paramètres dans le volet latéral, avec des actions rapides dans
  la notification pour mettre à jour, ouvrir À propos ou **Redémarrer maintenant**
  une fois prête.
- **Mises à jour automatiques (mineures et correctifs)** (désactivé par défaut)
  télécharge, vérifie (SHA-256 par rapport au `checksums.txt` de la version) et
  installe automatiquement en arrière-plan les mises à jour mineures (`0.x.0`)
  et correctives (`0.0.x`) pour les installations modifiables par l'utilisateur
  (Linux natif, Windows installeur/portable, macOS `.app`), puis affiche le
  journal des changements dans Paramètres → À propos.
- **Adaptation à la méthode d'installation** : les installations Arch `pacman`
  se mettent à jour via `pkexec pacman -U` au clic (jamais en arrière-plan sans
  interaction), les installations Windows (installeur/portable) préparent et
  appliquent la mise à jour proprement au redémarrage ou à la fermeture afin de ne
  jamais laisser de DLL chargées incompatibles, et les installations Flatpak en
  bac à sable notifient et permettent de copier la commande en un clic dans
  Paramètres → À propos.
- **Les versions majeures (`x.0.0`)** ne s'installent jamais automatiquement ;
  elles nécessitent de cliquer sur le bouton **Mettre à jour** dans
  l'application pour confirmer et installer la version.

## Aide

YouTube refuse les flux d'une vidéo pendant quelques secondes après les avoir
fournis (jusqu'à leur heure `available_at`). libmpv 0.42+ (avec Lua) attend
cette heure ; avec une libmpv plus ancienne, YouTFast réessaie d'ouvrir le flux
refusé après 0, 1, 3 et 7 secondes, si bien qu'une vidéo peut démarrer quelques
secondes plus tard qu'avec la 0.42. Mettre à jour libmpv vers 0.42+, ou utiliser
le Flatpak, attend le flux directement au lieu de réessayer.

Si la lecture affiche **« Impossible de lire cette vidéo. Le lecteur n'a pas pu
identifier la cause. Essayez de rouvrir la vidéo. »**, le flux était encore
refusé après environ 11 secondes de tentatives, ou autre chose a échoué :
suivez les [étapes de dépannage de l'installation](install.md#en-cas-de-problème).

Lisez [CONTRIBUTING.md](../../../CONTRIBUTING.md) pour les rapports de bug et
[SECURITY.md](../../../SECURITY.md) pour les problèmes privés de compte ou de
sécurité.
