# Mesure du codec natif

Médianes en microsecondes, 100 échantillons après 30 échauffements. Voir `VALIDATION.md` pour les conditions. Aucun gain global du jeu établi.

| Taille | Contenu | Rust + JNI | Java Deflater réutilisé | Octets compressés Rust / Java |
| --- | --- | ---: | ---: | ---: |
| 256 | zeros | 8.406 | 5.330 | 21 / 12 |
| 256 | seed42-random | 13.806 | 13.305 | 267 / 267 |
| 8192 | zeros | 32.230 | 33.212 | 31 / 31 |
| 8192 | seed42-random | 130.253 | 100.838 | 8203 / 8203 |
| 1048576 | zeros | 3232.398 | 4664.138 | 1039 / 1039 |
| 1048576 | seed42-random | 29138.923 | 23199.721 | 1048752 / 1048902 |
