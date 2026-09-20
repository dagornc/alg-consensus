//! ALG_CONSENSUS — implémentation Rust de référence.
//!
//! Portage fidèle du simulateur Python `sim_consensus.py` (Specification
//! ALG_CONSENSUS v5, §4.7.1). L'objectif est la **parité bit-à-bit** avec la
//! référence Python : mêmes graines, mêmes séquences pseudo-aléatoires, mêmes
//! résultats CSV.
//!
//! # Modèle
//!
//! - Topologie : grille 2D 6×5 (N = 30), voisinage 4-connexe, fanout f = 6.
//! - État par agent : couple `(valeur, horloge_logique)`, ordre total.
//! - Fusion : **semi-treillis** — `max` sur le couple (valeur, horloge).
//!   Opération commutative, associative, idempotente : c'est ce qui garantit
//!   la convergence indépendamment de l'ordre d'arrivée des messages.
//! - Perte i.i.d. par message, latence configurable, duplication, partition.
//!
//! # Parité avec Python
//!
//! Le point délicat est le générateur pseudo-aléatoire. Python utilise
//! Mersenne Twister (MT19937) avec `random.Random(seed)`. Ce crate embarque
//! une implémentation MT19937 compatible, ainsi que les méthodes dérivées
//! utilisées par le simulateur : `randint`, `random`, `sample`.
//!
//! Voir `rng.rs` pour les détails de compatibilité.

pub mod rng;
pub mod sim;
pub mod reconnexion;
pub mod quiescence;

pub use rng::PyRandom;
pub use sim::{simuler, Params, Resultat, GRID_H, GRID_W, N};
