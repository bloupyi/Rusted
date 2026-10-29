# Configurer Microsoft pour Rusted Launcher

Rusted Launcher ne contient pas encore d'identifiant d'application Microsoft.
Il faut enregistrer une application propre au projet avant de connecter un compte.
La préparation des fichiers et la gestion des instances fonctionnent sans connexion.

1. Ouvrir https://entra.microsoft.com et accéder à Microsoft Entra ID > Inscriptions
   d'applications > Nouvelle inscription. L'accès à un annuaire Entra permettant
   l'enregistrement peut être nécessaire selon le compte utilisé.
2. Choisir un nom, par exemple « Rusted Launcher », et un type de comptes acceptant
   les **comptes Microsoft personnels** : comptes personnels uniquement, ou comptes
   d'organisation et personnels. Les comptes Minecraft personnels utilisent cette
   possibilité. L'application emploie l'autorité OAuth `consumers`.
3. Dans Authentification, activer **Autoriser les flux de clients publics**. Rusted
   utilise le flux par code d'appareil ; aucun secret client et aucune URI de retour
   ne sont nécessaires pour ce flux. Ne pas ajouter de secret dans le lanceur.
4. Dans Vue d'ensemble, copier **ID d'application (client)**, le GUID public.
   Ce n'est ni l'ID d'objet, ni l'ID du locataire, ni un secret.
5. Dans Rusted Launcher > Comptes, coller ce GUID dans le champ prévu, puis cliquer
   « Se connecter à Microsoft ». L'identifiant est enregistré dans les réglages.
6. Ouvrir Microsoft avec le bouton affiché, entrer le code d'appareil et autoriser
   la connexion. Le mot de passe reste chez Microsoft. Le compte doit avoir accès
   à Minecraft Java et disposer d'un profil Xbox/Minecraft.

Le flux demande `XboxLive.signin offline_access`, puis échange le jeton auprès de
Xbox Live, XSTS et Minecraft Services. La création d'une application Entra seule
ne garantit pas l'accès à ces services : Microsoft/Minecraft peut exiger une
approbation supplémentaire de l'application ou imposer ses conditions d'accès.
Si Xbox/Minecraft refuse l'échange après la connexion Microsoft, vérifier ces droits
et le profil Xbox. Réutiliser l'identifiant d'un autre lanceur n'est pas une solution.

Pour distribuer Rusted publiquement avec une identité stable, le mainteneur doit
administrer cette application, ses autorisations et ses informations de consentement.
L'identifiant client est public ; les jetons de connexion sont secrets. Cette
configuration n'enregistre aucune application distante automatiquement.

Documentation officielle :
https://learn.microsoft.com/entra/identity-platform/quickstart-register-app
https://learn.microsoft.com/entra/identity-platform/v2-oauth2-device-code

Les sessions sont chiffrées localement par Windows DPAPI. « Déconnecter de Rusted »
retire la session locale ; la gestion des autorisations distantes se fait dans le
compte Microsoft. Ne pas partager le dossier accounts ou une sauvegarde du profil
Windows avec ses secrets.

## Fournir une connexion simple aux utilisateurs

Après enregistrement/autorisation de l'application, le mainteneur peut renseigner
`microsoft_client_id` dans `launcher-config.json`, à côté de Rusted Launcher.exe,
avant de distribuer le dossier complet. Les nouvelles installations liront cet
identifiant public automatiquement : les joueurs cliquent alors simplement sur
« Se connecter avec Microsoft » ; le navigateur s'ouvre et le code est copié.
Le champ avancé de l'application reste une autre manière de renseigner cet ID.
Ne jamais placer de secret client dans ce fichier. La version de démonstration
contient un champ vide, car aucune application n'a encore été enregistrée.
