# Compatibilité des mods — moteur Rusted 0.3.6 / lanceur 0.2.5

| Type | Rusted natif | Java / Fabric |
| --- | --- | --- |
| Mods natifs `.hmod` | API WASM ; activation et options dans le jeu | Sans objet |
| Fabric et Quilt sans code | Ressources et données prises en charge, chargées directement depuis `mods/*.jar` | Chargeur Java classique |
| Forge et NeoForge `lowcodefml` sans dépendances | Même chargement natif des ressources et données prises en charge | Chargeurs non intégrés au lanceur |
| Sodium | Code Java et mixins non portés ; renderer natif indépendant | Installation disponible ; partie réelle avec compte à valider |
| Iris | JAR non exécutable nativement ; les lecteurs natifs GLSL et WGSL sont distincts | Installation disponible ; partie réelle avec compte à valider |
| Distant Horizons | JAR et cache DH non portés ; LOD natif indépendant | Installation disponible ; partie réelle avec compte à valider |
| Mods avec code Forge, NeoForge ou Quilt | Portage natif nécessaire | Chargeurs non intégrés au lanceur |

Les modules passifs restent des JAR inchangés : pas de conversion en pack ni d'activation
manuelle d'un pack généré. Ils sont chargés au prochain lancement du moteur, sous les packs
de ressources choisis par le joueur et ceux du serveur. Les données natives prennent en charge
les fonctions, tags de fonctions, prédicats et modificateurs d'objets ; les packs du monde
peuvent les remplacer. Les recettes et les tables de butin ne sont pas encore prises en charge
par cet import de modules.

L'import et le lancement utilisent le même vérificateur. Les classes Java, mixins, hooks,
archives imbriquées, dépendances Java supplémentaires, données non prises en charge et
identifiants dupliqués sont refusés avec un diagnostic. Installer un mod incompatible depuis
Modrinth conserve le moteur de l'instance et n'installe aucun JAR du téléchargement refusé.
Les archives placées manuellement dans `mods` sont soumises aux mêmes contrôles au lancement.
Dans un pack Prism/Modrinth en mode automatique, les modules passifs compatibles conservent
le moteur natif ; les mods avec code conservent le parcours Java/Fabric existant.

Cette compatibilité ne fournit pas l'API Fabric ni les injections Java/OpenGL. Faire fonctionner
les JAR de Sodium, Iris et Distant Horizons dans Rusted demande encore des interfaces natives
et un travail sur leurs contrats de rendu, de monde et de threads. Aucun code source de ces
projets n'a été lu ou réutilisé.
