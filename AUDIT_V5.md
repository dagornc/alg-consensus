# Audit contradictoire — spécification v6 / Rust v5

Date : 20 septembre 2026. Base : branche v4, commit `58d59f6`.
Ce journal décrit des actions et résultats vérifiables, pas un raisonnement privé.

## Boucle 1 — planification

Inspecter branches, code, contrat Python et tests. Conserver le moteur historique
comme oracle. Séparer la numérotation documentaire (v6) du développement (v5).
Mesurer les ressources sans modifier les statistiques du modèle.

## Boucle 2 — premier brouillon

Extraction du moteur avec vues vers `optimise_v5.rs`, agrégation des messages
par destinataire/date, livraison FIFO, test d'accord sans hachage, réutilisation
du buffer des cibles. Ajout d'une API validée et d'une sélection CLI explicite.

## Boucle 3 — audit contradictoire

Points qui invalideraient une proposition trop large :

1. La réciprocité de reconnexion historique contourne le canal simulé.
   Un état peut changer malgré une perte totale. Un test témoin matérialise
   cette limite ; la v5 ne la présente pas comme un progrès de fiabilité.
2. « Semi-treillis donc convergence garantie » est faux sans hypothèse de
   communication. Le contrat sépare propriétés de fusion et vivacité.
3. `None` est neutre, pas absorbant. La spécification corrige le vocabulaire
   sans modifier le fichier historique servant d'oracle.
4. Un regroupement sans date livrerait certains états trop tôt ; un filtrage
   des doublons avant perte modifierait le RNG et le coût. Tous deux interdits.
5. Une FIFO n'est correcte ici que parce que la latence est fixe. Une latence
   variable future nécessiterait un calendrier ordonné, pas cette hypothèse.
6. Calculer `t + latence` sans contrôle peut déborder. La v5 filtre d'abord
   les arrivées hors horizon et rejette des bornes de compteurs non représentables.
7. Des résultats finaux seuls ne sont pas une preuve exhaustive de trajectoire.
   Les tests différentiels sont complétés par la justification inductive v6,
   sans prétendre à une preuve vérifiée par assistant formel.

## Boucle 4 — isolation

**Branche rejetée :** prétendre réduire le trafic ou corriger le protocole
réseau tout en conservant bit-à-bit les résultats du simulateur.

**Branche rejetée :** déclarer l'absence universelle de défaut après une suite
finie. L'acceptation porte uniquement sur le contrat, les tests et mesures publiés.

## Boucle 5 — corrections et mesures

- Conservation de la séparation des dates et de chaque tirage de perte.
- Validation des pertes non finies et des débordements potentiels.
- Test différentiel de 5 760 configurations/graines et 600 cas longs.
- Tests extrêmes : horizon zéro, latence `usize::MAX`, pertes totales,
  duplication zéro, retraits et graine `u64::MAX`.
- Contrôle des lois de jointure sur 16 états, donc 4 096 triplets.
- Comparaison indépendante avec CPython : 900 couples test/graine, zéro écart.
- Benchmark avec échauffement, sept répétitions, ordre des moteurs alterné.
- Correction du nouvel avertissement Clippy dans le moteur v5 ; avertissements
  historiques laissés visibles, sans réécriture opportuniste de la référence.

## Limites de la validation finale

Aucun défaut de compatibilité détecté dans les contrôles exécutés. Ce n'est
pas une certification ni un audit de sécurité réseau. Pas de validation de
latence variable, de processus distribués réels, d'échelle supérieure à 30
agents ou de reprise complète après crash. Le moteur v5 contient une copie
spécialisée du simulateur : toute évolution exige de rejouer les tests
différentiels, et pas seulement de régénérer le fichier.
