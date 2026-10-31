# État actuel — moteur 0.3.6 / lanceur 0.2.5

Corrigés et vérifiés localement : découverte des modèles d'objets imbriqués, textures
d'atlas hors des dossiers standard, damier de remplacement, styles des noms conservés
sur le réseau et règles d'équipe des joueurs distants. Un pack original de 16 387 fichiers
est chargé depuis un serveur vanilla local en environ 4,5 s sur le banc de test.
Le test à deux clients respecte `collisionRule never` et rétablit la poussée avec `always`.
Ces mesures ne décrivent pas le pack Hypixel ou les performances du PC du joueur.

Le catalogue public SkyBlock confirme 1 247 objets avec un modèle dans un chemin
imbriqué `hypixel_skyblock:…`. Un deuxième pack obligatoire original reprend ses
1 195 identifiants distincts, avec nos propres modèles et textures : 17 582 fichiers,
environ 5 s sur le banc Linux, objet visible dans la barre et en main. Ce test ne
contient pas le pack Hypixel et ne valide pas une connexion au serveur Hypixel.

Les modules passifs de ressources/données sont chargés automatiquement, dans les limites
[documentées](MOD-COMPATIBILITY.md). Les JAR Java de Sodium, Iris et Distant Horizons,
les API Java des autres loaders et le mélange universel de leurs injections restent à porter.
Le lecteur GLSL natif existant passe le test original à trois passes ; chaque pack tiers
reste à vérifier. Hypixel SkyBlock avec une session authentifiée reste à valider.

La compilation MSVC, les cinq jobs CI, le scan Defender actif, 40 redimensionnements,
le code Microsoft visible et son annulation sans compte passent sur Windows réel.
Une nouvelle connexion est disponible après annulation. Les contrôles portent sur
le ZIP exact de [ce rapport](results/rusted-launcher-0.2.5-windows.json) ; une préparation
entière et une partie avec un compte réel restent à vérifier.

[Résultats et portée des tests](results/rusted-native-0.3.6.json).
Les sections suivantes conservent l'historique des versions précédentes.

# Connexion Microsoft — Rusted Launcher 0.2.3

Le blocage d’enregistrement imposé dans 0.2.2 est levé : comme la bibliothèque msmc
utilisée par FaultlineLauncher, Rusted fournit un client public Live par défaut.
La demande de code Microsoft réelle et sa réponse authorization_pending ont été
vérifiées sans compte. Les points d’entrée Live sont utilisés aussi pour le
renouvellement ; les applications Entra personnalisées restent facultatives.
Aucun champ d’ID dans Comptes, aucune configuration éditeur requise pour ce défaut.
Voir [le détail de la comparaison](MICROSOFT-SETUP.md).

La connexion complète avec un vrai compte, les vérifications licence/profil et une
partie doivent encore être validées. Les cinq jobs de la CI 0.2.3 passent, dont la demande de code Microsoft sur
Windows, le scan Defender et le démarrage de 20 secondes, protections actives
(signatures 1.463.30.0). Voir le [rapport exact](results/rusted-launcher-0.2.3-windows.json).
Le client utilisé est partagé, pas une inscription Microsoft propre à Rusted.

# Blocage Windows signalé — Rusted Launcher 0.2.0

Defender détecte `Behavior:Win32/DefenseEvasion.A!ml` sur `Rusted Launcher.exe`.
La cause n'est pas confirmée et aucun correctif antivirus n'est validé. Les essais
Wine et les tests fonctionnels précédents ne couvraient pas Defender. Ne pas
contourner la quarantaine. Le pipeline Windows vérifie désormais Defender actif,
scanne l'archive et observe le démarrage isolé avant d'autoriser l'upload.
Voir [le diagnostic](WINDOWS-SECURITY.md).

# Rusted Launcher 0.2.1

Le bouton manuel de préchargement est supprimé : les créations et imports préparent
déjà l'instance automatiquement. L'accueil affiche un seul bouton Jouer. Les autres
pages gardent cette action et le nom de l'instance dans le pied de page. Annuler
n'apparaît que pendant une opération ou un lancement en attente. Jouer attend la
préparation et le compte ; changer d'instance, annuler ou rencontrer une erreur
supprime cette attente.

