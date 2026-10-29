# Résultats vérifiés — 28 octobre 2026

Le projet modifie **le Minecraft existant** : JVM, interface, gameplay et
sauvegardes d'origine conservés. Le composant porté en Rust est actuellement
l'encodage zlib des paquets sortants. Le backend complet n'est pas porté.

## Bibliothèque native

- Trois tests Rust passent : tailles jusqu'à 8 MiB, flux invalides/checksums/bornes
  et isolation de l'encodeur sur huit threads.
- La vraie JVM charge le natif depuis le JAR, vérifie son SHA-256 et son ABI.
- Sous `-Xcheck:jni` : 18 cas bidirectionnels Java/Rust, 400 appels concurrents
  et 53 assertions passent.
- Bibliothèque absente : trois assertions de retour Java passent. Empreinte
  embarquée volontairement invalide : quatre assertions passent ; le natif
  altéré est rejeté avant chargement.
- `cargo fmt --all -- --check`, Clippy avec avertissements interdits et le
  script complet `bash scripts/setup-cloud.sh` ont réussi.

## Branchement sur les vraies classes Minecraft

Les fichiers serveur officiels 1.20.4 sont vérifiés contre les empreintes du
manifeste Mojang. Les JAR internes du bundle sont vérifiés en SHA-256 ; les
bibliothèques Fabric le sont également. Le mod de test charge les classes
Minecraft remappées par Fabric et le **mixin de production**, sans classe factice.

| Mode | Cas réussis | Dont paquets TCP locaux | Appels Rust observés |
| --- | ---: | ---: | ---: |
| Rust activé | 301 | 24 | 189 |
| Rust désactivé | 301 | 24 | 0 |
| Bibliothèque native absente | 301 | 24 | 0 |

Les vérifications couvrent le contenu décompressé par le décodeur **original**,
la longueur VarInt, la consommation des buffers, cinq seuils de compression,
leur modification à l'exécution, des tailles allant jusqu'à 8 MiB et quatre
threads indépendants. Les 24 paquets TCP par mode traversent une vraie socket
locale avec des écritures fragmentées ; contenu et ordre sont vérifiés.

Ce sont des essais du codec réseau, **pas des connexions multijoueur
Minecraft authentifiées**. Le mod de test s'arrête avant le démarrage du serveur
ou d'un monde ; il n'accepte aucune EULA et ne modifie aucun contrôle d'accès.

Inspection du binaire officiel : `PacketDeflater` correspond à `uf`, et
`PacketInflater` à `ue`. Leurs classes sont identiques octet par octet dans les
JAR client et serveur 1.20.4. Le descripteur effacé ciblé par le mixin existe bien.
Rapports : `results/minecraft-network/{enabled,disabled,missing}.json`.

## Vrai client, interface et partie de démonstration

Le client officiel et ses **3 788 objets de ressources uniques** ont été
obtenus sur les serveurs officiels et vérifiés, dans `build/minecraft/` ignoré
par Git. Environ 657 Mo de ressources ; aucun de ces fichiers n'est distribué
dans le module ou dans l'archive Prism.

Le client original a été lancé sous Fabric, avec `rusted_backend 0.1.0` chargé,
en mode **démo officiel**, sans fabrication de compte ni retrait des restrictions
de la démo. Les captures ont été examinées visuellement et les logs du jeu
consultés. Scénario réellement exécuté :

1. Menu principal, options, réglages vidéo, contrôles et inventaire d'origine.
2. Création du monde de démonstration et déplacements du joueur.
3. Destruction d'un bloc de sol, avec trou et objet de terre visibles.
4. Sauvegarde et sortie normales ; arrêt complet puis redémarrage du processus.
5. Relecture du monde : position `(-45.5, 64.0, 55.95519899083406)` restaurée,
   trou et objet conservés. La position apparaît aussi dans le log de connexion
   locale du serveur intégré.
6. Ramassage de la terre, saut et replacement du bloc ; trou rempli et objet
   consommé dans la barre d'inventaire. Nouvelle sauvegarde et sortie normales.
7. Lancement séparé avec le backend désactivé : le menu d'origine s'affiche
   également et le processus quitte avec le code zéro. La conservation des logs
   par lancement dans `rusted-runs/` est vérifiée.

Captures et rapport : `results/minecraft-integration.json`, fichiers PNG sous
`build/client-runs/1793192996522892510/`. Ce scénario est un **smoke test visuel**,
pas une suite exhaustive ou une preuve que tout le gameplay a été porté en Rust.
Le gameplay et le stockage restent implémentés par Minecraft Java. La connexion
locale du serveur intégré ne teste pas le protocole d'authentification Internet.

Environnement de ces essais : Linux/Debian, cinq CPU Intel Xeon Platinum 8573C
visibles, 33 GiB de RAM visibles, OpenJDK 21.0.12.1, rendu Mesa llvmpipe logiciel,
1280×720, distance de rendu 4 et simulation 5. Le premier démarrage n'avait pas
de périphérique audio ; au redémarrage, OpenAL a démarré avec le pilote `null`
« No Output ». **Le son audible n'est pas validé**. Les accès de compte/Realms
échouent dans cette instance de développement sans compte connecté ; ils ne sont
pas présentés comme fonctionnels.

## Windows et distribution

La DLL GNU Windows x86_64 est compilée et ses trois exports JNI inspectés. Le
JAR de production contient Linux et Windows ; son CRC ZIP, le mixin et les
empreintes des bibliothèques natives sont vérifiés. Son contenu de classes,
binaires et licences reste identique au JAR ayant subi les essais Minecraft.

L'archive `build/Rusted-1.20.4-native-test.zip` contient le module et une
configuration Prism/MultiMC fixée à Minecraft 1.20.4 et Fabric 0.15.11. Sa
structure et son intégrité sont vérifiées. **Import dans Prism et exécution sous
Windows non effectués**. La CI Windows est définie, mais n'a pas été déclenchée.

## Mesures et limites

Les mesures précédentes du codec, sur un hôte AMD EPYC avant publication de
l'environnement, restent dans `results/benchmark.json` et `BENCHMARK.md`.
Elles sont contrastées ; aucun gain global du jeu n'est établi. Les FPS observés
pendant le smoke test sur rendu logiciel ne constituent pas une comparaison.
Pas de mesure comparative du démarrage, des ticks, des chunks, des sauvegardes,
du débit multijoueur ou de la montée en charge du jeu.

Non exécutés : multijoueur avec comptes authentifiés, Windows réel, autres
chargeurs, conflits avec d'autres mods, versions supplémentaires, tests exhaustifs
de redstone/recettes/commandes/datapacks/resource packs. La version 1.20.4 n'est
pas marquée intégralement validée.

## Reproduire

```sh
source scripts/cloud-env.sh  # cloud Linux uniquement
bash scripts/setup-cloud.sh
python3 scripts/test_minecraft.py
python3 scripts/prepare_client.py
python3 scripts/launch_demo.py
```

Pour le client sous affichage virtuel : voir `UI-SCENARIO.md`. Les fichiers
propriétaires restent dans le cache local ignoré. Les sorties JSON enregistrées
sous `docs/results/` décrivent cette session et ne certifient pas les futurs
changements.
