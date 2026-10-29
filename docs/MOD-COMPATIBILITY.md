# Compatibilité des mods — Rusted 0.2.0

| Type | Moteur natif Rusted | Java / Fabric |
| --- | --- | --- |
| Mods natifs .hmod | API WASM héritée, options dans le jeu | Sans objet |
| Fabric sans code : textures vanilla et langues | Premier adaptateur opérationnel, pack 97.1 à activer dans le jeu | Chargement Fabric classique |
| Sodium | JAR et hooks Java non portés | Installation par Modrinth disponible ; partie réelle à valider |
| Iris | JAR et shaders GLSL non portés ; le shader natif WGSL reste distinct | Installation par Modrinth disponible ; partie réelle à valider |
| Distant Horizons | Code Java, génération LOD et cache DH non portés | Installation par Modrinth disponible ; partie réelle à valider |
| Forge, NeoForge, Quilt | Absents | Non intégrés à ce lanceur pour le moment |

Le premier adaptateur lit seulement les métadonnées et fichiers de ressources d'un
JAR Fabric, sans exécuter ses classes. Il refuse code, mixins, JAR imbriqués, gameplay,
API Java obligatoires et ressources non prises en charge. Il laisse le fichier source
intact et conserve le moteur natif. Un mod de langue de test original a réellement
changé le texte du menu du jeu natif sans JVM ; le test et sa capture sont consignés
dans docs/results/rusted-launcher-0.2.0.json.

C'est une étape de compatibilité de ressources, pas une émulation de Fabric. La suite
nécessite des API natives pour les fonctionnalités des mods, ou une passerelle JVM/Rust
avec des contrats de monde, rendu et threads. Les injections Java et OpenGL ne deviennent
pas compatibles avec le renderer Rust/wgpu en chargeant simplement leurs JAR.
Aucun code source de Sodium, Iris ou Distant Horizons n'a été lu ou réutilisé.
