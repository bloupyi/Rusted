> Archive : guide du prototype 0.1.0. Pour Rusted actuel, utiliser [NATIVE-WINDOWS.md](NATIVE-WINDOWS.md).

# Passerelle Horizon pour Prism et l'application Modrinth — Windows

Ce kit est une première intégration en développement. Il contient notre adaptateur Rust,
pas un jeu complet. Le moteur choisi est [Horizon](https://github.com/horizon-rust/horizon),
révision `8993772de0bf4b0f92c9f76be64f7a1e6b21cf37`, ciblant Minecraft **26.3**.
Les échanges de lancement passent les tests Linux avec un programme de test écrit ici.
L'ouverture des interfaces Prism/Modrinth, l'exécution Windows et la connexion réelle
à un serveur avec un compte Microsoft restent à vérifier.

## Utiliser les mods existants pendant le développement

1. Extraire `Rusted-Horizon-Bridge-Windows-0.1.0.zip`, par exemple dans
   `C:/Rusted/TestFabric/`. Prévoir un dossier de configuration distinct par instance.
2. Dans Prism ou Modrinth, créer une instance **Fabric Java habituelle**, sur une version
   pour laquelle Sodium, Iris et Distant Horizons publient des versions compatibles entre elles.
   Les installer depuis leurs pages officielles. Aucun des trois n'est fourni dans le kit.
3. Garder `"mode": "java"` dans `rusted-horizon.json` (réglage livré par défaut).
4. Dans Prism : modifier l'instance → Paramètres → Commandes personnalisées →
   commande d'enveloppe / **Wrapper command**. Dans Modrinth App : paramètres de
   l'instance → **Custom game launch hooks** → **Wrapper**. Activer la surcharge
   des réglages pour cette instance uniquement.
5. Renseigner la même commande dans les deux applications :

   ```text
   "C:/Rusted/TestFabric/rusted-launch.exe" --config "C:/Rusted/TestFabric/rusted-horizon.json" --
   ```

Le `--` final est nécessaire : le lanceur ajoute ensuite sa commande Java.
Utiliser des `/` dans les chemins évite les différences d'interprétation des antislashs.
La connexion Microsoft se fait dans le lanceur habituel. Le mode Java transmet sa commande,
son entrée standard et son code de sortie ; le rendu reste celui de Java et des mods.
Il n'apporte pas de gain de FPS ni de compatibilité supplémentaire entre les versions des mods.
On peut aussi utiliser l'instance Fabric directement sans installer la passerelle.

## Passer au moteur natif quand il est préparé

Créer une **autre instance**, vanilla **26.3**, sans Fabric, Forge ni JAR dans `mods/`,
et un autre dossier pour sa passerelle. Le mode natif ne charge pas les mods Java.

Il faut d'abord obtenir le client natif et les ressources préparées depuis sa propre copie
de Minecraft. Pour une compilation reproductible, depuis ce dépôt, avec Git, Python,
Rust et un JDK **25** installés :

```powershell
rustup toolchain install 1.99.1 --profile minimal
python scripts/horizon.py fetch
python scripts/horizon.py build-tools
python scripts/horizon.py build-game
python scripts/horizon.py prepare --minecraft-dir "C:/chemin/installation-minecraft" --server-jar "C:/chemin/server-26.3.jar"
```

`horizon-prepare` utilise les fichiers de votre installation autorisée, jouée une première fois,
et vérifie les versions/empreintes attendues. Il ne télécharge pas Minecraft. Le dossier fourni
doit contenir ou permettre de retrouver le client, les bibliothèques et les assets de 26.3.
La préparation source nécessite le JDK 25 ; le JDK 21 de l'ancien prototype ne suffit pas.
La compilation Windows complète du moteur n'a pas été vérifiée ici.

Copier `build/horizon/target/release/horizon-client.exe` dans `native/` à côté de la
configuration. Copier **localement seulement** les ressources préparées de
`build/horizon/source/.local/horizon-client/26.3/` dans `resources/`. Ces ressources
ne doivent pas être incluses dans une archive publique. À défaut, les chemins absolus
vers ces dossiers peuvent être renseignés dans la configuration.

Remplacer `rusted-horizon.json` par le contenu de `rusted-horizon.native.example.json`
et vérifier les chemins. Le dossier natif `native-data/` est séparé des sauvegardes Java.
Au premier démarrage il doit être neuf ou vide ; la passerelle y place un marqueur
de révision. Elle refuse un dossier déjà rempli sans ce marqueur.
Dans PowerShell :

```powershell
.\rusted-launch.exe --config .\rusted-horizon.json --check
```

Ce contrôle vérifie le fichier de configuration, la présence du binaire et la version déclarée
des ressources ; il ne prouve pas la provenance du binaire, la complétude des ressources,
la parité du jeu ou le bon fonctionnement du compte. Lancer ensuite depuis le compte Microsoft
connecté de Prism/Modrinth avec la commande d'enveloppe de cette nouvelle instance.

Le jeton déjà fourni par le lanceur est transmis dans un fichier temporaire privé, lu puis
supprimé par Horizon ; notre adaptateur ne l'écrit pas dans ses logs ni dans les arguments
du processus enfant. Aucune nouvelle connexion, récupération de mot de passe ou fabrication
de jeton n'est implémentée. Les tests utilisent uniquement un marqueur de test sans validité.
Le mode natif refuse la démo Java, les identités hors ligne et Quick Play solo/Realms.
Si Horizon s'arrête avant de lire sa session, un fichier temporaire peut subsister sous Unix :
ne pas inclure le dossier temporaire dans des rapports.

## Distribution ultérieure

Le ZIP est un kit à configurer manuellement, pas un `.mrpack` ni un pack Prism prêt à jouer.
Un `.mrpack` décrit une installation Minecraft et ses fichiers ; il n'enregistre pas cette
commande d'enveloppe personnalisée. Une publication de pack sur Modrinth devra respecter
son format, ses règles de distribution et disposer d'une procédure d'installation du runtime.
Nous n'avons rien publié sur Modrinth ni modifié les installations du joueur.

Pour revenir au lancement normal, vider le champ Wrapper de cette instance. Pour conserver
les mods Java, utiliser l'instance Fabric et sa configuration `"mode": "java"`.
