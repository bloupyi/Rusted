# Chargement des packs : copies Windows et cache

Le journal reçu pour Hypixel SkyBlock mesure **240,019 s de construction de
l'overlay**, puis 24,268 s pour les modèles/textures, 5,834 s pour l'interface,
11,815 s pour les objets et 0,070 s pour les mipmaps. La priorité est donc la
construction de l'arborescence de ressources, avant le GPU.

Deux changements ciblent cette étape :

- Sous Windows, les dossiers vanilla intacts utilisent des jonctions NTFS,
  sans élévation. Les fichiers isolés utilisent des liens physiques lorsque
  c'est possible. Si le système de fichiers refuse ces liens, le moteur
  conserve le repli sur une copie complète. Les dossiers concernés par des
  remplacements ou filtres sont parcourus pour respecter l'ordre des packs.
- Réutiliser un téléchargement vérifié ne modifie plus la date du ZIP. La date
  d'utilisation est conservée dans un fichier `.last-used`, pour garder le bon
  ordre de nettoyage sans invalider l'overlay. Les marqueurs ne comptent pas
  dans la limite de vingt packs et sont supprimés avec leurs packs.

Le format de cache passe à 5 pour reconstruire les anciens overlays Windows
matérialisés. Les tests vérifient les remplacements, filtres, changements de
contenu, accès concurrents, réutilisation à la reconnexion et nettoyage. Un
test Windows spécifique vérifie que supprimer ou reconstruire l'overlay ne
supprime ni ne remplace les fichiers vanilla derrière une jonction.

Le banc Linux utilise un pack original de 5 000 fichiers, **pas le pack
Hypixel**. Il confirme la réutilisation du cache et mesure séparément les
étapes CPU. Il ne mesure ni le temps gagné sur le PC du joueur, ni le temps du
premier chargement Windows. Les tests de cache et de téléchargement passent
aussi sur les runners Windows réels (Actions 41609103593), notamment la
suppression des jonctions sans toucher aux fichiers vanilla. Aucun gain chiffré
de chargement Windows n'est encore mesuré.

## Comparaison complète avec Java

Sur le même ZIP original de 5 000 fichiers, à l'écran titre, avec des profils
neufs et les deux clients en Vulkan logiciel sur le même banc Linux :

| Rechargement complet | Rusted natif | Minecraft Java 26.3 |
| --- | ---: | ---: |
| Première activation | 1,409 s | 3,972 s |
| Médiane des deux rechargements suivants | 1,248 s | 3,226 s |

Ces mesures comprennent les ressources CPU et leur application GPU, pas
seulement l'overlay. Le téléchargement et le démarrage initial du jeu sont
exclus. Les caches du système d'exploitation n'ont pas été vidés. Elles
**ne garantissent pas ces temps pour Hypixel ni pour un PC Windows** : les
images, modèles et polices du pack réel peuvent changer le coût du rechargement.
Le [rapport](results/rusted-resource-reload-comparison.json) conserve les
échantillons, l'empreinte du ZIP et l'environnement.

`scripts/compare_resource_reload.py --help` décrit le banc reproductible. Il
utilise un JDK 25 et une installation Minecraft locale. Un agent de test
original instrumente le tick Java avec l'API Classfile du JDK, sélectionne le
pack et mesure la fin de `Minecraft.reloadResourcePacks()`. Le contrôle
développeur `resource-pack ID` mesure les deux phases du client natif. Aucun
compte, JAR Minecraft ou fichier du pack Hypixel n'est publié.

Les huit définitions d'objets Hypixel refusées dans le journal ne sont pas
corrigées par cette optimisation. Le prochain journal indiquera la raison de
chaque erreur JSON, au lieu de supprimer ce détail ; les fichiers concernés
restent nécessaires pour reproduire leur format exact.
