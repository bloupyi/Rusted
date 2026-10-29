# Démarrage automatique Rusted

Code original, limité au point d'entrée public `PreLaunchEntrypoint` de Fabric Loader.
Aucune référence aux classes Minecraft, aucun Mixin, aucune implémentation de Fabric.
Le module lance notre adaptateur Rust et lui transmet les arguments sur une pipe privée,
puis attend la fin du jeu. Une trame de contrôle distincte du code de sortie demande de poursuivre la JVM lorsque
le mode Java ou des mods Java supplémentaires sont sélectionnés. Le moteur natif n'est
pas utilisé dans ce dernier cas. Il ne modifie aucun réglage de Prism ou Modrinth.

La JVM du lanceur, conservée depuis le début du projet, sert ici au démarrage ; le moteur
natif et la validation de la session restent dans Rust. Aucun JAR tiers n'est repackagé.
Les anciennes instructions upstream « everything players run is Rust » ne décrivent
pas ce bootstrap original, rendu nécessaire par le parcours d'import demandé.
