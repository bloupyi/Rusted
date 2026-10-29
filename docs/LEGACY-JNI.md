# Rusted

**Migration progressive du Minecraft Java existant vers un backend Rust.**
Minecraft reste le programme exécuté : ses menus, son rendu, ses fonctionnalités,
ses sauvegardes et son multijoueur restent gérés par le jeu d'origine pendant la
migration. La JVM est conservée. Le dépôt ne fournit pas un autre jeu de blocs.

Première cible : **Minecraft Java 1.20.4**, Fabric Loader **0.15.11**, Java **17+**,
Windows/Linux x86_64. Ces versions constituent une cible d'intégration ; leur
compatibilité complète avec ce module n'est **pas encore validée**.

La première brique implémentée est une bibliothèque native Rust appelée par JNI,
avec un adaptateur Fabric destiné à remplacer la compression zlib des paquets
sortants. L'interface et le protocole restent ceux de Minecraft. Seule cette
opération est déléguée à Rust ; **le backend du jeu entier n'est pas encore porté**.

La bibliothèque est désormais appelée par **les vraies classes réseau de
Minecraft, transformées par Fabric**. Les trois modes Rust activé, désactivé et
bibliothèque absente passent chacun 301 cas, dont 24 paquets sur TCP local.
Le client original a également été lancé sous Linux en mode démo officiel :
menus, déplacements, inventaire, destruction/ramassage/placement d'un bloc et
sauvegarde/relecture après redémarrage ont été vérifiés. Voir
[les résultats et leurs limites](VALIDATION.md) et
[l'état de reprise](RESUME.md).

## Essayer le module déjà construit

**Utilisateur Windows : [guide d'installation sans compilation](WINDOWS.md).**
L'archive dédiée est `build/Rusted-Windows-1.20.4.zip`.

Le JAR `build/rusted-backend-0.1.0-mc1.20.4.jar` contient les bibliothèques natives
Linux et Windows x86_64. Aucun compilateur Rust n'est nécessaire pour l'utiliser.
Créer une instance de test Minecraft **1.20.4**, avec Fabric Loader **0.15.11**,
y copier le JAR dans `mods/`, puis ajouter l'argument JVM
`-Drusted.enabled=true`. Utiliser son lanceur et son compte autorisé habituels.

L'archive `build/Rusted-1.20.4-native-test.zip` prépare cette configuration pour
Prism/MultiMC : importer le ZIP comme **nouvelle instance**. Elle ne contient que
notre module et la configuration ; le lanceur télécharge le jeu et gère le compte.
La structure de l'archive est vérifiée, mais son import et l'exécution du module
sous Windows restent à tester. Ne pas installer `rusted-integration-test.jar`
ni `bridge-test.jar` comme mods de jeu : ils sont réservés aux tests.

## Compiler et tester la passerelle

Prérequis locaux : Rust 1.90.0, JDK 17+ avec `javac`, Python 3. Sous Windows,
utiliser Rust MSVC et les outils de compilation C++ Visual Studio. Depuis la
racine du dépôt, les mêmes commandes fonctionnent sous Windows et Linux :

```sh
cargo test --workspace --locked
python scripts/test_bridge.py
python scripts/test_bridge.py --no-build --benchmark
```

Le test construit une vraie bibliothèque native, l'embarque dans un JAR de test,
la charge dans Java et vérifie les échanges dans les deux sens. Ce JAR de test
ne doit pas être installé comme mod Minecraft. Les rapports se trouvent dans
`build/reports/`. Aucun compte ni aucune installation Minecraft n'est nécessaire
pour ces tests de passerelle.

Pour tester le branchement sur le Minecraft original :

```sh
cargo build --release --locked
python scripts/build_mod.py --host
python scripts/test_minecraft.py
```

Ce dernier test obtient les fichiers serveur officiels, vérifie leurs empreintes,
puis charge les vrais encodeur/décodeur et notre mixin sous Fabric. Le mod de test
arrête le processus avant de démarrer un serveur ou un monde. Il ne contourne
pas d'authentification et ne constitue pas un test multijoueur connecté à un compte.

Le client démo isolé, sans changement d'une installation existante :

```sh
python scripts/prepare_client.py
python scripts/launch_demo.py
```

La préparation télécharge environ 657 Mo de ressources officielles, plus le
client et ses bibliothèques. La démo conserve ses restrictions d'origine.
`--disabled` permet de lancer avec le backend Java ; `--instance` réutilise
uniquement un dossier de test déjà créé par ce script. Les logs de chaque
lancement sont conservés dans `rusted-runs/`, en plus des logs/crash reports du jeu.

Dans cet environnement cloud :

```sh
bash scripts/setup-cloud.sh
source scripts/cloud-env.sh
```

## Préparer le module pour le vrai Minecraft

La compilation et l'assemblage suivants sont vérifiés dans cet environnement :

```sh
# Linux :
cargo build --release --locked
python scripts/build_mod.py --linux target/release/librusted_backend.so

# Windows, après compilation locale :
cargo build --release --locked
python scripts/build_mod.py --windows target/release/rusted_backend.dll
```

Le script vérifie les dépendances Java avec les empreintes publiées par leurs
dépôts officiels sur HTTPS. Il produit `build/rusted-backend-0.1.0-mc1.20.4.jar`.
Il ne distribue aucun binaire ni aucune ressource de Minecraft.

Pour utiliser le lanceur existant, créer **une instance de test distincte** de
Minecraft 1.20.4 avec Fabric Loader 0.15.11. Y placer notre JAR dans `mods/`, puis
ajouter `-Drusted.enabled=true` aux arguments JVM. Utiliser son compte autorisé
normalement. Ne pas copier de sauvegarde personnelle pour le premier essai.

Un générateur de configuration de test Prism/MultiMC est également fourni :

```sh
python scripts/package_prism.py --mod build/rusted-backend-0.1.0-mc1.20.4.jar
```

Importer le ZIP généré dans Prism/MultiMC, puis laisser ce lanceur obtenir le jeu
et gérer l'authentification. **L'import et le lancement de cette configuration
n'ont pas encore été testés.** Ce script ne lance pas un jeu réimplémenté en Rust.

## Vérifier l'intégration réelle

Les menus Minecraft habituels et une partie de démo ont été vérifiés sur Linux.
Une connexion à un
serveur Java de test avec compression activée doit provoquer les logs
`Rust backend JNI loaded` puis `First native encode call`. Vérifier ensuite les
chunks, l'inventaire, le chat, les reconnexions et les sauvegardes avec et sans
`-Drusted.enabled=true`. Un log de chargement natif seul ne suffit pas.

Sans activation, la compression reste Java. En cas d'échec de chargement JNI ou
d'échec natif avant modification des buffers, l'encodage conserve le chemin
Java. Une incompatibilité du point d'injection Fabric provoque volontairement
une erreur de chargement plutôt qu'une fausse réussite ; retirer le mod de
l'instance de test pour revenir au jeu d'origine.

## Architecture et suite

- `native/` : backend Rust, ABI JNI et tests ; zlib en Rust via `miniz_oxide`.
- `java/` : petite passerelle et mixin Fabric, sans remplacement de l'interface.
- `scripts/` : compilation, tests différentiels, mesure et instance isolée.
- `compatibility/versions.json` : état réel de chaque version étudiée.
- `docs/TECHNICAL.md` : sources, décisions, contrats et limites.

Les prochains ports devront conserver les API observées par le client Java et
les mods, mesurer le coût des copies JNI et tester les comportements sur le jeu
original. Une autre version ou un autre chargeur exigera son propre adaptateur et
ses tests. Aucun gain de performance global n'est établi.
