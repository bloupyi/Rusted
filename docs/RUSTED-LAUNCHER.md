# Rusted Launcher 0.2.5 — Windows x64

La connexion Microsoft est fournie automatiquement, avec le même client public
Live que la bibliothèque utilisée par FaultlineLauncher. Aucun ID d’application
ni inscription Azure n’est demandé. La connexion ouvre le navigateur Microsoft,
copie le code à coller si nécessaire, puis attend la validation. L’intégration est
écrite en Rust ; Electron n’est pas embarqué. Voir [le fonctionnement](MICROSOFT-SETUP.md).

Extraire **tout** `Rusted-Launcher-Windows-0.2.5.zip`, puis ouvrir
**Rusted Launcher.exe**. Garder le sous-dossier `rusted` à côté de l'exécutable.
Il contient le moteur Rusted 0.3.6 et ses outils. Aucun terminal, Prism ni
Configurer-Rusted.exe n'est nécessaire pour utiliser ce lanceur.

## Nouveautés 0.2.5 / moteur 0.3.6

Les modèles d'objets imbriqués et les textures déclarées dans des dossiers personnalisés
sont chargés ; une texture absente affiche un damier. Les noms conservent leurs couleurs
et leur `italic:false`, y compris après transfert réseau. Les équipes des joueurs distants
sont résolues par leur nom de profil, pour respecter les règles de collision du serveur.
Le chargement des packs évite de relire toute la liste de fichiers pour chaque dossier
et affiche des étapes plus précises.

Les modules de ressources/données sans code Java sont chargés directement depuis `mods`.
Fabric, Quilt et les modules Forge/NeoForge `lowcodefml` pris en charge restent en natif.
L'installation d'un JAR incompatible depuis Modrinth conserve le moteur de l'instance et
explique le portage nécessaire. Une erreur de préparation est affichée dans le statut et
la console, sans bloquer la connexion Microsoft par une boîte de dialogue.

Les essais locaux et leur portée sont consignés dans
[le rapport 0.3.6](results/rusted-native-0.3.6.json). Hypixel SkyBlock avec un compte connecté
reste à vérifier.

## Nouveautés 0.2.4

La connexion affiche une fenêtre au-dessus de l’accueil avec le code Microsoft,
Copier, Ouvrir Microsoft et Annuler. Elle se ferme quand la connexion se termine
ou est annulée. La préparation continue en arrière-plan.

Le redimensionnement place désormais les contrôles en une seule opération sans
copier leurs anciens pixels et repeint toute la fenêtre et ses enfants après
la mise en page. Les tests Windows incluent des redimensionnements et la demande
d’un vrai code Microsoft, annulée sans compte enregistré.

## Nouveautés 0.2.1

Le bouton de préchargement manuel est retiré. « Jouer » prend sa place dans la
carte d'accueil, sans doublon dans le pied de page. Les autres pages affichent
l'instance concernée à côté de leur action Jouer. « Annuler » n'apparaît que
pendant une opération ou un lancement en attente.

## Fonctions conservées depuis 0.2.0

Le sélecteur propose toutes les versions officielles sans obfuscation publiées
jusqu'au 29 octobre 2026 : **63 versions**, dont les cinq stables 26.1, 26.1.1,
26.1.2, 26.2 et 26.3. Cocher « Snapshots, préversions et candidates » pour les
58 autres, y compris la version spéciale 26w14a et 26.4-snapshot-6.
La liste se met à jour auprès de Mojang et reste disponible hors ligne grâce
au catalogue inclus. Les métadonnées de la version choisie sont vérifiées avant
le téléchargement. Le moteur natif reste limité à 26.3 ; les autres versions
utilisent Java, ou Fabric lorsque celui-ci prend en charge la version choisie.

Les instances créées se préparent automatiquement. Le moteur n'est pas lancé
pendant cette préparation. Les connexions HTTPS sont réutilisées et les assets
se téléchargent sur 24 tâches au lieu de 4. Un échantillon froid de 256 petits
assets a pris 8,45 s contre 32,06 s auparavant sur notre banc Linux ; ce résultat
ne garantit pas un temps de 30 s sous Windows. Le tout premier démarrage natif
exige aussi l'export des ressources du moteur, travail absent du lancement Java
classique de Prism.

Les sélecteurs et listes utilisent le thème cuivre, les mods ont des onglets
« Mes mods » et « Découvrir », avec cartes, descriptions et raccourcis Sodium,
Iris et Distant Horizons. Les changements de page sont dessinés en une fois et
les textes immobiles ne sont plus réécrits à chaque passage du timer.

