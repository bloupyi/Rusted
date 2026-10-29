# Connexion Microsoft : configuration de l’éditeur

Les joueurs ne doivent saisir aucun identifiant d’application. Dans une distribution
configurée, ils cliquent **Se connecter avec Microsoft** : le navigateur ouvre
Microsoft, le code de connexion est copié, puis le lanceur reçoit sa propre session.
Le mot de passe reste sur le site Microsoft. Un compte possédant Minecraft Java
et un profil Xbox/Minecraft est nécessaire.

## Enregistrement unique de Rusted

Microsoft OAuth exige un identifiant public pour reconnaître le logiciel qui demande
la connexion. Les autres lanceurs fournissent le leur dans leur distribution.
Ouvrir simplement une page de connexion Microsoft sans application enregistrée ne
permet pas de recevoir un jeton Minecraft. Cet enregistrement relève du mainteneur,
pas de chaque joueur. Aucun secret client ne doit être créé ou distribué pour ce flux.

1. Avec le compte qui administrera Rusted, ouvrir https://entra.microsoft.com puis
   **Inscriptions d’applications > Nouvelle inscription**. L’accès à un annuaire
   permettant les inscriptions est nécessaire.
2. Nommer l’application **Rusted Launcher** et accepter les **comptes Microsoft
   personnels**. Dans Authentification, autoriser les flux de **clients publics**.
   Le flux de code d’appareil utilise l’autorité `consumers` et ne nécessite pas
   d’URI de retour ni de secret client.
3. Vérifier les conditions/approbations Minecraft/Xbox requises pour cette application.
   Une inscription Entra seule ne garantit pas l’accès aux API Minecraft Services.
4. Fournir une fois le GUID public **ID d’application (client)** à la publication,
   dans la variable GitHub Actions `RUSTED_MICROSOFT_CLIENT_ID` du dépôt, ou dans
   `launcher/default-config.json` pour un assemblage local. Ce n’est ni un ID de
   locataire, ni un ID d’objet, ni un secret. Ne pas utiliser l’identité d’un autre lanceur.
5. Assembler, valider le ZIP Windows puis vérifier une vraie connexion avec un compte
   autorisé avant d’annoncer l’authentification comme validée.

## Assemblage et vérifications

Le pipeline Windows de distribution attend la variable publique
`RUSTED_MICROSOFT_CLIENT_ID`. Les tests unitaires et de communication continuent
sans elle. `scripts/package_launcher.py` injecte l’identité dans
`launcher-config.json`, à côté de l’exécutable. Ce fichier appartient à la distribution :
il n’est plus éditable dans la page Comptes et ne fait plus partie des préférences
joueur. Il est lu à chaque ouverture, même avec un ancien profil utilisateur.

L’assemblage normal échoue si l’identité est vide ou mal formée. La seule exception
est `--allow-unconfigured-auth`, réservée au développement : elle crée un ZIP nommé
`-preview.zip`, avec `microsoft_login_configured: false` et
`distribution_kind: development-preview` dans son manifeste. Le bouton Microsoft
est alors désactivé avec une explication, sans demander d’ID au joueur. La validation
du format d’un GUID ne prouve pas son enregistrement ni l’autorisation Xbox/Minecraft.

Les ZIP 0.2.1 précédents ont été validés pour leur scan Defender et leur premier
démarrage, mais leur configuration Microsoft était vide. Ils ne valident donc
aucune connexion. Aucun accès Azure administrateur n’est disponible dans
l’environnement de développement pour créer l’application à la place du propriétaire.

Les sessions et les jetons de renouvellement sont chiffrés avec Windows DPAPI.
« Déconnecter de Rusted » retire la session locale ; les autorisations distantes
se gèrent dans le compte Microsoft. Ne pas partager le dossier accounts.

Documentation officielle :
https://learn.microsoft.com/entra/identity-platform/quickstart-register-app
https://learn.microsoft.com/entra/identity-platform/v2-oauth2-device-code
