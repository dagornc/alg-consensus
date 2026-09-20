# Audit contradictoire — Rust v6 / spécification v7

## Boucle 1 — planification

Priorités : supprimer les échanges hors canal, compter toutes les tentatives,
préserver les délais, interdire les messages d'anciennes incarnations, reprendre
exactement une exécution et agréger sans modifier les décisions du canal.

## Boucle 2 — premier brouillon

Nouveau moteur isolé, sondes cycliques, file brute/agrégée, sauvegarde JSON.
Les moteurs historiques restent inchangés pour maintenir les comparaisons.

## Boucle 3 — audit

- Un simple accord instantané peut masquer un événement futur : aucun arrêt
  anticipé sur accord dans le nouveau contrat.
- L'absence de contrôle précis des incarnations dans une sauvegarde était
  insuffisante : comparer aux événements déjà exécutés.
- La sérialisation flottante doit préserver exactement la probabilité :
  activer `float_roundtrip` et vérifier l'identité des checkpoints.
- Limite initiale 16 Mio trop faible pour le stockage brut maximal : porter
  la borne à 64 Mio, contrôler aussi l'export et borner la lecture du CLI.
- Parcourir tous les événements à chaque tour gaspille du temps : recherche
  binaire dans la séquence triée puis traitement du seul tour courant.
- Les modes partagent le canal : leur équivalence ne suffit pas à prouver le
  protocole. Ajouter des cas analytiques et adverses indépendants.

## Boucle 4 — branches rejetées

**Branche rejetée :** compatibilité bit-à-bit v5 revendiquée pour un nouveau
modèle réseau. Seule la non-régression des moteurs historiques est exigée.

**Branche rejetée :** présenter le gain contre l'oracle brut comme un gain
contre une implémentation optimisée ou un réseau réel. Les limites de la
baseline sont explicites dans le rapport de mesure.

## Boucles 5–6 — corrections et validation

Validation structurelle renforcée du calendrier et des compteurs ; cas de
perte totale, zéro délai, budget exact, saturation, réintégration et monotonie.
Matrice différentielle à chaque tour, reprises à cinq points par scénario
et par mode, comparaison des checkpoints finaux. Rejeu des tests historiques.

Résultats locaux : 78 tests réussis en debug et en release ; 900 comparaisons
historiques Rust v5/Python sans écart. Test CLI réel : reprise après 17 tours
identique à l'exécution continue, refus d'écrasement d'une sauvegarde existante.

## Réserves non levées par les tests

Pas de preuve formelle machine, pas de Byzantine, pas de réseau réel, pas de
fuzzing exhaustif, ni de persistance transactionnelle résistante aux pannes.
Une sauvegarde cohérente mais fabriquée n'est pas authentifiée. Un accord
observé n'est pas un certificat distribué. Aucune promesse « zéro défaut ».
Voir V6_PROTOCOLE.md pour les commandes et mesures reproductibles.
