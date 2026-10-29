# Moteur Rusted

Cette arborescence est une base de code maintenue par Rusted, dérivée du code
MIT de Horizon au commit `8993772de0bf4b0f92c9f76be64f7a1e6b21cf37`.
L'essentiel de la simulation et du rendu provient encore de Horizon : Rusted
ne revendique pas l'écriture originale de ces composants. La licence d'origine
reste dans `LICENSE`; la provenance de chaque fichier initial est enregistrée
dans `../horizon/source-baseline.json`.

Les modifications Rusted actuelles sont intégrées aux sources du jeu :

- Binaire `rusted`, bannière RUSTED EDITION, fenêtre, icône et menus Rusted.
- Dossier `RUSTED_HOME`, avec repli sur HORIZON_HOME pour les outils existants.
- Module original `rusted-platform`, partagé avec la passerelle : inventaire
  des métadonnées des mods Java, sans exécution et sans extraction des archives.
- Page de compatibilité dans Options > Mods, alimentée par l'instance du lanceur.
- Commande `rusted --inspect-mods CHEMIN` utilisable sans GPU ni compte.
- Désactivation du compteur de joueurs Horizon, de son identité Discord et
  des migrations automatiques vers son registre de mods dans ce produit dérivé.
- Crédit de Horizon dans l'écran des crédits et au début du journal.

Le système natif `.hmod` reste celui hérité de Horizon. Identifier Fabric,
Quilt, Forge ou NeoForge ne signifie pas que leurs mods Java fonctionnent.
Sodium, Iris et Distant Horizons ne s'exécutent pas dans le moteur natif.

Depuis la racine de Rusted :

```sh
python scripts/rusted.py changes
python scripts/rusted.py build
```

La compilation génère `rusted-build.json`, qui lie les exécutables à
l'empreinte de ces sources. L'empaquetage refuse un ancien moteur ou un moteur
compilé avant les dernières modifications. Les dossiers `.local`, les JAR et
les ressources de Minecraft ne font pas partie de cette arborescence publique.

Les autres documents de cette arborescence sont ceux de Horizon conservés comme
référence technique. Leur nom, leurs liens et leur feuille de route décrivent
le projet amont, pas les fonctionnalités ajoutées ou promises par Rusted.