Dans une distribution configurée, la connexion Microsoft ouvre automatiquement le
navigateur et copie le code à saisir. Elle fonctionne indépendamment d'une préparation
en cours. Aucun identifiant technique n'est demandé au joueur. Dans un aperçu sans
connexion disponible, le bouton est désactivé avec une explication.

Le PNG du R est désormais le fichier original fourni par le propriétaire,
copié octet pour octet, avec ses bords noirs et son alpha d'origine. Seules les
versions techniques ICO et RGBA 64×64 sont redimensionnées pour les API du système.

## Premier démarrage

1. Une instance **Rusted 26.3** est créée automatiquement.
2. La préparation démarre automatiquement à la création et à l'import. Il n'y a
   plus de bouton de préchargement manuel. Elle télécharge Minecraft, ses bibliothèques, ses textures,
   sons et langues, ainsi que Java 25. Les empreintes des téléchargements sont
   vérifiées. Pour le moteur natif, les ressources sont également exportées avant
   d'ouvrir le jeu. La première préparation peut prendre plusieurs minutes.
3. Dans **Comptes**, cliquer **Se connecter avec Microsoft** et terminer la connexion
   dans le navigateur Microsoft. Le code est copié automatiquement ; le saisir si
   Microsoft le demande. Sélectionner le compte qui possède Minecraft Java.
4. Le bouton **Jouer** est l'action principale de l'accueil ; sur les autres pages,
   il reste dans la barre de l'instance sélectionnée. Pendant une préparation,
   cliquer Jouer programme le lancement dès que les fichiers et le compte sont prêts.
   Annuler ou changer d'instance supprime cette attente. Se connecter avec un compte qui donne accès à Minecraft Java, dans le navigateur
   Microsoft. Copier le code affiché si nécessaire. Sélectionner ce compte, puis
   **Jouer**. Le bouton Jouer termine aussi la préparation si elle n'a pas été faite.

La préparation ne demande pas de compte. Les fichiers propriétaires de Minecraft
et le runtime Java sont récupérés auprès de leurs fournisseurs ; ils ne sont pas
inclus dans le ZIP. Un mod peut effectuer ses propres téléchargements après son
démarrage : le lanceur ne les contrôle pas.

## Fonctions de cette première version

- Interface Windows originale, thème cuivre et icône Rusted.
- Instances séparées : création, groupes, duplication avec conservation de la source,
  ouverture du dossier, suppression vers la corbeille interne.
- Modes **Rusted natif**, **Java vanilla** et **Java + Fabric** ; version Minecraft
  sélectionnable lors de la création, moteur natif limité à 26.3.
- Mémoire Java, Java personnalisé ou automatique, arguments JVM (un par ligne), taille
  de fenêtre et plein écran ; distance et API graphique pour le moteur natif.
- Cache partagé et vérifié, progression, annulation ; préparation automatique au
  démarrage activable par instance ; réduction du lanceur pendant une partie.
- Console en direct, copie des journaux, arrêt du jeu et de ses processus enfants.
- Comptes Microsoft multiples, renouvellement de session ; secrets chiffrés avec
  Windows DPAPI pour l'utilisateur courant. Aucun mot de passe saisi dans le lanceur.
- Mods locaux activables, désactivables ou déplaçables vers la corbeille ; recherche
  Modrinth et installation de mods Fabric avec leurs dépendances requises.
- Import **Prism ZIP** et **Modrinth .mrpack**, export **Prism ZIP**. Les commandes
  arbitraires contenues dans les packs ne sont pas exécutées à l'import.

## Mods et mondes

Les JAR comportant du code Java utilisent **Java + Fabric**. Les modules passifs
compatibles restent sur **Rusted natif** et leurs ressources/données sont chargées au
prochain lancement. L'installation d'un mod incompatible en mode natif est refusée avec
un diagnostic ; elle ne change pas le moteur. Voir [la matrice de compatibilité](MOD-COMPATIBILITY.md).

Sodium, Iris et Distant Horizons nécessitent des versions compatibles avec la version
Minecraft choisie. Leur partie réelle avec ce nouveau lanceur n'est pas encore validée.
Les mods natifs `.hmod` et Rusted Soft Light restent propres au moteur natif ; leurs
options s'activent dans le jeu. Les mondes Java et natifs sont séparés.

