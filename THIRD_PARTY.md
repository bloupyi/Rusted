# Dépendances et provenance

- Code du projet : MIT, voir `LICENSE`.
- `jni`/`jni-sys` : MIT ou Apache-2.0 ; ABI JNI de la JVM utilisée localement.
- `flate2` : MIT ou Apache-2.0 ; backend `miniz_oxide` en Rust, MIT ou Zlib ou
  Apache-2.0. Le graphe exact et les checksums sont dans `Cargo.lock`.
- Yarn : CC0 1.0, [licence au commit utilisé](https://github.com/FabricMC/yarn/blob/f7f320f7305c92be62c1cf96679b03c29bb3b87d/LICENSE).
  Aucune source décompilée Minecraft n'est intégrée.
- Fabric Loader : Apache-2.0, [version 0.15.11](https://github.com/FabricMC/fabric-loader/blob/0.15.11/LICENSE).
- SpongePowered Mixin : MIT, [licence](https://github.com/SpongePowered/Mixin/blob/0.8.5/LICENSE.txt).
- Netty : Apache-2.0, [licence](https://github.com/netty/netty/blob/netty-4.1.97.Final/LICENSE.txt).

Mixin/Netty servent à compiler l'adaptateur et sont fournis au runtime par
l'installation Minecraft/Fabric ; le JAR du projet ne les embarque pas.
Le script d'assemblage copie les notices des crates téléchargées dans le JAR.

Minecraft, ses binaires, ressources, mappings officiels éventuels et marques
restent soumis aux conditions de leurs titulaires. Aucun de ces fichiers n'est
redistribué ici. Les intégrations Prism/Modrinth transmettent la session fournie par leur lanceur
via un canal privé. Rusted Launcher peut aussi obtenir sa propre session auprès de
Microsoft/Xbox/Minecraft avec une application configurée par l'utilisateur ; elle
est chiffrée par Windows DPAPI et transmise au jeu par un fichier temporaire privé.

## Base native Horizon et adaptateur de lancement

La base native est [Horizon](https://github.com/horizon-rust/horizon), sous licence
MIT, copyright 2026 Ludovic Marie and the Horizon contributors. Sa source publique est conservée dans `engine/`, avec sa licence originale intacte.
La révision de base est dans `horizon/upstream.lock.json`; les empreintes originales
de chaque fichier sont dans `horizon/source-baseline.json`. `engine/RUSTED.md`
décrit les modifications du produit dérivé, sans attribuer à Rusted l'écriture
du moteur initial. Les noms de crates et formats `horizon-*` hérités restent
présents pour préserver les API et les données. Les crédits sont aussi visibles
sur l'écran des crédits du jeu.

Le code original Rusted comprend la passerelle Prism/Modrinth, le bootstrap de
préparation et le module `rusted-platform` de diagnostic. Les distributions natives
incluent le moteur construit depuis ces sources, les licences et notices des
crates, les fontes et leurs licences, ainsi que les exporters originaux vérifiés.
Les exporters compilés proviennent d'une release MIT Horizon dont l'empreinte
SHA256 et la correspondance exacte des sources sont vérifiées. Aucun JAR,
asset, compte ou monde Minecraft n'est distribué.

## Rusted Launcher original

`launcher/` contient notre interface Win32 et notre contrôleur d'instances, de
téléchargements, d'authentification et de packs. Le code de l'ancien lanceur Horizon
GPUI n'est pas réutilisé par cette application. Le petit point d'entrée Java est
original et compilé contre la seule bibliothèque standard ; il n'embarque aucune
classe Minecraft/Fabric. Les notices des dépendances du nouveau lanceur sont
ajoutées au ZIP par `scripts/package_launcher.py`. Java et les mods tiers sont
téléchargés séparément, avec leurs propres licences, et ne sont pas inclus dans
l'archive du lanceur.
