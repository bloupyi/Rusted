# Connexion Microsoft de Rusted Launcher 0.2.3

Cliquer **Se connecter avec Microsoft** : Rusted ouvre Microsoft dans le navigateur
et copie le code de connexion. Si Microsoft demande ce code, le coller, choisir son
compte puis confirmer. Le lanceur attend la validation et récupère le profil Minecraft.
Aucun identifiant d’application, portail Azure, secret ou réglage de projet n’est
demandé au joueur. Le compte doit avoir accès à Minecraft Java et disposer d’un
profil Xbox/Minecraft.

## Comparaison avec FaultlineLauncher

Le dépôt fourni par le propriétaire a été inspecté à la révision
`acc0e7524c41826a0ddc110d135578c7ef7ff7c1`. Son fichier main.js appelle
`new Auth('select_account').launch('electron')`, via **msmc 5.0.5**, dont l’archive
npm a été vérifiée contre l’intégrité SHA-512 de son package-lock.json.

Cette bibliothèque inclut le client OAuth Live public `00000000402b5328` et utilise
`login.live.com`. Elle ne nécessite donc pas que Faultline enregistre sa propre
application Entra. L’affirmation précédente selon laquelle tout lanceur devait
obligatoirement enregistrer sa propre application était trop générale.

Rusted utilise désormais ce même client public par défaut avec le **flux par code
d’appareil** Live, et une implémentation Rust originale :

- demande : `https://login.live.com/oauth20_connect.srf` ;
- validation du code et renouvellement : `https://login.live.com/oauth20_token.srf` ;
- portées : `XboxLive.signin offline_access` ;
- échanges Xbox Live, XSTS, Minecraft Services et vérification du profil/licence.

Faultline utilise une fenêtre Electron et le code d’autorisation ; Rusted ouvre le
navigateur habituel et évite d’ajouter Electron à son lanceur Rust. Le client public
partagé n’est pas une application enregistrée au nom de Rusted. Les réponses de
consentement Microsoft peuvent donc afficher l’identité du client existant.
Son fonctionnement dépend des politiques du service Microsoft.

Le client Live ne doit pas être envoyé aux points d’entrée Entra
`login.microsoftonline.com/consumers/oauth2/v2.0` : ceux-ci renvoient AADSTS700016.
Le renouvellement des sessions suit également le fournisseur qui les a émises.

## Distribution et identité personnalisée facultative

Le client public est intégré dans le lanceur et dans `launcher-config.json`.
Un ancien fichier de configuration vide utilise automatiquement ce défaut ;
les anciennes préférences joueur ne peuvent pas le remplacer. Aucun champ d’ID
n’est affiché dans Comptes. Le pipeline Windows ne dépend plus d’une variable
Azure/GitHub obligatoire.

Le mainteneur peut facultativement fournir son propre GUID d’application Entra
via `RUSTED_MICROSOFT_CLIENT_ID` ou `launcher/default-config.json`. Ce chemin conserve
le flux Entra existant et requiert les autorisations propres à cette application.
Les identifiants non reconnus ou mal formés sont refusés. Aucun secret client
n’est nécessaire ni distribué.

Les sessions et les jetons de renouvellement sont chiffrés avec Windows DPAPI.
« Déconnecter de Rusted » retire la session locale ; les autorisations distantes
se gèrent dans le compte Microsoft. Aucun compte Faultline ou fichier de jeton
existant n’a été lu ou importé pour cette adaptation.

## Vérifications et limites

Les points d’entrée Microsoft réels ont accepté le client public et renvoyé un code
de connexion ; le sondage sans validation utilisateur renvoie `authorization_pending`.
Un test explicite du code Rust demande ce code, annule immédiatement, puis vérifie
qu’aucune session n’a été enregistrée. Il n’affiche ni code ni jeton dans les journaux.
Ce test ne remplace pas une connexion réelle : licence/profil, échanges Xbox/XSTS,
consentement, renouvellement d’une vraie session et partie restent à valider avec
un compte utilisateur. Les anciens ZIP 0.2.1 ne contenaient pas ce changement.

Références :
https://github.com/bloupyi/FaultlineLauncher/blob/acc0e7524c41826a0ddc110d135578c7ef7ff7c1/main.js#L292
https://github.com/Hanro50/MSMC (MIT ; référence de comparaison, dépendance non embarquée)
https://learn.microsoft.com/entra/identity-platform/v2-oauth2-device-code