Les packs Rusted 0.3.5 existants restent utilisables dans Prism et Modrinth. L'import
dans le nouveau lanceur retire uniquement notre bootstrap reconnu par son empreinte.
L'export Prism inclut l'adaptateur Rusted nécessaire au lancement depuis Prism.

## Données

Par défaut : `%APPDATA%\RustedLauncher`.

- `instances/<identifiant>/minecraft` : fichiers de l'instance Java.
- `instances/<identifiant>/minecraft/rusted/native-data` : données du moteur natif.
- `cache` : fichiers Minecraft, Java et ressources exportées, partagés entre instances.
- `accounts` : profils et sessions chiffrées ; ne pas partager ce dossier.
- `trash` : instances et fichiers retirés. Le lanceur ne vide pas cette corbeille.

Le dossier peut être choisi avec `--data-dir DOSSIER` ou `RUSTED_LAUNCHER_HOME`.
Ces options sont facultatives. Les exports ne contiennent ni comptes, ni cache,
ni mondes ; les packs tiers peuvent redistribuer certains mods sous leurs licences.

## Limites connues

Cette version n'a pas encore toutes les fonctions de Prism ou du lanceur officiel.
Pas de chargeurs Java Forge/NeoForge/Quilt, mélange universel de loaders, gestion des skins,
Bedrock, plusieurs jeux simultanés, mise à jour automatique du lanceur ou export
Modrinth. Les anciennes versions à arguments de lancement legacy sont refusées.
Les imports conservent la version Minecraft, mais installent Fabric 0.20.1 ; les packs
exigeant une autre révision du loader peuvent nécessiter Prism.

Le client public Live est fourni par défaut ; aucune inscription d'application
personnelle n'est requise. Une connexion complète n'a pas encore été validée avec
un compte réel ici ; elle nécessite l'accès Minecraft Java et un profil Xbox valide.
L'interface a été essayée sous Wine ; 40 redimensionnements, le code Microsoft visible
et son annulation sans compte passent aussi sur Windows réel. La compilation MSVC, les
tests unitaires et de communication, un scan Defender et un premier démarrage de
20 secondes réussissent sur Windows réel, protections en temps réel et comportementale
actives (signatures 1.463.36.0 ; [rapport](results/rusted-launcher-0.2.5-windows.json)).
Ce contrôle ne couvre pas une préparation entière, un compte réel ou une partie.
Le jeu sur un vrai PC Windows, le GPU et le multijoueur authentifié restent à confirmer.
Le ZIP n'est pas signé avec un certificat de publication Windows.

Le **lanceur** et son interface sont des développements originaux Rusted. Le
**moteur et le renderer** restent dérivés de Horizon sous MIT : voir
`rusted/PROVENANCE.md` et les licences incluses. Aucun renommage n'est présenté
comme une réécriture du moteur. Minecraft est un produit de Mojang/Microsoft.

## Compilation

Rust 1.90, Python 3, JDK avec `javac`, outils C++/SDK Windows.

```sh
cargo test --workspace --locked
cargo clippy -p rusted-launcher --all-targets --locked -- -D warnings
cargo build --release --locked -p rusted-launcher
python scripts/build_launcher_entry.py
python scripts/test_launcher_java_entry.py --java CHEMIN_VERS_JAVA
python scripts/package_launcher.py --source
```

L'assemblage attend `build/Rusted-Native-Windows-0.3.6.zip`, produit par les outils
du moteur décrits dans le README. Il vérifie ses empreintes et refuse des sources
de moteur différentes de celles du runtime. Les versions du lanceur et du moteur
évoluent séparément.

## Import de modules passifs en natif

Sur une instance native 26.3, **Mods > Mes mods > Importer un module** accepte les
archives Fabric/Quilt et les modules Forge/NeoForge `lowcodefml` pris en charge.
Le JAR est copié intact dans `mods` ; au prochain lancement, ses ressources et données
sont chargées automatiquement. Aucun pack généré à activer manuellement.

Données prises en charge : fonctions, tags de fonctions, prédicats et modificateurs
d'objets. Les classes, hooks Java, mixins, archives imbriquées, dépendances supplémentaires,
identifiants dupliqués et données non prises en charge sont refusés. Les recettes et
tables de butin ne font pas encore partie de cet import. **Sodium, Iris et Distant Horizons
nécessitent encore Java/Fabric ou un portage natif.**

Les `.hmod` utilisent l'API WASM héritée. Pour les shaderpacks, voir
[le parcours natif GLSL/WGSL](NATIVE-SHADERPACKS.md).
