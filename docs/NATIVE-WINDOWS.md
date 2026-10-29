# Rusted 0.3.5 — packs complets Windows x64

## Prism : importer puis Jouer

1. Télécharger **Rusted-Native-Prism-0.3.5.zip**, le pack complet.
2. Dans Prism, choisir Ajouter une instance → Importer et sélectionner ce ZIP.
3. Choisir son compte Microsoft autorisé puis cliquer **Jouer**.

Aucune commande Wrapper à saisir, aucun Configurer-Rusted.exe à exécuter.
Le pack déclare Minecraft **26.3** et Fabric Loader **0.20.1** ; Prism installe ces
composants et choisit Java **25**. Les commandes personnalisées du pack sont vides.
Les chemins avec espaces, apostrophes et `#` passent par ProcessBuilder et ne sont
plus interprétés comme une commande shell. Déplacer ou renommer le dossier ne
nécessite pas de réécrire un chemin absolu.

## Modrinth App : importer puis Jouer

1. Télécharger **Rusted-Native-Modrinth-0.3.5.mrpack**, le pack complet.
2. Dans Modrinth App, importer ce fichier comme une nouvelle instance.
3. Choisir son compte connecté puis cliquer **Jouer**.

Le pack déclare les mêmes dépendances Minecraft/Fabric et inclut le module de
démarrage dans overrides/mods. Aucun réglage global de Modrinth ou Wrapper n'est
modifié. Ces fichiers sont des imports locaux, pas une publication au catalogue.

## Comment le lancement fonctionne

Fabric appelle le petit module original **rusted_bootstrap** avant le point d'entrée
du jeu. Ce module n'utilise ni Mixin ni classe Minecraft. Il démarre notre adaptateur
Rust, résout son emplacement depuis le dossier réel de l'instance et lui transmet
les arguments de session par une pipe privée. La session n'est pas ajoutée aux
arguments de la commande du processus Rust. L'adaptateur vérifie les empreintes du
runtime et du bootstrap avant de préparer puis démarrer le moteur natif.

La JVM du lanceur reste présente pour ce démarrage et attend le processus natif.
L'arrêt du lanceur termine aussi l'arbre natif, même si la JVM est fermée de force.
Les comptes et paramètres du lanceur ne sont ni modifiés ni lus par un configurateur.

La première préparation importe la copie locale autorisée de Minecraft 26.3 : client,
bibliothèques et objets d'assets vérifiés. Elle peut télécharger les métadonnées et
le serveur officiels aux empreintes figées pour les exporters ; elle ne lance pas le
serveur et n'accepte pas d'EULA. Java 25 du lanceur suffit ; aucun JDK, compilateur,
Python ou outil de développement n'est demandé au joueur. Les ressources préparées
sont mises en cache pour les lancements suivants.

## Mods

Le mode **auto** démarre le moteur Rust lorsqu'il n'y a que le module de démarrage
Rusted dans mods. Si d'autres JAR sont installés, il rend la main à **la JVM Fabric
d'origine**. Le moteur natif est alors inactif ; les mods et leurs Mixins sont ceux
du jeu Java. Le bootstrap seul ne force donc plus le parcours Java.

Installer Sodium, Iris ou Distant Horizons par le gestionnaire de mods du lanceur,
avec des versions et dépendances compatibles avec **la même version Minecraft**.
Le test de référence 26.3 a chargé Sodium 0.9.2, Iris 1.11.7 et DH 3.3.4, puis échoué
sur le backend graphique logiciel Linux. Une partie avec ces trois mods n'est pas
certifiée ici. Leurs JAR ne s'exécutent pas dans le moteur Rust.

Ce pack utilise Fabric. Ajouter Forge/NeoForge en même temps ne crée pas une
compatibilité entre chargeurs. Les anciens adaptateurs Java restent dans les
sources, mais les autres familles ne sont pas certifiées pour ce pack d'import.
Les mods natifs .hmod sont gérés séparément par le moteur et restent désactivés
par défaut. Far View est celui d'Horizon, pas un port de Distant Horizons.

## Shaders natifs

Le pack original **Rusted Soft Light** est inclus, désactivé par défaut, dans
rusted/native-data/shaderpacks. Il applique une légère correction de couleur WGSL
sur l'image du monde, en conservant l'alpha et la plage HDR. Le choisir dans les
options natives de Shader Packs pour l'activer. Il réutilise l'API de stages
héritée d'Horizon ; le code du shader Rusted est original. Il n'ajoute ni ombres,
ni bloom, ni prise en charge des packs Iris/GLSL. Voir SHADERS.md.

## Sauvegardes, provenance et limites

Les sauvegardes natives sont dans **rusted/native-data/saves**, distinctes des
sauvegardes Java. Le passage au mode Java ne transfère pas un monde automatiquement.
Importer la 0.3.5 crée une nouvelle instance et ne modifie pas l'ancienne ; garder son
dossier de données si l'on souhaite conserver ses mondes. Aucun correctif séparé
n'est nécessaire pour installer les nouveaux packs.

Le moteur reste un dérivé de Horizon MIT, compilé depuis nos sources maintenues dans
engine/. L'identité des journaux et le doublon de détection GPU sont corrigés en
0.3.5 ; cela ne transforme pas le renderer hérité en une réécriture indépendante.
Les crédits et licences restent dans le pack. Les sources originales Rusted
comprennent notamment le bootstrap, le protocole d'import, la passerelle, les
contrôles de processus, le diagnostic des mods, l'identité et le shader optionnel.

Les tests de processus Windows sont exécutés sous Wine ; le démarrage complet
Fabric → Rusted est vérifié dans un bac à sable Linux. Les interfaces réelles de
Prism/Modrinth sous Windows et le multijoueur natif authentifié restent à confirmer.
Aucun compte, client, asset ou classe Minecraft n'est distribué dans ces packs.
