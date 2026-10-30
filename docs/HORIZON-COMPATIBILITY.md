# Compatibilité de Rusted 0.3.6

Le moteur natif reste dérivé de Horizon MIT. Notre démarrage automatique, protocole
de session et adaptation aux imports sont des composants originaux Rusted, pas une
réécriture du renderer ni une implémentation des API Java de Minecraft/Fabric.

| Fonction | État livré |
|---|---|
| Import complet Prism | Fabric et bootstrap inclus, Wrapper vide ; importer puis Jouer |
| Import local Modrinth | dépendance Fabric et bootstrap inclus dans overrides ; aucun Wrapper |
| Démarrage natif | bootstrap et modules passifs compatibles, session par pipe, données séparées |
| Modules passifs Fabric/Quilt/lowcode | ressources et données prises en charge chargées directement depuis les JAR |
| Mods Fabric avec code | le bootstrap rend la main au jeu Java d'origine en mode auto |
| Sodium/Iris/Distant Horizons dans Rust | aucun JAR exécuté par le moteur natif |
| API Java Forge/NeoForge/Quilt | non prises en charge ; les modules passifs restent un cas distinct |
| Mods natifs .hmod | système hérité disponible ; Far View n'est pas Distant Horizons |
| Shader WGSL Rusted Soft Light | original, optionnel, API de stages héritée |
| Format GLSL natif | lecteur partiel ; test original à trois passes vérifié, packs tiers à valider |

Le bootstrap ne transforme pas les Mixins des mods en code Rust. Le mode Java
conserve les classes Minecraft et leur chargeur d'origine ; le moteur Rust est
alors inactif. Aucun port de Sodium, Iris ou DH n'est annoncé.

L'essai Java isolé de référence a chargé Fabric 0.20.1, Sodium **0.9.2**, Iris
**1.11.7** et DH **3.3.4** pour Minecraft **26.3** (55 mods avec dépendances
intégrées). Sodium 0.9.2 correspond à la dépendance explicite d'Iris. Le rendu a
ensuite échoué : visual GLX indisponible dans l'écran virtuel, puis absence du
contexte OpenGL attendu par Iris au repli Vulkan. Aucune partie avec les trois
mods n'est donc certifiée. Ce résultat ne prédit pas celui du matériel Windows.

Le test 0.3.5 du démarrage automatique emploie le vrai Fabric et notre bootstrap :
il atteint le menu natif dans un bac à sable sans réseau, avec une session fictive.
Les exécutables Windows et l'arrêt forcé sont testés sous Wine. L'import depuis
les interfaces réelles des lanceurs sous Windows reste à confirmer.

Aucun code source de Sodium, Iris, Distant Horizons ou d'un autre shader n'a été lu,
copié ou traduit. Les binaires des trois mods ont uniquement servi à l'essai Java
isolé précédent et ne sont pas redistribués. Les licences et crédits MIT du moteur
Horizon restent conservés. Voir KNOWN-ISSUES.md pour le travail restant.

Voir [les résultats natifs 0.3.6](results/rusted-native-0.3.6.json) et [les shaderpacks](NATIVE-SHADERPACKS.md).
