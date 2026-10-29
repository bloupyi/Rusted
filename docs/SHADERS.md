# Rusted Soft Light 0.1.0

Petit pack natif WGSL original, fourni désactivé. Il applique un rééquilibrage léger
des couleurs : warmth 0.04 et saturation 1.05 par défaut. L'alpha et la plage HDR
sont conservés ; le HUD n'est pas affecté. Aucun coût GPU ou gain de performances
n'est annoncé sans mesure sur le matériel concerné.

Le sélectionner dans les réglages natifs de Shader Packs. Il se trouve dans
rusted/native-data/shaderpacks/Rusted-Soft-Light.hshader. Retirer la sélection
restaure le rendu sans correction de couleur. Les sources et la licence MIT sont
dans shaderpacks/rusted_soft_light/.

L'API, le chargeur et les hooks sont ceux hérités d'Horizon ; seul ce pack est
original Rusted. Ce n'est pas un remplacement d'Iris, et aucun shader ou code de
Sodium, Iris, DH ou d'un pack externe n'a été utilisé pour l'écrire.
