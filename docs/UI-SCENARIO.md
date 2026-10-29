# Scénario visuel exécuté sur le Minecraft original

Prérequis : Linux avec X11/Xvfb, libXtst, Python Pillow et Mesa/OpenGL. Dans le
cloud préparé, Xvfb est conservé sous `/workspace/.rusted-tools/xvfb/usr/bin/Xvfb`.
L'API de screenshot X11 du helper suppose un affichage 32 bits little endian.

```sh
source scripts/cloud-env.sh
python3 scripts/prepare_client.py
/workspace/.rusted-tools/xvfb/usr/bin/Xvfb :97 -screen 0 1280x720x24 -nolisten tcp -noreset
# Dans un autre shell :
DISPLAY=:97 LIBGL_ALWAYS_SOFTWARE=1 ALSOFT_DRIVERS=null python3 scripts/launch_demo.py
```

Le script affiche le dossier jetable. Le fichier `process.json` indique le PID
et le chemin du log de ce lancement. Les dossiers ne sont réutilisables que s'ils
portent le marqueur d'instance du script. Tous les fichiers de jeu restent locaux.

Les actions suivantes ont été effectuées avec les vrais événements X11 du helper
`scripts/x11_test_input.py`, puis les captures examinées visuellement :

1. Afficher le menu principal démo ; ouvrir Options, Video Settings, Controls,
   Mouse Settings. La démo garde les boutons et la limite de temps d'origine.
2. Play Demo World, attendre la génération du monde, Continue Playing. Déplacement
   avec S et inventaire avec E. Utiliser un appui suffisamment long sous rendu
   logiciel ; les captures constituent une vérification visuelle, pas une
   reconnaissance automatique de l'écran.
3. Pour piloter le regard dans Xvfb : Raw Input OFF dans les réglages de **cette
   instance uniquement**, puis plusieurs événements `move` vers le bas. Attaquer
   le sol par un clic gauche tenu environ une seconde. Un trou et l'objet de
   terre apparaissent. Ne pas modifier la survie ni les restrictions de démo.
4. Escape, Save and Quit to Title, Quit Game. Vérifier arrêt du serveur intégré,
   fin des sauvegardes et code de sortie zéro.
5. Relancer `launch_demo.py --instance <dossier_jetable>`, Play Demo World,
   Continue Playing. Vérifier position, trou et objet restaurés depuis le disque.
6. Entrer dans le trou, ramasser la terre, sauter puis clic droit pour reposer le
   bloc. Vérifier remplissage du trou et consommation de l'objet. Sauver/quitter.
7. Lancer `launch_demo.py --disabled` dans une nouvelle instance ; vérifier que
   le menu original s'affiche et que le processus quitte normalement.

Les premières captures sont sous
`build/client-runs/1793192996522892510/`. La vérification de la relecture et les
empreintes des captures sont enregistrées dans `docs/results/minecraft-integration.json`.
Le premier lancement n'avait pas de périphérique audio. Avec `ALSOFT_DRIVERS=null`,
le second indique `OpenAL initialized on device No Output` et `Sound engine started`.
Le son audible n'a pas été testé. Les erreurs de compte/Realms sont attendues sans
compte connecté et ne sont pas interprétées comme une authentification réussie.

Ces contrôles confirment un scénario de fonctionnement du vrai client avec le
module installé. Le serveur intégré utilise une connexion locale ; les appels
au codec Rust sont prouvés séparément par les tests Minecraft/Fabric et TCP.
Aucun test de serveur multijoueur authentifié n'est revendiqué ici.
