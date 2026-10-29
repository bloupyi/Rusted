# Validation Windows de Rusted Launcher 0.2.3

Le ZIP complet 0.2.3 a passé le scan Defender et un premier démarrage de 20 secondes
sur Windows réel, protection en temps réel et comportementale active, sans détection.
Moteur Defender 1.1.26100.4, signatures 1.463.30.0.

- [Exécution CI](https://github.com/bloupyi/Rusted/actions/runs/40953795188).
- [Rapport exact](results/rusted-launcher-0.2.3-windows.json).
- SHA-256 de l’archive : `a896b80580219af25fc38ef5d39fc65c232d37175e5fdaa0ccae1d8a2cf992d5`.
- SHA-256 du lanceur : `70840abebba6f9464f70bf3333bb1c79a923d590b46f90b2f6f127c24cc96fa2`.

Le lanceur et les wrappers sont compilés sur Windows avec MSVC. Les deux binaires
moteur inchangés sont repris de la compilation MSVC déjà validée de 0.2.1, après
contrôle de l’archive épinglée, de l’empreinte des sources moteur et du reçu binaire.
Les packs complets sont réassemblés, puis leur contenu final est à nouveau scanné.
Les 1 525 empreintes du contenu et les 46 empreintes des sources du lanceur
correspondent à la distribution récupérée et aux sources actuelles.

Un test indépendant du code Rust sous Windows a obtenu un code Microsoft puis
annulé sans compte enregistré. Le scan/démarrage ne valide ni une connexion complète,
ni une préparation entière, ni une partie. Les exécutables restent non signés et
ces résultats ne déterminent pas la cause de l’alerte historique 0.2.0.

# Rusted Launcher 0.2.0 : détection Defender en cours d'analyse

Le propriétaire a signalé **Behavior:Win32/DefenseEvasion.A!ml** sur
**Rusted Launcher.exe**, puis sa suppression par Microsoft Defender.
Cette alerte est un blocage antivirus, pas seulement un avertissement SmartScreen
sur un éditeur inconnu. Son nom ne prouve ni une infection réelle ni un faux positif.
La version 0.2.0 n'a pas de correctif Defender confirmé à ce stade.

Ne pas désactiver Defender, ajouter d'exclusion, restaurer ou relancer l'exécutable
pour contourner ce blocage. Conserver l'historique de détection pour l'analyse.

## Éléments vérifiés sur l'archive conservée

- ZIP SHA-256 : `76cc862a2ed52ef68df3394fa28c9e214e3325eef76f97fd12fdb4e92a4d1e43`.
- Rusted Launcher.exe SHA-256 :
  `24e56d59f1c79c1769bc96a84bcd195fe912b8d85c186b30f3017b872e0b3a37`.
- Les fichiers de l'archive correspondent à leurs empreintes dans le manifeste.
- Les quatre exécutables distribués ne portent pas de signature Authenticode.
  L'absence de signature ne démontre pas la cause de l'alerte ; signer un fichier
  ne dispense pas de vérifier une détection antivirus.
- L'inspection de leurs tables PE n'a pas trouvé de section à la fois inscriptible
  et exécutable. Ce constat ne constitue pas un résultat antivirus.
- La revue du code de démarrage du lanceur et de ses appels de processus n'a pas
  trouvé de commande modifiant les réglages Defender, d'exclusion antivirus,
  de contournement de stratégie PowerShell ni de commande de suppression de service.
  Cette revue limitée ne certifie pas toute la chaîne de dépendances.

Ces vérifications portent sur notre archive locale, pas sur une copie extraite du
poste de l'utilisateur. Les tests fonctionnels et Wine précédents ne pouvaient pas
valider Microsoft Defender. Le rapport technique est conservé dans
`docs/results/rusted-launcher-0.2.0-security.json`.

## Validation ajoutée au pipeline Windows

Le job Windows vérifie maintenant l'utilisation native de MSVC, puis appelle
`scripts/verify_windows_release.ps1` avant l'upload des ZIP. Ce script :

1. exige que Defender, sa protection en temps réel et son analyse comportementale
   soient déjà actifs ;
2. met à jour les signatures puis contrôle les empreintes du contenu distribué ;
3. appelle le scanner officiel Microsoft dont il vérifie la signature ;
4. observe 20 secondes du premier démarrage réel dans des dossiers temporaires
   réservés au test, sans compte, puis ferme le lanceur ;
5. contrôle les détections liées à ces fichiers et écrit un rapport JSON ;
6. échoue si un contrôle est indisponible, une menace détectée, un fichier disparu
   ou le démarrage interrompu. Dans ce cas, le job ne publie pas les ZIP.

Le script ne change aucun réglage de protection et ne restaure aucun fichier
quarantainé. Son observation de démarrage ne couvre pas une préparation entière,
l'authentification ou une partie. Un résultat sans détection doit être présenté
avec ces limites et la version de signatures utilisée.

Les sources et le workflow ont été publiés avec l'autorisation du propriétaire dans
[bloupyi/Rusted](https://github.com/bloupyi/Rusted). La validation de la version 0.2.1
est suivie dans [GitHub Actions](https://github.com/bloupyi/Rusted/actions/runs/40907164336).
Les cinq jobs réussissent, dont la compilation complète MSVC et le contrôle Defender
sur Windows réel. Le [rapport exact](results/rusted-launcher-0.2.1-windows.json) donne :

- archive 0.2.1 SHA-256 :
  `a6778ff504ba886e4d0b1e5e42718073476e9dc0b93afb11681b07fb146798d6` ;
- moteur Defender `1.1.26100.4`, signatures `1.463.24.0` ;
- protection en temps réel et comportementale active ;
- scan sans menace et premier démarrage de 20 secondes réussis ;
- aucun fichier modifié ou supprimé lors de ces contrôles.

Les quatre exécutables restent non signés Authenticode. La validation porte sur
l'archive exacte de cette exécution et sur ce démarrage limité ; elle ne confirme
pas la cause du signalement 0.2.0, ni une connexion, une préparation complète ou une
partie. La disponibilité d'un nouveau ZIP ne justifie pas de contourner une nouvelle
détection éventuelle sur un autre poste.

Les machines Windows éphémères de GitHub démarrent avec certaines protections
désactivées. Le script séparé `prepare_windows_defender_ci.ps1` active la protection
en temps réel et comportementale **avant la compilation** et vérifie leur état.
Il est limité aux machines GitHub hébergées, n'est pas exécuté par le produit,
n'ajoute aucune exclusion et ne diminue aucune protection. Si l'activation échoue,
le job s'arrête. Le script de validation exige ensuite que ces protections restent actives.

## Si la détection persiste pendant la validation Windows

Faire analyser l'échantillon exact par Microsoft via son portail officiel :
https://www.microsoft.com/en-us/wdsi/filesubmission

La soumission doit indiquer le nom de détection, l'empreinte de l'échantillon,
la version et les actions réellement réalisées par le logiciel. Un modèle de
description est fourni dans `docs/security/microsoft-submission-0.2.0.txt`.
Aucun exécutable n'a été envoyé à Microsoft ou à un service d'analyse tiers
dans cette intervention.
