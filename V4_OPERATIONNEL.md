# V4 — Couche opérationnelle du consensus (loop engineering)

> **Statut** : implémentée, testée, parité préservée.
> **Méthode** : loop engineering (Lulla et al., arXiv:2608.21884v2, JAWs@ASE 2026).
> **Branche** : `v4`. **Base** : v3 (quiescence) + v2 (reconnexion).

---

## 1. Pourquoi une v4

La v3 implémentait le loop engineering comme **cadre descriptif** : la
quiescence est une condition d'arrêt, la reconnexion un mécanisme de
résilience. Mais la v3 produit des **compteurs** — convergence, messages,
émissions évitées — pas un **système exploitable**.

Six manques opérationnels ont été identifiés :

1. **Pas d'état exploitable** — le résultat est un couple `(valeur, horloge)`
   par agent, sans état de consensus complet exportable (qui sait quoi, qui
   est d'accord avec qui).
2. **Pas de persistance** — un run ne peut pas être sauvegardé ni repris.
3. **Pas de reconfiguration dynamique** — les retraits sont fixés à un
   instant, sans possibilité de réintégration.
4. **Pas de vérification d'invariants en ligne** — aucune vérification
   indépendante des propriétés du semi-treillis pendant l'exécution.
5. **Pas de supervision** — aucune détection de dérive ni escalade.
6. **Pas de budgets durs** — la boucle s'arrête sur stabilité, pas sur un
   budget explicite.

La v4 comble ces six manques en rendant les **8 building blocks** du loop
engineering exécutables.

---

## 2. Les 8 building blocks, rendus exécutables

| # | Building block | Implémentation | Fichier |
|---|---|---|---|
| 1 | Déclenchement | `Declencheur` : Initial, Cadence, Partition, Reconfiguration, Derive, Budget | `boucle_v4.rs` |
| 2 | Condition d'arrêt machine-checkable | `ConditionArret` : accord_min, max_tours, exiger_invariants, budget_messages | `boucle_v4.rs` |
| 3 | État / mémoire | `EtatConsensus` : vues par agent, sérialisation texte stable | `operationnel.rs` |
| 4 | Skills / intention | `Intention` : reconnexion, quiescence, duplication | `boucle_v4.rs` |
| 5 | Isolation bac à sable | `Bac` : graine déterministe, exécution reproductible | `boucle_v4.rs` |
| 6 | Vérification maker/checker | `verifier_run`, `verifier_semi_treillis` — indépendants du simulateur | `verificateur.rs` |
| 7 | Supervision / audit | `Supervision` : journal, détection de dérive, escalade | `boucle_v4.rs` |
| 8 | Budgets durs | `BudgetV4` : max_runs, max_evaluations | `boucle_v4.rs` |

---

## 3. Modules ajoutés

- **`src/operationnel.rs`** (547 l.) — `EtatConsensus`, `VueAgent`,
  `Reconfiguration`, `MetriquesOperationnelles`, `construire_etat`,
  sérialisation/désérialisation, **5 invariants**.
- **`src/verificateur.rs`** (307 l.) — vérification indépendante :
  invariants d'état, monotonie de la valeur, budget de messages,
  propriétés du semi-treillis (commutativité, associativité, idempotence).
- **`src/boucle_v4.rs`** (559 l.) — la boucle complète, `Adaptation`
  (move *handoff*), `Supervision`, escalade sur tous les chemins de sortie.
- **`src/sim.rs`** — ajout de `simuler_avec_vues` : retourne les **vues
  réelles** de chaque agent. `simuler` reste **strictement inchangé**
  (parité préservée).
- **`examples/operationnel_v4.rs`** (124 l.) — démonstration du cycle
  complet, y compris trois attaques d'état corrompu.

---

## 4. Résultats mesurés

### Cycle nominal (partition + quiescence, graines 1000-1009)

```
runs=1  evaluations=1  arrêt=true  motif=condition d'arrêt satisfaite au tour 0
accord final=1.000  valeur=Some(8)  actifs=30
invariants : []
```

### Mode dégradé (perte totale) + adaptation

```
runs=4  arrêt=false  motif=budget de runs épuisé
tour 0 accord=0.200 msgs=1078 dup=1
tour 1 accord=0.233 msgs=2156 dup=2
tour 2 accord=0.167 msgs=3234 dup=3
tour 3 accord=0.300 msgs=4312 dup=4
ESCALADE : condition d'arrêt non satisfaite après 4 run(s)
```

**Constat honnête** : sous perte totale, l'adaptation augmente la duplication
(1→4) et le coût (1078→4312 messages) **sans améliorer la convergence**
(accord final 0.30). C'est un résultat négatif réel. La v4 prétend
**détecter, consigner et escalader** — pas résoudre.

### Cas de convergence vérifiés

| Configuration | Accords | Interprétation |
|---|---|---|
| Partition forcée + divergence | 10/10 | converge réellement (une seule valeur) |
| Perte 0.9 | 10/10 | converge réellement (graphe dense) |
| Perte 1.0 | 0/10 | **ne converge pas** — cas d'escalade |
| Perte 0.99 | 0/10 | **ne converge pas** |
| max_periodes=1 | 0/10 | **ne converge pas** (budget trop court) |

---

## 5. Attaques de la solution (la vérification n'est pas du théâtre)

Trois corruptions testées, **toutes rejetées** :

1. **Agent inactif porteur d'état** → invariant 5 (activité ↔ état) déclenché.
2. **Champ non numérique** → désérialisation refusée.
3. **Cardinalité incohérente** (en-tête ≠ nombre de lignes) → refusée.

Ces attaques ont révélé **deux vrais défauts**, corrigés :

- `EtatConsensus::vide(n)` créait `n` agents **actifs sans état** —
  incohérent. Corrigé : 0 actif.
- `Reconfiguration::appliquer` retirait un agent **en conservant son état** —
  incohérent. Corrigé : l'état du retiré est effacé.

Sans ces attaques, la couche de vérification aurait été décorative.

---

## 6. Parité

`simuler` est **inchangé**. La parité bit-à-bit avec la référence Python est
vérifiée : **99 graines, 0 écart** (graines 1000-1010).

La couche v4 est **additive** : elle n'altère pas le cœur algorithmique.

---

## 7. Limites

1. **Pas de parité Python pour la couche v4.** La parité porte sur le cœur
   algorithmique (v1/v2/v3). La couche opérationnelle est propre à Rust.
2. **Une seule politique d'adaptation** (duplication croissante). Un système
   réel en aurait plusieurs, choisies selon le mode de défaillance.
3. **Pas de système distribué réel.** Les agents ne communiquent pas par
   réseau ; la reconfiguration s'applique à l'état, pas à des processus
   vivants.
4. **L'adaptation ne résout pas la perte totale** — elle escalade.

---

## 8. Utilisation

```bash
# Boucle opérationnelle sur une plage de graines
./target/release/consensus_rs --v4 --seeds 1000-1009

# Avec journal d'audit
./target/release/consensus_rs --v4 --seeds 1000-1009 --journal

# Avec état sérialisé
./target/release/consensus_rs --v4 --seeds 1000-1002 --etat

# Démonstration complète du cycle
cargo run --release --example operationnel_v4
```
