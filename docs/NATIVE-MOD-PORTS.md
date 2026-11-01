# Ports natifs associés aux JAR — expérimental

Rusted peut charger le **port Rust/WASM fourni dans un JAR** Fabric, Quilt, Forge
ou NeoForge. Le code Java, les entrypoints Java et les mixins du JAR restent
inactifs. Le port natif remplace leur comportement ; ses capacités sont celles
du SDK natif existant. Ce mécanisme ne traduit pas le bytecode Java et ne rend
pas compatibles les JAR actuels de Sodium, Iris ou Distant Horizons.

Le joueur importe le JAR depuis le launcher puis active le module dans
**Options > Mods**. Son code reçoit les événements du moteur et peut produire du
HUD, des commandes ou du contenu selon ses capacités déclarées. Retirer ou
renommer le JAR en `.jar.disabled` supprime le module de la découverte au prochain
chargement. Le cache seul n'est jamais un emplacement d'installation actif.

L'import direct de JAR est couvert. La résolution Modrinth reste celle de Fabric :
un projet exigeant des bibliothèques Java sans port sera encore refusé dans ce
parcours. L'exécution complète de mods Java inchangés reste hors de ce mécanisme.

## Contrat d'archive, version 1

Le JAR conserve ses fichiers Java et les métadonnées de son loader. Il ajoute :

- `META-INF/rusted/native.json` ;
- le module `.hmod` indiqué par cette déclaration, sous `META-INF/rusted/`.

```json
{
  "schema": 1,
  "minecraft": "26.3",
  "id": "example_mod",
  "package": "META-INF/rusted/port.hmod",
  "sha256": "empreinte SHA-256 hexadécimale du fichier .hmod"
}
```

L'identifiant doit être le même dans le manifeste natif et dans toutes les
métadonnées du loader présentes dans l'archive. Un seul mod source par archive
est pris en charge. Le `.hmod` contient `horizon-mod.toml` et `mod.wasm`, un
composant WASM du SDK natif. Les API 0.1, 0.2 et 0.3 utilisent le runtime existant,
qui vérifie le manifeste complet, les imports et les capacités avant exécution.
Les ressources requises par le port sont incluses dans le `.hmod` ; le contenu
Java extérieur n'est pas chargé automatiquement comme ressource native.

Rusted vérifie l'empreinte, l'identité, les limites de taille et les chemins des
deux archives. Il met uniquement les octets du `.hmod` en cache, par empreinte,
sans extraire les classes Java. Les identifiants dupliqués sont refusés. Le
module reste soumis à la sandbox WASM et à ses limites de mémoire et de temps.

## Construire une archive avec son propre port

Après avoir développé et testé le port avec le SDK, l'auteur utilise Python 3.11+
pour produire un **nouveau** JAR, sans modifier l'original :

```sh
python scripts/build_native_companion.py --jar original.jar --native port.hmod --output mod-rusted.jar
```

Fournir un `.hmod` est indispensable : cet outil assemble les fichiers, il
n'écrit pas le port. Les dépendances du parcours Java ne sont pas chargées dans
le moteur natif ; le port doit être autonome ou utiliser les capacités du SDK.
Ce premier contrat ne résout pas encore les dépendances entre ports natifs.

## Vérification d'exécution

Le probe de développement `rusted_native_port` suit le chemin réel de découverte
et d'exécution : JAR, déclaration, cache, runtime WASM, 60 ticks et 60 images,
puis présence d'un panneau HUD. Le fixture utilise le compteur FPS natif
existant et une classe Java de test qui échouerait si son initializer était
exécuté. Il ne représente aucun des trois mods prioritaires.

```sh
cargo +1.99.1 run --release --manifest-path engine/Cargo.toml -p horizon-mods --features runtime --example rusted_native_port -- MODS_DIR CACHE_TEMPORAIRE
```

Le chargement du jeu utilise cette même découverte pour les mods de code et de
contenu, avec ou sans runtime WASM compilé. Un build sans runtime signale que le
code natif ne peut pas démarrer.

Probe reproductible à partir du module FPS natif déjà construit :

```sh
python scripts/test_native_companions.py --native build/horizon/native-mods/fps_overlay.hmod
```

Le script crée sa propre interface de test Fabric et un initializer Java, assemble
un nouveau JAR et vérifie la production HUD native dans un dossier temporaire.
Il ne télécharge aucun mod tiers et ne démarre aucune JVM dans le moteur.