Corrigé sur Windows réel : le verrou d'opération n'est plus situé dans le dossier
d'instance, ce qui empêchait Windows de déplacer celle-ci dans la corbeille du
lanceur. Le verrou reste actif pendant l'opération et empêche les accès concurrents.
Les tests de communication utilisent maintenant des flux binaires afin de vérifier
correctement les fins de ligne LF/CRLF et les chemins Unicode sous Windows.

La compilation native MSVC, les tests unitaires et de communication, le scan Defender
et un premier démarrage de 20 secondes réussissent sur Windows réel. La protection
en temps réel et comportementale était active (signatures 1.463.24.0).
Voir le [rapport exact](results/rusted-launcher-0.2.1-windows.json). Ce contrôle
ne couvre pas un compte réel, une préparation entière ou une partie et n'établit
pas la cause du signalement antivirus de 0.2.0. Les exécutables restent non signés.

# Rusted Launcher 0.2.0

Résolus : catalogue déobfusqué complet (63 versions, snapshots incluses), préchargement
automatique à la création/import, connexions HTTPS réutilisées et 24 téléchargements,
contrôles cuivre et cartes de mods, rafraîchissement des textes sans réécriture à vide,
ouverture automatique de Microsoft indépendante d'une préparation, logo PNG original
conservé avec tous ses pixels noirs et son alpha.

Premier adaptateur natif : textures/langues vanilla de mods Fabric sans code, converties
en packs de ressources 26.3. Ce chemin ne porte pas Sodium, Iris ou Distant Horizons.
Le catalogue multiversion concerne Java/Fabric : le moteur natif reste 26.3.

L'identifiant de l'application Microsoft doit encore être fourni par le mainteneur
(dans launcher-config.json ou les options avancées) et son accès aux API autorisé.
Une connexion à un compte réel, une partie Windows et la parité complète de Prism
restent à valider. Le gain de téléchargement mesuré porte sur 256 petits assets
sous Linux, pas sur une comparaison de temps de lancement avec Prism sous Windows.

# Rusted Launcher 0.1.0

Le lanceur prépare Java, les fichiers Minecraft et les exports natifs avant le jeu.
Il possède sa propre interface Windows ; plus besoin d'une commande ni d'un
configurateur externe. Les chemins, le cache partagé, la console et l'arrêt des
processus enfants sont gérés par ce lanceur.

Restent ouverts pour le lanceur : enregistrement/autorisation de l'application
Microsoft puis validation avec un compte réel ; jeu sur un vrai PC Windows ; parité
complète Prism/Minecraft Launcher, gestion des skins, Forge/NeoForge/Quilt, anciennes
versions legacy, mise à jour automatique et export Modrinth. Fabric 0.20.1 est choisi
à la préparation, même si un pack importé indique une autre révision. Les détails et
les limites de validation sont dans [RUSTED-LAUNCHER.md](RUSTED-LAUNCHER.md).

# État des problèmes — Rusted 0.3.5

Résolus dans les packs complets : import Prism/Modrinth sans configurateur ni Wrapper ;
chemins dépendant du PC ; sélection native malgré la présence du bootstrap Fabric ;
arrêt forcé de la JVM laissant le moteur natif ouvert ; journaux renderer Horizon et
annonce GPU répétée au rechargement des ressources ; filtre de journaux qui ignorait les modules du binaire renommé rusted.

La formulation « tous les shaders natifs indisponibles » était trop large. L'API WGSL
native existe dans la base et le pack original optionnel Rusted Soft Light est inclus.
L'ancien support de packs externes n'est pas pour autant certifié.

Restent ouverts :

- Exécution des JAR Fabric/Sodium/Iris/Distant Horizons dans le moteur Rust : absente.
  Les mods Java supplémentaires utilisent la JVM du lanceur en mode auto.
- Partie réelle avec Sodium/Iris/DH : non validée ; le banc d'essai logiciel Linux
  a chargé les mods puis échoué au démarrage graphique.
- Compatibilité des packs Iris/GLSL et mélange de chargeurs : non certifiés.
- Remplacement du renderer hérité par un renderer original : pas réalisé. Les
  nouveaux modules d'intégration et shaders Rusted ne remplacent pas ce sous-système.
- Parité complète et multijoueur natif authentifié : à valider.
- Import et lancement depuis les interfaces desktop réelles de Prism/Modrinth sous
  Windows : à confirmer sur le poste de l'utilisateur. Le parcours Fabric réel est
  testé sous Linux isolé ; les processus Windows et leur arrêt sont testés sous Wine.
