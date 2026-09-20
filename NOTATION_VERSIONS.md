# Notation des 3 versions de ALG_CONSENSUS

**Date** : 2026-09-20
**Périmètre** : implémentation Rust `consensus_rs` — branches `master` (v1),
`v2`, `v3` du dépôt `dagornc/alg-consensus`.
**Base de mesure** : 9 tests × 1 000 graines = 9 000 simulations par version,
exécutées par le CLI (`--v2`, `--v3`), résultats bruts en CSV.

---

## 1. Grille de notation

| Critère | Poids | Ce qui est mesuré |
|---|---|---|
| **A. Correction** | 6 | Proportion de graines où le consensus est atteint |
| **B. Coût en messages** | 5 | Nombre total de messages émis (normalisé sur le meilleur) |
| **C. Latence** | 3 | Période de convergence maximale (normalisée sur le meilleur) |
| **D. Robustesse** | 3 | Accord sous partition (T5, T5D, T9) |
| **E. Ingénierie** | 3 | Tests, parité, documentation, non-régression |
| **Total** | **20** | |

---

## 2. Mesures brutes (1 000 graines par test)

| test | v1 accord | v1 msg | v2 accord | v2 msg | v3 accord | v3 msg |
|---|---|---|---|---|---|---|
| T1 | 1000/1000 | 470 204 | 1000/1000 | 470 204 | 1000/1000 | 470 166 |
| T2 | 1000/1000 | 470 204 | 1000/1000 | 470 204 | 1000/1000 | 470 166 |
| T3 | 1000/1000 | 529 396 | 1000/1000 | 529 396 | 1000/1000 | 529 207 |
| T4 | 1000/1000 | 940 408 | 1000/1000 | 940 408 | 1000/1000 | 940 332 |
| T5 | 1000/1000 | 3 348 978 | 1000/1000 | 3 348 978 | 1000/1000 | 795 873 |
| T5D | 1000/1000 | 9 094 000 | 1000/1000 | 9 094 000 | 1000/1000 | 1 541 184 |
| T6 | 1000/1000 | 470 204 | 1000/1000 | 470 204 | 1000/1000 | 470 166 |
| T8 | **990/1000** | 619 038 | 1000/1000 | 420 982 | 1000/1000 | 420 875 |
| T9 | 1000/1000 | 3 348 978 | 1000/1000 | 3 348 978 | 1000/1000 | 795 873 |

**Agrégats :**

- v1 : accord **8 990/9 000**, messages **19 291 410**, convergence max 103
- v2 : accord **9 000/9 000**, messages **19 093 354**, convergence max 103
- v3 : accord **9 000/9 000**, messages **6 433 842**, convergence max 117

---

## 3. Notes

| Version | A /6 | B /5 | C /3 | D /3 | E /3 | **Total** |
|---|---|---|---|---|---|---|
| **v1** | 5,99 | 1,67 | 3,00 | 3,00 | 2,00 | **15,7 / 20** |
| **v2** | 6,00 | 1,68 | 3,00 | 3,00 | 2,60 | **16,3 / 20** |
| **v3** | 6,00 | 5,00 | 2,64 | 3,00 | 2,90 | **19,5 / 20** |

**Classement : v3 (19,5) > v2 (16,3) > v1 (15,7).**

---

## 4. Robustesse de la notation

Le classement a été testé sous 6 pondérations différentes :

| Pondération | v1 | v2 | v3 | Ordre stable ? |
|---|---|---|---|---|
| Équilibrée (6,5,3,3,3) | 15,7 | 16,3 | 19,5 | OUI |
| Coût dominant (4,8,2,3,3) | 13,7 | 14,3 | 19,7 | OUI |
| Correction dominante (10,3,2,2,3) | 17,0 | 17,6 | 19,7 | OUI |
| Coût faible (8,2,3,4,3) | 17,7 | 18,3 | 19,5 | OUI |
| Latence forte (5,4,6,2,3) | 16,3 | 16,9 | 19,2 | OUI |
| Sans ingénierie (6,5,3,3,0) | 13,7 | 13,7 | 16,6 | OUI |

**L'ordre v1 < v2 < v3 est stable sur les 6 scénarios.**

Point d'honnêteté : dans le scénario « sans ingénierie », v1 et v2 sont à
égalité (13,7). L'écart v1/v2 vient donc **entièrement** du critère ingénierie
et des 10 graines T8 — pas du coût en messages, où v2 est quasi identique à v1.

---

## 5. Lecture des résultats

**v1 → v2 : gain ciblé, coût nul.** La v2 corrige un défaut structurel précis
(agents isolés par retrait, toujours des coins de la grille). Gain : +10 graines
sur T8, −32 % de messages sur ce test. Aucun effet ailleurs — le mécanisme ne
se déclenche que sur isolement réel. La note progresse peu (15,7 → 16,3) parce
que le défaut corrigé était marginal à l'échelle des 9 000 simulations.

**v2 → v3 : gain systémique.** La v3 attaque un défaut de fond — l'émission
inconditionnelle à chaque période. Gain : **−66 % de messages** sur l'ensemble,
avec un accord maintenu à 1000/1000 partout. C'est le seul changement qui
affecte le régime nominal, pas seulement un cas pathologique.

**Le coût de la v3.** T5D converge à 117 périodes au lieu de 103 (+14). C'est
le prix du réveil périodique, indispensable : sans lui, la quiescence pure fait
tomber T5D à **0/1000** d'accord. Échange très favorable.

---

## 6. Limites de cette notation

1. **Le critère E est un jugement, pas une mesure.** Les 3 points d'ingénierie
   sont attribués par appréciation (tests, parité, documentation). C'est le
   seul critère non dérivé d'un chiffre.
2. **La normalisation du critère B écrase les écarts.** v1 et v2 obtiennent
   ~1,67/5 parce que la v3 est 3× plus économe. Cela ne signifie pas que v1 et
   v2 sont mauvais en absolu — seulement qu'ils ne sont pas optimisés.
3. **Le harnais de tests est celui de la spécification v5.** Il ne mesure pas
   la consommation d'énergie, la bande passante réelle, ni le comportement à
   grande échelle (N = 30 agents seulement).
4. **Aucune version n'est fusionnée dans `master`.** v2 et v3 sont des modes
   additionnels sur branches dédiées ; le comportement par défaut reste la v1.

---

## 7. Reproduction

```bash
cd consensus_rs
cargo build --release
python3 notation_versions.py      # notes /20
python3 sensibilite_notation.py   # robustesse de la notation
python3 compare_v1_v2_v3.py       # comparaison v1/v2/v3 sur les 9 tests
python3 verify_parite_rust.py --seeds 1001-2000   # parité v1 : 9000 graines, 0 écart
```
