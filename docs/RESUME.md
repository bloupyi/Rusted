# Priorité — connexion Microsoft et sources 0.2.2

L’utilisateur veut une connexion Microsoft classique sans saisir d’ID. Le champ
avancé et le guide technique ont été retirés du parcours joueur. Le client public
est maintenant une configuration de distribution, lue à chaque démarrage et non
sauvegardée dans les réglages joueur. Les anciennes préférences sont acceptées mais
leur ancien microsoft_client_id est ignoré. Une identité absente désactive le bouton
et ne programme pas un lancement impossible en attente.

La version source est 0.2.2 ; aucun nouveau ZIP jouable n’est produit sans application
Microsoft enregistrée/configurée. Le packager refuse par défaut une configuration
vide/mal formée ; --allow-unconfigured-auth ne produit qu’un -preview.zip. La CI de
distribution attend vars.RUSTED_MICROSOFT_CLIENT_ID, puis conserve les contrôles
Defender. Les tests de compilation et de communication restent indépendants.
L’environnement n’a aucune identité Azure pour créer l’application ; les droits
GitHub fournis ne permettent pas de lire/gérer les variables Actions (HTTP 403).
Une inscription unique côté éditeur reste requise, sans secret client. Ne pas
réutiliser l’identité d’un autre lanceur ou annoncer la connexion comme fonctionnelle.
Le ZIP 0.2.1 historique reste inchangé pour conserver ses preuves Defender.

Tests 0.2.2 : 38 tests Rust Linux et 3 tests Python du garde de distribution passent ;
Clippy Linux/Windows cross passe. Deux parcours UI sous Wine confirment l’absence
des contrôles d’ID et l’activation du bouton uniquement avec configuration fournie.
Rapport : results/rusted-launcher-0.2.2-auth.json. Pas de compte réel testé et pas
de nouvelle validation Defender pour 0.2.2 ; l’archive 0.2.1 demeure inchangée.

CI 0.2.2 réussie : https://github.com/bloupyi/Rusted/actions/runs/40949763727
(commit a4b9780). Tests Windows réels et Linux, Clippy, passerelle, outils et parseur
réussis ; job de distribution Windows ignoré car l’identité éditeur manque.
Aucun nouveau binaire distribuable n’est produit.

# Travail actuel — 0.2.1 et tests Windows

Le propriétaire autorise la publication des sources et les tests sur GitHub,
et demande de prendre ensuite ces initiatives sans reconfirmation. Les boutons
manuels de préchargement sont supprimés ; Jouer les remplace dans l'accueil,
sans doublon dans le pied de page, et attend la préparation automatique si besoin.
La mise en attente est annulée en cas d'erreur, annulation ou changement d'instance.

Le signalement Defender 0.2.0 reste ouvert : ne pas considérer une compilation
ou Wine comme une validation antivirus et ne pas demander d'exclusion/restauration.
Sources publiées sur https://github.com/bloupyi/Rusted, branche master. Tests Linux
et Windows de communication, unitaires, outils et parseur Prism réussis.
La compilation complète Windows et le contrôle Defender de 0.2.1 ont réussi ici :
https://github.com/bloupyi/Rusted/actions/runs/40907164336 (commit 4c1eb72).
Les cinq jobs sont au vert. Le ZIP exact a passé le scan et le premier démarrage
de 20 secondes avec protection en temps réel et comportementale active.
Rapport : results/rusted-launcher-0.2.1-windows.json, signatures 1.463.24.0.
Livrables complets : build/windows-release-0.2.1/. Cette validation ne couvre pas
de compte réel, de préparation entière ou de partie et ne prouve pas la cause de
la détection 0.2.0. Les exécutables restent non signés Authenticode.

# Priorité actuelle : blocage Microsoft Defender 0.2.0

Le propriétaire signale Behavior:Win32/DefenseEvasion.A!ml sur Rusted Launcher.exe,
mis en quarantaine. Ne pas proposer de restaurer/exclure le fichier ni considérer
le faux positif comme établi. Aucun nouvel EXE corrigé n'a été produit. L'archive
0.2.0 originale est préservée avec son SHA-256 pour l'analyse ; les empreintes sont
conformes au manifeste local, mais aucun test Defender n'est possible sous Linux.
Le workflow Windows vérifie MSVC puis ajoute scan et observation de démarrage avec
Defender actif avant tout upload. Il est maintenant publié et en cours d'exécution
sur une machine Windows réelle ; la validation de 0.2.1 est réussie. Voir
WINDOWS-SECURITY.md et le rapport JSON. Le signalement de l'ancien binaire reste ouvert.

