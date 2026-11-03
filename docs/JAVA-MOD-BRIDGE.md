# JAR Fabric existants dans le client Rust — expérimental

Le pont `rusted-java` exécute le bytecode de JAR Fabric existants dans une JVM
embarquée dans le processus natif. Le monde, le réseau et le rendu restent ceux
du client Rust. Il n'appelle pas le point d'entrée de Minecraft Java et ne
convertit pas les JAR en WASM.

Ce travail est en développement. Le ZIP stable du launcher **0.2.7** corrige le
démarrage Windows mais contient encore le moteur stable, sans ce pont. Ajouter
un JAR dans ce ZIP ne fournit donc pas cette compatibilité expérimentale.

## Ce qui est vérifié

| Cas | Résultat |
| --- | --- |
| YOSBR 0.1.2, JAR publié inchangé | Copie des options par défaut au premier lancement ; conservation des options existantes au suivant |
| JAR de test original, compilé contre les signatures Minecraft 26.3 | Initialisation `main` puis `client`, ressources du JAR, identité du thread, événements de tick, santé et temps natifs, message dans le chat |
| Monde réel du client Rust | Même test réussi avec serveur intégré sur son thread et avec simulation dans la frame ; sauvegarde et arrêt normal |
| Mod dont un événement échoue | Diagnostic et désactivation de ses callbacks ; le mod de test continue |
| API Minecraft absente | Erreur explicite ; aucune résolution vers les classes du jeu Java présentes dans les bibliothèques de test |
| Chemins Unicode contenant `;` | Chargement réussi |
| Sodium, Iris, Distant Horizons | **Non compatibles actuellement** : mixins et contrats de rendu manquants |

Les JAR Fabric 26.3 publiés de Sodium 0.9.3-beta.1, Iris 1.11.7 et Distant
Horizons 3.3.4 sont maintenant épinglés par SHA-512 pour des tests de composants
reproductibles. Le pont fournit les conteneurs de modules, leurs versions et
leurs ressources, ainsi que la partie valeur des identifiants Minecraft.
Ces ajouts font progresser l'initialisation de Sodium jusqu'à un accesseur de
mixin non transformé ; Distant Horizons atteint l'API `DeltaTracker` manquante.
Iris ne déclare pas d'initialiseur standard : sans mixins, il ne démarre pas.
**Aucun de ces résultats n'établit leur compatibilité en jeu.** Voir le
[rapport des composants](results/rusted-render-mod-components.json).

Le [rapport de validation](results/rusted-java-bridge-local.json) distingue le
JAR publié, le JAR de test et les scénarios réellement exécutés. Ces tests
locaux ont tourné sous Linux avec JDK 25 et Vulkan logiciel. Ils ne prouvent pas
encore le fonctionnement du pont dans une partie Windows ou sur Hypixel.

## Parcours du launcher en développement

Dans une instance **Rusted natif 26.3**, la page Mods propose **JAR Java
(expérimental)**. L'option est désactivée par défaut et ne change pas le moteur
de l'instance. Elle s'applique au prochain lancement.

Le launcher vérifie la présence du marqueur `java-mod-bridge.json` dans le
runtime, utilise le Java téléchargé et vérifié pendant la préparation de
l'instance, puis transmet les bibliothèques par un manifeste JSON. Aucun Java
global, script manuel ou commande à saisir n'est nécessaire. Les configurations
de l'instance sont copiées dans le profil natif seulement si son dossier de
configuration n'existe pas encore. L'écriture des options de fenêtre attend
l'exécution des initialisations Java pour permettre les réglages de premier
lancement de YOSBR.

L'import local et les téléchargements Modrinth partagent le contrôle de
métadonnées du lancement. Un JAR candidat peut encore échouer à l'exécution si
une API lui manque : « candidat » ne signifie pas « mod validé ».

## Périmètre actuel

Le pont comprend les phases Fabric `preLaunch`, `main`, `client`, les
constructeurs d'adaptateurs de langage, quelques informations du loader, les
événements de tick client et une petite vue de l'état Minecraft natif. Voir la
[liste des façades originales](../engine/crates/rusted-java/README.md).

Ne sont pas implémentés : mixins, access wideners, JAR imbriqués, références
`Classe::méthode`, remappage des anciens noms obfusqués, résolution complète
des dépendances Fabric, API Fabric complète, modification du monde depuis Java,
interfaces OpenGL/LWJGL, shaders Iris et cache Distant Horizons. Les ressources
des JAR exécutables ne sont pas encore fusionnées avec les packs du moteur.
Le code Java Forge, NeoForge et Quilt n'est pas pris en charge par ce pont.

La JVM vit jusqu'à la fin du processus ; changer les mods demande de redémarrer
le jeu. Les hooks d'arrêt Java et le rechargement à chaud ne sont pas intégrés.
Le code Java dispose des capacités habituelles de la JVM : ce parcours n'offre
pas le confinement du runtime WASM.

## Reproduire les contrôles

Avec Rust 1.99.1, Python 3 et `JAVA_HOME` vers un JDK 25 :

```sh
cargo +1.99.1 test --manifest-path engine/Cargo.toml --locked -p rusted-platform
cargo +1.99.1 build --manifest-path engine/Cargo.toml --locked -p rusted-java --example java_mod_probe
python scripts/test_java_mod_runtime.py --probe engine/target/debug/examples/java_mod_probe
python engine/tests/test_rusted_java_source_audit.py
```

Le test minimal compile le JAR de test contre nos façades. Pour vérifier les
signatures réelles, ajouter `--minecraft-jar CHEMIN/client-26.3.jar` et
`--libraries CHEMIN/libraries` avec une installation locale légitime. L'option
`--yosbr CHEMIN/yosbr-0.1.2.jar` vérifie son SHA-512 publié avant exécution et
teste les options au premier et au second lancement.

`scripts/test_native_java_game.py --help` décrit le test de deux mondes natifs.
Il exige le client Rust compilé, les ressources préparées localement, le JAR de
test généré et le JAR YOSBR. Il utilise des profils temporaires et aucun compte.
Aucun fichier Minecraft ni JAR tiers n'est publié dans ce dépôt.

Les valeurs `Identifier` et `Version` ont aussi été comparées en exécutant
les mêmes appels sur nos façades et sur les binaires de référence locaux :

```sh
python scripts/test_java_value_contracts.py --probe engine/target/debug/examples/java_mod_probe --java-home CHEMIN/JDK25 --minecraft-jar CHEMIN/client-26.3.jar --libraries CHEMIN/libraries --fabric-loader CHEMIN/fabric-loader-0.19.5.jar
```

Le probe de composants se compile avec `--example mod_entrypoint_diagnostic`.
`scripts/probe_render_mod_entrypoints.py --help` décrit son pilote. Il vérifie
les empreintes de `horizon/render-mod-test-jars.json` et utilise un profil
temporaire par mod. Il ne passe pas les contrôles d'éligibilité du jeu, n'applique
pas les mixins et ne démarre aucun monde. Il retourne un échec de validation,
y compris lorsqu'un JAR n'a aucun initialiseur à appeler : ce n'est pas un
banc d'acceptation du rendu.

## Suite nécessaire pour les mods de rendu

Les prochains travaux portent sur les identités de classes et le remappage, les
contrats d'injection et les interfaces natives de rendu et de monde. Une JVM
capable d'appeler un initialiseur ne suffit pas à faire fonctionner les mixins
de Sodium, Iris ou Distant Horizons sur le renderer wgpu. Leur prise en charge
reste un chantier distinct qui demande des tests avec leurs JAR réels.
