# Étude et contrat technique

Objectif confirmé : modifier le Minecraft existant, conserver initialement sa
JVM et déplacer progressivement les composants internes en Rust. Le jeu, les
menus et les fonctionnalités ne sont pas remplacés par une imitation.

## Référence et sources

1.20.4 est retenue comme première référence : version fixe de Java Edition,
écosystème Fabric et mappings Yarn accessibles. Cela permet d'isoler une première
frontière technique sans porter immédiatement la simulation et l'interface.

Sources consultées le 28 octobre 2026 :

| Information | Source | État |
| --- | --- | --- |
| Classe de compression, champ et signature effacée | [Yarn PacketDeflater, commit fixé](https://github.com/FabricMC/yarn/blob/f7f320f7305c92be62c1cf96679b03c29bb3b87d/mappings/net/minecraft/network/handler/PacketDeflater.mapping) | Vérifié dans le binaire officiel puis exécuté sous Fabric |
| `class_2534`, `field_11636`, `encode(ChannelHandlerContext,Object,ByteBuf)` | Même fichier Yarn | Correspondances réelles, non inventées |
| Décompression et ses différences internes | [Yarn PacketInflater](https://github.com/FabricMC/yarn/blob/f7f320f7305c92be62c1cf96679b03c29bb3b87d/mappings/net/minecraft/network/handler/PacketInflater.mapping) | Documenté ; pas de remplacement de cette classe |
| Version Mixin de Fabric Loader 0.15.11 | [gradle.properties du chargeur](https://github.com/FabricMC/fabric-loader/blob/0.15.11/gradle.properties) | Documenté : `0.13.3+mixin.0.8.5` |
| Résolution des méthodes ciblées par Mixin | [InjectionInfo, Mixin 0.8.5](https://github.com/SpongePowered/Mixin/blob/0.8.5/src/main/java/org/spongepowered/asm/mixin/injection/struct/InjectionInfo.java) | Analyse statique puis injection réelle dans la classe Minecraft |
| ABI JNI, zlib Rust vers Java et Java vers Rust | Code et tests locaux | Vérifié sur Linux avec JVM réelle, `-Xcheck:jni` |
| Mixin, menus et fonctionnement | Classes Minecraft/Fabric + client officiel démo | Mixin et smoke test Linux vérifiés ; multijoueur authentifié non testé |

Les correspondances Yarn utilisent le namespace **intermediary de production
Fabric**, pas les noms de développement Loom. C'est un choix délibéré pour ce
petit adaptateur sans références directes aux classes du jeu dans le code Java.
La compilation utilise uniquement Mixin et Netty ; elle n'est pas une preuve que
la transformation du jeu réussit. `require=1` rend une cible manquante visible.

## Frontière choisie

Le mixin vise la méthode `encode` effacée documentée de `PacketDeflater` :

1. Garder le chemin Java si le module est désactivé ou la taille sous le seuil.
2. Copier les octets lisibles du `ByteBuf` sans changer ses indices.
3. Demander une compression zlib complète au backend Rust.
4. Après succès seulement, écrire la longueur décompressée en VarInt puis le
   flux zlib dans le buffer Netty, consommer l'entrée et annuler la méthode Java.
5. Si le natif échoue avant cette écriture, désactiver le backend et laisser
   Minecraft exécuter sa méthode d'origine.

Cette description est notre adaptation du format de compression réseau ; son
framing et les seuils normaux ont été vérifiés contre les classes originales,
y compris sur TCP. Les cas limites non couverts et le multijoueur authentifié
restent à tester ; les tests zlib seuls ne les établiraient pas.
La décompression native existe pour l'oracle de test mais n'est pas branchée
dans Minecraft. Aucun paquet d'authentification ni aucune règle du protocole de
jeu n'est réimplémenté.

## Propriété, ordre et limites

La JVM conserve les données du jeu. Chaque appel JNI reçoit une copie bornée ;
aucune référence Java, aucun `JNIEnv` et aucun buffer Netty ne traverse un thread.
L'appel est synchrone sur le thread Netty appelant : pas de réordonnancement des
paquets, de file Rust asynchrone ou de pool supplémentaire. La compression est
un calcul CPU court ; lui ajouter de l'async ne supprimerait pas son coût.

Un encodeur Rust est réutilisé par thread, réinitialisé entre les flux. Cela
élimine une allocation importante observée dans la première mesure. Les sorties
restent propres à chaque appel. Pas de verrou global. La limite native de 8 MiB
est **notre garde d'allocation**, pas une preuve de la limite exacte du jeu.
Au-delà, le mixin conserve le chemin Java. L'encodeur natif alloue au maximum
entrée + 1 % + 128 octets pour sa sortie ; s'il ne termine pas dans cette borne,
il renvoie une erreur. Le décodeur de test borne sa sortie à la taille déclarée
+ 1 et rejette les flux tronqués, les octets en trop et les checksums invalides.

Les copies JNI et les allocations restantes sont mesurées dans le benchmark.
Pas d'optimisation de simulation, de physique ou d'aléatoire Java : ces fonctions
restent exécutées par Minecraft, avec leur ordre et leurs particularités.

## Versions, données et extensions

Chaque version aura un adaptateur d'injection fixé, des capacités et une matrice
de validation. Les points d'entrée, le format réseau et les modèles de données
peuvent changer ; réutiliser une compression zlib n'établit pas la compatibilité
d'une version. Les familles anciennes, les snapshots et les transitions de
formats doivent être étudiés à partir de leurs artefacts propres.

Les sauvegardes Anvil/NBT, la génération, les ressources, le rendu, les commandes,
les datapacks et les menus restent actuellement dans le jeu original. Leur
présence vient de Minecraft ; aucune implémentation Rust de ces fonctions n'est
revendiquée. La compatibilité des mods dépend aussi du chargeur et de leurs
injections : Fabric seulement est ciblé ici, Forge/NeoForge et les conflits de
mixins restent à étudier. La JVM est explicitement requise.

## Pipeline reproductible d'analyse

`scripts/prepare_minecraft.py` obtient le manifeste et les fichiers officiels,
vérifie leurs empreintes, puis extrait uniquement les entrées du bundle dont
le SHA-256 est vérifié. Il conserve la provenance localement. Les bibliothèques
et mappings Fabric sont fixés par version et vérifiés. L'inspection bytecode et
les essais `scripts/test_minecraft.py` confirment ensuite le point d'injection.
Les fichiers propriétaires restent sous `build/`, ignoré par Git.

La suite est la mesure du hook en partie multijoueur authentifiée et sous
Windows, puis le choix du prochain composant à porter selon ses coûts et
contrats. Les données, hooks et observations doivent rester propres à chaque
version ; ni les mappings ni le pipeline ne réalisent une conversion automatique.

## Mise à jour : intégration réellement exécutée

Le mixin de production est maintenant chargé dans les classes Minecraft
remappées par Fabric. `javap` sur le binaire officiel confirme le descripteur
`encode(ChannelHandlerContext,Object,ByteBuf)` et les constructeurs utilisés.
Les classes `uf`/`ue` sont identiques dans le client et le serveur 1.20.4.
Les trois modes activé/désactivé/natif absent passent chacun 301 cas, dont 24
paquets TCP ; le mode activé compte exactement 189 appels Rust attendus.
Le changement du seuil sur l'objet Minecraft lui-même est observé correctement.

Le client officiel a aussi fonctionné en démo Linux avec ses menus et un scénario
solo. La sauvegarde Anvil et la génération restent du code Minecraft Java, pas
des ports Rust. Voir `VALIDATION.md` pour distinguer les tests du codec, la
vérification visuelle, le multijoueur authentifié non testé et Windows.

La prochaine étape du pipeline d'analyse (fichiers officiels, empreintes,
inspection bytecode, premier test d'injection) est donc terminée pour ce codec.
Les expérimentations ultérieures doivent garder leur provenance et comparer
les mêmes artefacts. Le pipeline ne convertit pas automatiquement le jeu.
