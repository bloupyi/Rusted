# Reprise du travail

## Objectif confirmé

Modifier **le Minecraft existant**, conserver sa JVM initialement, ses menus et
ses fonctionnalités, et déplacer progressivement les composants internes en
Rust. Ne pas reprendre le prototype de jeu autonome, archivé hors du dépôt sous
`/workspace/.rusted-tools/standalone-prototype`.

## État concret

- L'environnement a été publié puis restauré ; le code et les outils ont été
  retrouvés. Le réseau est ouvert et appliqué. L'ancienne référence `master`
  inexistante a été remplacée par `HEAD` dans la configuration. Le dépôt reste
  sans commit, sur `work`, avec le travail local non suivi : le préserver, ne
  pas réinitialiser ni pousser pour contourner une publication.
- Rust 1.90.0 sous `/workspace/.cargo` et `/workspace/.rustup`. JDK local vérifié
  et wrapper javac sous `/workspace/.rusted-tools` ; JRE système conservé.
- Backend Rust : encodeur zlib avec ABI JNI, encodeur réutilisé par thread,
  bornes d'allocation et retour Java avant modification des buffers si échec.
- **Le mixin fonctionne dans les vraies classes Minecraft/Fabric.** Trois modes
  passent chacun 301 cas, dont 24 paquets sur TCP ; 189 appels Rust quand activé,
  zéro quand désactivé ou le natif absent. Le décodeur original relit les données.
- `uf`/`ue` correspondent aux cibles étudiées et leurs octets sont identiques
  dans le client et le serveur officiels 1.20.4.
- Le vrai client démo Linux a été lancé : menus/options/contrôles/inventaire,
  déplacements, destruction, ramassage et placement de bloc vérifiés visuellement.
  Monde modifié sauvegardé et relu après arrêt complet/redémarrage. Restrictions
  de démo conservées. Son audible et authentification Internet non validés.
- Lancement avec backend désactivé et conservation des logs par lancement testés.
- DLL Windows compilée, **pas exécutée sous Windows**.
- La CI Linux/Windows inclut maintenant JNI, assemblage du JAR et les essais des
  classes Minecraft, mais cette CI n'a pas été déclenchée à distance.

## Livrables

- `build/rusted-backend-0.1.0-mc1.20.4.jar` : production, natifs Linux/Windows.
- `build/Rusted-1.20.4-native-test.zip` : instance à importer dans Prism/MultiMC.
  Structure vérifiée ; import et exécution Windows à faire.
- `docs/VALIDATION.md`, `docs/results/minecraft-network/` et
  `docs/results/minecraft-integration.json` : preuves et limites.
- `scripts/prepare_minecraft.py` : fichiers officiels serveur + Fabric, avec
  checksums ; `scripts/prepare_client.py` : client et ressources complets vérifiés.
- `scripts/test_minecraft.py` : véritable intégration du mixin, arrêt avant
  serveur/monde, sans authentification, EULA acceptée ou contrôle d'accès modifié.
- `scripts/launch_demo.py` : client original en démo, dossiers isolés, logs et
  codes de sortie conservés. Ne jamais l'utiliser comme lanceur authentifié.

Les caches Minecraft, mappings officiels et ressources sont sous `build/`,
ignorés par Git. Ne pas les redistribuer. Les archives du code initial sont sous
`/workspace/backups` ; elles ne sont pas des checkouts à déclarer.

## Commandes vérifiées

```sh
cd /workspace/Rusted
source scripts/cloud-env.sh
bash scripts/setup-cloud.sh
python3 scripts/test_minecraft.py
python3 scripts/prepare_client.py
DISPLAY=:97 LIBGL_ALWAYS_SOFTWARE=1 ALSOFT_DRIVERS=null python3 scripts/launch_demo.py
```

L'affichage Xvfb de la session de test a été arrêté après usage. Ne pas supposer
qu'un processus est conservé avec un snapshot. Le cache client complet contient
3 788 objets uniques (environ 657 Mo), en plus des JAR et bibliothèques.

## Prochaine action

1. Essayer l'archive ou le module sous Windows avec un compte autorisé et le
   lanceur habituel, dans une instance neuve 1.20.4/Fabric 0.15.11. Ne pas ouvrir
   de sauvegarde personnelle par défaut. Confirmer chargement natif et menus.
2. Faire une vraie connexion multijoueur authentifiée avec compression activée,
   puis comparer les mêmes actions avec le backend activé/désactivé. Les tests
   TCP existants ne valident pas le login ni toute une session Minecraft.
3. Profiler l'encodage en partie réelle, notamment copies JNI et allocations.
   Les mesures du seul codec sont contrastées ; ne pas promettre de gain global.
4. Pour un prochain port (p. ex. décompression), inspecter d'abord le bytecode
   réel et les cas limites, puis construire le différentiel contre la classe
   originale. Conserver ses validations et particularités observables. La
   décompression JNI actuelle est un oracle strict ; elle n'est pas branchée au
   jeu et ne garantit pas les mêmes comportements sur les paquets malformés.
5. Étendre la matrice seulement avec un adaptateur et des essais propres.

La première brique native fonctionne ; **le backend reste majoritairement en Java**, en
dehors de l'encodage zlib délégué à Rust. Le gameplay observé est celui du jeu
original, et non une réimplémentation Rust du gameplay.
