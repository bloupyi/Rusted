# Shaderpacks dans Rusted natif

Le moteur possède un lecteur natif pour les packs `.hshader` et pour une partie du format
GLSL `shaders/` utilisé par les shaderpacks classiques. Un seul pack est actif à la fois.

1. Dans les options du jeu, ouvre **Shader Packs** puis **Open Pack Folder**.
2. Copie ton shaderpack `.zip` dans le dossier ouvert.
3. Reviens à la page Shader Packs et sélectionne le pack.

Le fichier à installer dans ce parcours natif est le shaderpack. Le mod Iris reste une
extension Java/Fabric ; son JAR et ses API ne sont pas exécutés par Rusted natif.
La compatibilité dépend des fonctionnalités utilisées par chaque pack. Le journal du jeu
indique les programmes chargés et les fonctionnalités refusées.

Le test original `post-test`, déjà présent dans les sources du moteur, a été vérifié en
partie avec les trois passes `composite`, `composite1` et `final` sous Vulkan logiciel.
Ce résultat ne valide pas automatiquement les packs tiers.

[Capture du test](screenshots/rusted-native-0.3.6-glsl-test.png) ·
[Résultats des vérifications](results/rusted-native-0.3.6.json)
