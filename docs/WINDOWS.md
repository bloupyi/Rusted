# Essayer Rusted sous Windows

Pour **Windows x86_64 (Intel/AMD)**, avec Java 17 ou 21 **64 bits**.
La DLL Windows est déjà dans le module : aucune installation de Rust, Python ou
Visual Studio n'est nécessaire pour essayer l'archive construite.

## Avec Prism Launcher

1. Installer Prism Launcher depuis son site officiel :
   https://prismlauncher.org/download/windows/
2. Dans Prism, ajouter son compte Microsoft possédant Minecraft Java Edition par
   le parcours de connexion habituel. Ne jamais saisir de token dans un script
   ou dans cette conversation.
3. Choisir **Ajouter une instance**, puis **Importer** et sélectionner
   `Rusted-Windows-1.20.4.zip`. Ne pas simplement extraire le ZIP dans `.minecraft`.
4. Choisir Java 17 ou 21 en 64 bits si Prism le demande, puis lancer l'instance.

L'instance fixe Minecraft à **1.20.4**, Fabric Loader à **0.15.11**, et active le
module avec `-Drusted.enabled=true`. Le jeu et les ressources sont obtenus par
le lanceur normal. Créer un monde de test neuf pour commencer.

## Avec une installation Fabric existante

Dans un **profil de test séparé** Minecraft 1.20.4 / Fabric 0.15.11 :

1. Copier `rusted-backend-0.1.0-mc1.20.4.jar` dans le dossier `mods` de ce profil.
2. Ajouter `-Drusted.enabled=true` aux arguments JVM du profil.
3. Lancer normalement avec son compte autorisé et Java 64 bits.

La DLL se charge automatiquement depuis le JAR ; ne pas la copier dans Windows
ou dans `System32`. Les menus restent ceux de Minecraft.

## Ce qui est prêt et ce qui reste à vérifier

Le paquet Windows et la DLL x86_64 sont construits ; leur intégrité et leurs
exports JNI sont vérifiés. **L'exécution sous Windows n'a pas encore été testée**
dans cet environnement cloud Linux. L'import dans Prism reste également à vérifier.

Le composant actuellement porté en Rust est **la compression des paquets réseau
sortants**. Le reste du jeu demeure Java. Le solo utilise une connexion locale
qui peut ne jamais solliciter cet encodeur : son fonctionnement n'est donc pas
prouvé par le seul lancement d'un monde solo.

Lors d'une connexion de test avec compression activée, les logs doivent contenir
`Rust backend JNI loaded` puis `First native encode call`. Le multijoueur
authentifié reste à valider. Si le natif ne se charge pas, le module garde le
chemin Java et écrit `Rust backend unavailable` avec le détail de l'erreur.

Dans Prism, la console de l'instance et le dossier `logs` du jeu contiennent les
informations utiles. Pour comparer, enlever `-Drusted.enabled=true` conserve
l'encodage Java. Aucun gain de performances global n'est établi à ce stade.
