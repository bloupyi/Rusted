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

1. exige que Defender et sa protection en temps réel soient déjà actifs ;
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

L'environnement de développement actuel est Linux et ne fournit pas Defender.
Le dépôt GitHub sélectionné est public et encore vide : aucun workflow n'y est
enregistré. Exécuter ce contrôle exige une première publication des sources et
du workflow dans le dépôt ; cette publication publique n'a pas été effectuée.
Le nouveau contrôle n'a donc pas encore produit de résultat Windows réel. Il ne faut pas
annoncer ce travail comme une version corrigée de l'exécutable.

## Si la détection persiste pendant la validation Windows

Faire analyser l'échantillon exact par Microsoft via son portail officiel :
https://www.microsoft.com/en-us/wdsi/filesubmission

La soumission doit indiquer le nom de détection, l'empreinte de l'échantillon,
la version et les actions réellement réalisées par le logiciel. Un modèle de
description est fourni dans `docs/security/microsoft-submission-0.2.0.txt`.
Aucun exécutable n'a été envoyé à Microsoft ou à un service d'analyse tiers
dans cette intervention.
