# Rusted

Rusted est un client Minecraft natif en Rust, **dérivé de Horizon**, avec ses
propres sources dans [`engine/`](engine/RUSTED.md). Il cible **Minecraft Java 26.3**.
La simulation, le rendu et le système WASM hérités sont principalement le travail
de Horizon. Les crédits et sa licence MIT sont conservés ; Rusted ne revendique
pas leur écriture originale.

Les changements Rusted comprennent le lancement depuis Prism/Modrinth, la
préparation des fichiers déjà installés par ces lanceurs, l'isolation des mondes,
le diagnostic des mods Java partagé avec le moteur, sa page dans Options > Mods,
le dossier `RUSTED_HOME` et l'identité du jeu : RUSTED EDITION, fenêtre, icône et
menus. Les services d'identité Discord, le compteur de joueurs et les migrations
vers le registre Horizon sont désactivés pour ce produit dérivé.

## Rusted Launcher — Windows

La version **0.2.6** simplifie l'interface cuivre : accueil avec accès directs,
bibliothèque d'instances séparée du formulaire de création, brouillon conservé,
commandes indisponibles désactivées et navigation clavier sans arrêt sur les textes.
La disposition se compacte jusqu'à 1120 × 720 et les contrôles suivent les
redimensionnements. [Aperçu de l'accueil](docs/images/launcher-0.2.6/home.png).

La connexion affiche le code Microsoft dans une fenêtre au-dessus de
l’accueil, avec Copier, Ouvrir Microsoft et Annuler. Aucun ID ni inscription Azure
n’est demandé au joueur. La connexion complète avec un vrai compte reste à valider.
Voir [le fonctionnement](docs/MICROSOFT-SETUP.md).

**[Archives Windows validées — Rusted Launcher 0.2.6](https://github.com/bloupyi/Rusted/actions/runs/41195621144/artifacts/13040817089)**
(téléchargement Actions avec connexion GitHub). L’artefact contient le ZIP complet
`Rusted-Launcher-Windows-0.2.6.zip`, le moteur **0.3.6**, les packs Prism/Modrinth,
les sources et leurs empreintes. Extraire tout le ZIP du lanceur, puis ouvrir
`Rusted Launcher.exe`. Le [rapport Windows](docs/results/rusted-launcher-0.2.6-windows.json)
est conservé dans le dépôt. Il s’agit d’une version de développement.

Les cinq jobs CI passent. Sur Windows réel, le scan Defender, le démarrage de
20 secondes, 40 redimensionnements, la navigation entre les pages et la demande d’un vrai code Microsoft avec
annulation sans compte réussissent, protections en temps réel et comportementale
actives. Ces contrôles ne couvrent pas une connexion complète, une préparation
entière ou une partie. Les exécutables restent non signés.

Le moteur corrige les modèles d’objets imbriqués, les textures personnalisées,
les styles des noms transmis sur le réseau et les règles de collision des équipes
distantes. Un pack obligatoire original utilisant 1 195 chemins du catalogue
SkyBlock s’affiche sur le banc Linux ; le pack réel et une session Hypixel restent
à confirmer. Voir [les résultats et limites](docs/results/rusted-native-0.3.6.json).

La distribution Windows **Rusted Launcher 0.2.0 est en attente de validation antivirus** :
Microsoft Defender a signalé `Behavior:Win32/DefenseEvasion.A!ml` sur
`Rusted Launcher.exe`. Ne pas restaurer le fichier ni désactiver la protection pour
l'exécuter. Les binaires 0.2.0 dans `build/` sont conservés pour l'analyse ; ils ne
constituent pas un correctif validé. Voir [le suivi](docs/WINDOWS-SECURITY.md).

Le lanceur est développé dans `launcher/`. Son interface
cuivre gère les instances, le préchargement vérifié de Java/Minecraft et des ressources
natives avant le jeu, les comptes, les mods Fabric, la console, l'import Prism/Modrinth
et l'export Prism. Aucun configurateur séparé ni terminal n'est requis.

Depuis 0.2.1, l'accueil présente un unique bouton **Jouer**. La préparation reste
automatique à la création ou à l'import de l'instance ; Jouer attend sa fin si
nécessaire. Les vérifications Windows sont suivies dans
[GitHub Actions](https://github.com/bloupyi/Rusted/actions/runs/41195621144).

La connexion utilise par défaut un client public Microsoft Live partagé ; une identité
Entra personnalisée reste facultative : [guide Microsoft](docs/MICROSOFT-SETUP.md). Les préparations se font
sans compte. Cette première version n'atteint pas encore toute la parité de Prism ;
lire [les fonctions et limites](docs/RUSTED-LAUNCHER.md). Le jeu/renderer reste dérivé
de Horizon, tandis que l'interface et le contrôleur de ce lanceur sont originaux.

## Windows : importer puis Jouer

Les **packs complets 0.3.6** se trouvent dans build/ :

- **Rusted-Native-Prism-0.3.6.zip** : importer dans Prism, puis Jouer.
- **Rusted-Native-Modrinth-0.3.6.mrpack** : importer dans Modrinth App, puis Jouer.

Aucun Configurer-Rusted.exe, aucune commande Wrapper et aucun terminal ne sont
nécessaires après l'import. Le pack déclare Fabric Loader 0.20.1 et contient notre
petit module de démarrage original. Il transmet la session par une pipe au processus
Rust sans modifier les paramètres du lanceur. Java **25** du lanceur reste nécessaire.
Le ZIP Windows autonome reste un runtime destiné aux intégrations, pas le pack à
importer. Les anciennes instructions d'installation 0.3.4 ne s'appliquent pas ici.
Lire [le guide](docs/NATIVE-WINDOWS.md).

## Mods, shaders et travail restant

Le mode auto conserve le moteur natif avec le bootstrap Rusted et les modules passifs
de ressources/données compatibles. Avec des mods Java exécutables, il poursuit dans
la JVM Fabric d'origine ; le moteur Rust est alors inactif. **Les JAR Sodium/Iris/Distant Horizons ne fonctionnent pas dans le
moteur natif.** Leur partie en jeu n'est pas validée ici. Plusieurs familles de chargeurs
ne sont pas combinées. Les mods natifs .hmod hérités de Horizon restent séparés. Voir [la matrice des mods](docs/MOD-COMPATIBILITY.md).

Un premier shader original **Rusted Soft Light**, optionnel et désactivé par défaut,
est inclus au format natif WGSL. Il réutilise l'API de stages héritée, avec notre propre
correction de couleur. Le lecteur GLSL natif distinct passe un test original à trois passes ; chaque pack
tiers reste à vérifier. Voir [les formats et limites](docs/NATIVE-SHADERPACKS.md).

Les journaux renderer portent désormais l'identité Rusted et n'annoncent le GPU
qu'une fois à l'ouverture du périphérique. Le renderer lui-même reste celui de la
base Horizon : un changement de nom ne constitue pas une réécriture. La compatibilité
native des mods Java, les packs Iris/GLSL, la parité complète et le multijoueur natif
authentifié restent des travaux ouverts. Voir [les limites](docs/KNOWN-ISSUES.md).

## Sources et compilation

Les sources du jeu sont conservées dans ce dépôt ; la compilation de Rusted ne
retélécharge pas un jeu Horizon prêt à l'emploi. La révision amont et les
empreintes initiales sont dans `horizon/`. Pour inspecter les changements :

```sh
python scripts/rusted.py changes
```

Rust 1.90.0 pour l'adaptateur, Rust 1.99.1 pour le moteur, Python 3 pour les outils.
Sous Windows, installer les Build Tools C++ de Visual Studio et le SDK Windows.
Sous Linux, le moteur complet utilise les en-têtes ALSA et udev.

```sh
rustup toolchain install 1.99.1 --profile minimal
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python scripts/test_horizon_bridge.py
python scripts/rusted.py build
cargo build --release --locked -p rusted-launch
```

Les binaires du moteur sont dans `build/horizon/target/release/`. Le nom `horizon`
de certains dossiers/crates et formats internes est conservé pour les outils
hérités. Le binaire du jeu est `rusted.exe` sous Windows, `rusted` sous Linux.
`rusted-build.json` lie les binaires aux sources ; l'empaquetage refuse une
compilation périmée. Sous Windows, après compilation :

```sh
python scripts/package_horizon_native.py --binary-dir build/horizon/target/release --bridge-binary target/release/rusted-launch.exe
```

L'assemblage réutilise seulement les exporters originaux d'une release MIT,
après vérification de son SHA256 et de la correspondance exacte des sources
Java. Les fichiers Minecraft nécessaires restent privés sur la machine.

## Validation et travaux restants

Les vérifications reproductibles et leurs limites sont enregistrées dans
[`docs/results/rusted-native-0.3.0.json`](docs/results/rusted-native-0.3.0.json).
Les tests de protocoles utilisent notre programme de test ; ils ne prouvent pas
un démarrage depuis les interfaces réelles des lanceurs avec un compte connecté.
Wine reste une émulation, pas un test sur une vraie installation Windows.

L'ancien prototype JNI/Fabric 1.20.4 est conservé dans `native/`, `java/` et les
scripts associés comme travail antérieur ; il ne constitue pas le moteur Rusted.