# État actuel — Rusted Launcher 0.2.0

ZIP complet Windows : Rusted-Launcher-Windows-0.2.0.zip. Nouveau catalogue Mojang
63 versions sans obfuscation (5 stables + 58 autres, dont 26w14a), disponible hors
ligne et actualisable. Java est choisi selon les métadonnées ; natif limité à 26.3.
Préparation automatique des créations/imports ; pool HTTPS partagé et 24 tâches.
Interface cuivre avec sélecteurs personnalisés et cartes de mods. Login Microsoft
indépendant du préchargement, navigateur ouvert et code copié automatiquement ;
le mainteneur doit encore fournir son propre ID public d'application, configurable
une seule fois via launcher-config.json avant distribution.

Le logo est maintenant le PNG exact de https://i.ibb.co/6Rnd09fY/model.png, avec
ses bords noirs et son alpha ; aucune retouche ni suppression de pixels. Le moteur
Windows a été recompilé pour mettre à jour son icône aussi. Son code de jeu reste
celui du fork Horizon 0.3.5 ; les nouveaux travaux ne constituent pas une réécriture.

La phase compatibilité native commence avec un adaptateur strict de mods Fabric
sans code, limité aux textures vanilla et langues, vers les packs de ressources
natifs. Le mod de test original s'affiche réellement dans le menu du moteur, sans
Java. Sodium/Iris/Distant Horizons exigent toujours Java/Fabric ; ni leur exécution
native ni des packs GLSL ne sont annoncés compatibles.

# État actuel — Rusted Launcher 0.1.0 + moteur 0.3.5

Le nouveau livrable principal est **Rusted-Launcher-Windows-0.1.0.zip** : lanceur
Windows original avec interface cuivre, instances, téléchargement vérifié/cache partagé,
préparation native avant jeu, Java automatique, connexion Microsoft configurable et
sessions DPAPI, mods Modrinth Fabric, console, import Prism/mrpack et export Prism.
Extraire puis ouvrir Rusted Launcher.exe. Aucun configurateur séparé n'est requis.
L'utilisateur n'a pas encore d'application Microsoft ; sa configuration et le guide
sont inclus. Aucun identifiant de lanceur tiers n'est réutilisé. Une connexion réelle
et une partie Windows restent à valider après cet enregistrement.

Voir [le guide du lanceur](RUSTED-LAUNCHER.md), [Microsoft](MICROSOFT-SETUP.md) et
[les preuves](results/rusted-launcher-0.1.0.json). Les options principales sont présentes,
mais la parité complète avec Prism/Minecraft Launcher reste ouverte. Ne pas présenter
le moteur hérité comme une réécriture ni les JAR comme compatibles avec le mode natif.

# État de Rusted 0.3.5

Livrables prioritaires : packs complets Prism ZIP et Modrinth .mrpack. Importer puis
Jouer, sans Configurer-Rusted.exe ni Wrapper. Fabric Loader 0.20.1 charge notre
bootstrap Java original, qui parle au processus Rust par pipe privée. La JVM du
lanceur est conservée ; le moteur natif reste Rust. Avec d'autres JAR, auto rend la
main au Java/Fabric d'origine. Aucun JAR Minecraft ou tiers n'est distribué.

Corrections : lancement indépendant du chemin d'import, gestion arrêt JVM/natif,
empreinte du bootstrap, nom du profil Fabric normalisé selon la version Minecraft,
identité des logs renderer et détection GPU unique. Pack original WGSL Rusted Soft
Light inclus, désactivé par défaut, sur l'API native héritée d'Horizon.

Le moteur/renderer reste dérivé de Horizon MIT ; ne jamais présenter l'identité
corrigée comme une réécriture. Pas d'exécution native de Sodium/Iris/DH, pas de
mélange universel de loaders ni de multijoueur authentifié certifié. Voir KNOWN-ISSUES.md.
