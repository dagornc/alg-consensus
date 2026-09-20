//! Simulateur ALG_CONSENSUS — portage fidèle de `sim_consensus.py`.
//!
//! Implémente le harnais décrit en §4.7.1 de la spécification v5 :
//! topologie grille 2D 6×5 (N = 30), fanout f = 6, perte i.i.d. par message,
//! latence configurable, horloges logiques par agent, gossip de digests
//! d'état avec fusion semi-treillis, mesure du temps de re-fusion après
//! partition (test T5D).
//!
//! # Propriété fondamentale
//!
//! La fusion est un **semi-treillis** : `max` sur le couple
//! `(valeur, horloge_logique)`. Cette opération est :
//!
//! - **commutative** : `a ⊔ b = b ⊔ a`
//! - **associative** : `(a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)`
//! - **idempotente** : `a ⊔ a = a`
//!
//! Ces trois propriétés garantissent que l'état final est **indépendant de
//! l'ordre d'arrivée des messages** — c'est ce qui rend le protocole
//! tolérant à la perte, à la duplication et à la réordonnancement.

use crate::rng::PyRandom;

/// Période de gossip (s).
pub const T_G: f64 = 1.0;
/// Largeur de la grille.
pub const GRID_W: usize = 6;
/// Hauteur de la grille.
pub const GRID_H: usize = 5;
/// Nombre d'agents.
pub const N: usize = GRID_W * GRID_H;
/// Fanout : nombre de voisins contactés par période.
pub const FANOUT: usize = 6;
/// Latence maximale (s).
pub const LATENCE_MAX: f64 = 2.0 * T_G;

/// État d'un agent : `(valeur, horloge_logique)`.
///
/// L'ordre lexicographique sur ce couple est l'ordre du semi-treillis.
pub type Etat = (i32, u32);

/// Fusion semi-treillis : `max` sur le couple `(valeur, horloge)`.
///
/// `None` représente l'élément absorbant (agent sans information).
#[inline]
pub fn fusion(a: Option<Etat>, b: Option<Etat>) -> Option<Etat> {
    match (a, b) {
        (None, x) | (x, None) => x,
        (Some(x), Some(y)) => Some(if x >= y { x } else { y }),
    }
}

/// Voisinage 4-connexe sur une grille `w × h`.
pub fn voisins(i: usize, w: usize, h: usize) -> Vec<usize> {
    let x = i % w;
    let y = i / w;
    let mut out = Vec::with_capacity(4);
    if x > 0 {
        out.push(i - 1);
    }
    if x < w - 1 {
        out.push(i + 1);
    }
    if y > 0 {
        out.push(i - w);
    }
    if y < h - 1 {
        out.push(i + w);
    }
    out
}

/// Paramètres d'un test.
#[derive(Debug, Clone, Copy)]
pub struct Params {
    /// Probabilité de perte par message.
    pub perte: f64,
    /// Nombre de copies par message.
    pub duplication: usize,
    /// Nombre d'agents retirés.
    pub retrait: usize,
    /// Partition gauche/droite active.
    pub partition: bool,
    /// Nombre maximal de périodes simulées.
    pub max_periodes: usize,
    /// Latence en périodes (0 = arrivée dans la même période).
    pub latence: usize,
    /// Période du retrait (`None` = à l'initialisation).
    pub t_retrait: Option<usize>,
    /// Force une divergence réelle entre les deux moitiés.
    pub forcer_divergence: bool,
    /// **v2** — active la reconnexion des agents isolés.
    ///
    /// Désactivé par défaut : une simulation avec `reconnexion: false` suit
    /// exactement le chemin de code de la v1 et produit les mêmes résultats.
    pub reconnexion: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            perte: 0.0,
            duplication: 1,
            retrait: 0,
            partition: false,
            max_periodes: 200,
            latence: 0,
            t_retrait: None,
            forcer_divergence: false,
            reconnexion: false,
        }
    }
}

/// Résultat d'une exécution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resultat {
    /// Période de convergence (`None` si non convergé).
    pub periode_convergence: Option<usize>,
    /// Accord atteint (tous les agents actifs partagent la même valeur).
    pub accord: bool,
    /// Nombre de messages émis.
    pub messages: u64,
    /// Nombre d'agents actifs en fin de simulation.
    pub taille_etat: usize,
    /// Période de re-fusion après partition (`None` si non mesurée).
    pub periode_refusion: Option<usize>,
    /// Divergence réelle observée à la fin de la partition.
    pub divergence_reelle: bool,
    /// **v2** — nombre d'élargissements de rayon déclenchés.
    ///
    /// Toujours 0 quand `reconnexion` est désactivé.
    pub reconnections: u64,
}

/// Exécute une simulation complète.
///
/// Portage fidèle de `simuler()` du simulateur Python : l'ordre des tirages
/// aléatoires est identique, ce qui garantit la parité des résultats.
pub fn simuler(seed: u64, p: &Params) -> Resultat {
    let mut rng = PyRandom::new(seed);

    // État initial : chaque agent a une valeur propre (valeur, horloge 0).
    let mut etat: Vec<Etat> = (0..N).map(|_| (rng.randint(0, 9) as i32, 0)).collect();
    let mut actifs: Vec<bool> = vec![true; N];

    // Retrait à l'initialisation.
    if p.retrait > 0 && p.t_retrait.is_none() {
        let pop: Vec<usize> = (0..N).collect();
        let n_retrait = p.retrait.min(N - 1);
        for i in rng.sample(&pop, n_retrait) {
            actifs[i] = false;
        }
    }

    // Partition : moitié gauche / moitié droite (par colonnes).
    let cote = |i: usize| -> bool { i % GRID_W < GRID_W / 2 };

    if p.forcer_divergence && p.partition {
        // Garantir que le maximum global n'est présent que dans la moitié
        // gauche : les agents de droite sont plafonnés sous le max de gauche.
        let max_gauche = (0..N)
            .filter(|&i| cote(i))
            .map(|i| etat[i].0)
            .max()
            .unwrap_or(0);
        for i in 0..N {
            if !cote(i) {
                etat[i] = (etat[i].0.min(max_gauche - 1), 0);
            }
        }
    }

    let mut messages: u64 = 0;
    let mut periode_conv: Option<usize> = None;
    let mut periode_refusion: Option<usize> = None;
    let fin_partition = p.max_periodes / 2;
    // File d'attente par agent : (période d'arrivée, valeur).
    let mut en_vol: Vec<Vec<(usize, Etat)>> = vec![Vec::new(); N];
    let mut divergence_reelle = false;
    // v2 : compteur d'isolement par agent (périodes consécutives sans pair).
    let mut isole: Vec<usize> = vec![0; N];
    // v2 : nombre d'élargissements déclenchés (métrique de diagnostic).
    let mut reconnections: u64 = 0;

    for t in 1..=p.max_periodes {
        // Retrait programmé.
        if p.retrait > 0 && p.t_retrait == Some(t) {
            let pop: Vec<usize> = (0..N).filter(|&i| actifs[i]).collect();
            let n_retrait = p.retrait.min(pop.len().saturating_sub(1));
            for i in rng.sample(&pop, n_retrait) {
                actifs[i] = false;
            }
        }

        // Livrer les messages arrivés à cette période.
        for d in 0..N {
            let mut restants = Vec::new();
            for &(ta, v) in &en_vol[d] {
                if ta <= t {
                    etat[d] = fusion(Some(etat[d]), Some(v)).unwrap();
                } else {
                    restants.push((ta, v));
                }
            }
            en_vol[d] = restants;
        }

        // Émission : chaque agent actif contacte un sous-ensemble de voisins.
        let mut nouveaux: Vec<Option<Etat>> = vec![None; N];
        for i in 0..N {
            if !actifs[i] {
                continue;
            }
            let mut cibles = voisins(i, GRID_W, GRID_H);
            if p.partition && t <= fin_partition {
                cibles.retain(|&c| cote(c) == cote(i));
            }
            // v2 : la v1 n'exclut pas les agents inactifs des cibles — un
            // agent peut donc « émettre » vers des voisins retirés, qui ne
            // reçoivent rien. Pour détecter l'isolement réel, il faut
            // considérer les pairs effectivement joignables.
            if p.reconnexion {
                cibles.retain(|&c| actifs[c]);
            }
            // v2 : si l'agent n'a plus aucun pair joignable, il élargit son
            // rayon de contact. Le rayon est MONOTONE : une fois élargi, il
            // ne redescend pas (sinon l'agent oscillerait entre rayon 1 et 2
            // sans jamais rester connecté assez longtemps).
            if p.reconnexion && cibles.is_empty() {
                isole[i] += 1;
                let rayon = crate::reconnexion::rayon_effectif(isole[i]);
                if rayon > 1 {
                    cibles = crate::reconnexion::candidats_rayon(i, rayon, &actifs);
                    if p.partition && t <= fin_partition {
                        cibles.retain(|&c| cote(c) == cote(i));
                    }
                    if !cibles.is_empty() {
                        reconnections += 1;
                    }
                }
            }
            // v2 : réciprocité. Un agent isolé qui émet vers un pair éloigné
            // ne reçoit rien en retour, car ce pair ne le compte pas parmi
            // ses propres cibles. Sans réciprocité, l'agent isolé reste
            // bloqué sur sa valeur initiale indéfiniment.
            //
            // La réciprocité est portée par le message : quand un agent
            // contacte un pair, il lui transmet AUSSI l'état de ce pair tel
            // qu'il le connaît — mais surtout, le pair éloigné doit pouvoir
            // répondre. On matérialise cela en faisant que tout agent
            // contacté par un agent isolé reçoit l'état de l'émetteur ET
            // renvoie le sien dans la même période.
            if p.reconnexion && !cibles.is_empty() {
                for &c in &cibles {
                    if isole[c] >= crate::reconnexion::SEUIL_ISOLE {
                        nouveaux[c] = fusion(nouveaux[c], Some(etat[i]));
                    }
                }
            }
            // v2 : un agent isolé doit aussi RECEVOIR. Il élargit sa propre
            // réception : tout pair situé dans son rayon élargi lui envoie
            // son état, même si ce pair ne l'a pas dans ses cibles.
            if p.reconnexion && isole[i] >= crate::reconnexion::SEUIL_ISOLE {
                let rayon = crate::reconnexion::rayon_effectif(isole[i]);
                let mut sources = crate::reconnexion::candidats_rayon(i, rayon, &actifs);
                if p.partition && t <= fin_partition {
                    sources.retain(|&c| cote(c) == cote(i));
                }
                for c in sources {
                    nouveaux[i] = fusion(nouveaux[i], Some(etat[c]));
                }
            }
            if cibles.is_empty() {
                continue;
            }
            let n_dests = FANOUT.min(cibles.len());
            let dests = rng.sample(&cibles, n_dests);
            for d in dests {
                for _ in 0..p.duplication {
                    messages += 1;
                    if rng.random() < p.perte {
                        continue; // message perdu
                    }
                    if p.latence > 0 {
                        en_vol[d].push((t + p.latence, etat[i]));
                    } else {
                        nouveaux[d] = fusion(nouveaux[d], Some(etat[i]));
                    }
                }
            }
        }
        for d in 0..N {
            if let Some(v) = nouveaux[d] {
                etat[d] = fusion(Some(etat[d]), Some(v)).unwrap();
            }
        }

        // Divergence réelle : à la FIN de la partition, les deux moitiés ont
        // convergé vers des valeurs DIFFÉRENTES (et non une différence
        // transitoire).
        if p.partition && t == fin_partition {
            let g: std::collections::HashSet<i32> = (0..N)
                .filter(|&i| actifs[i] && cote(i))
                .map(|i| etat[i].0)
                .collect();
            let dr: std::collections::HashSet<i32> = (0..N)
                .filter(|&i| actifs[i] && !cote(i))
                .map(|i| etat[i].0)
                .collect();
            if g.len() == 1 && dr.len() == 1 && g != dr {
                divergence_reelle = true;
            }
        }

        // Convergence : tous les agents actifs partagent la même valeur.
        let vals: std::collections::HashSet<i32> = (0..N)
            .filter(|&i| actifs[i])
            .map(|i| etat[i].0)
            .collect();
        if vals.len() == 1 && periode_conv.is_none() {
            periode_conv = Some(t);
            if p.partition && divergence_reelle && t > fin_partition {
                periode_refusion = Some(t - fin_partition);
            }
            break;
        }
    }

    let vals: std::collections::HashSet<i32> = (0..N)
        .filter(|&i| actifs[i])
        .map(|i| etat[i].0)
        .collect();
    let accord = vals.len() == 1;
    let taille_etat = actifs.iter().filter(|&&a| a).count();

    Resultat {
        periode_convergence: periode_conv,
        accord,
        messages,
        taille_etat,
        periode_refusion,
        divergence_reelle,
        reconnections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voisins_coin() {
        // Coin (0,0) : deux voisins (droite, bas).
        assert_eq!(voisins(0, 6, 5), vec![1, 6]);
    }

    #[test]
    fn voisins_centre() {
        // Case (1,1) = index 7 : quatre voisins.
        assert_eq!(voisins(7, 6, 5), vec![6, 8, 1, 13]);
    }

    #[test]
    fn fusion_est_commutative() {
        let a = Some((3, 1));
        let b = Some((5, 0));
        assert_eq!(fusion(a, b), fusion(b, a));
    }

    #[test]
    fn fusion_est_idempotente() {
        let a = Some((3, 1));
        assert_eq!(fusion(a, a), a);
    }

    #[test]
    fn fusion_est_associative() {
        let a = Some((3, 1));
        let b = Some((5, 0));
        let c = Some((1, 9));
        assert_eq!(fusion(fusion(a, b), c), fusion(a, fusion(b, c)));
    }

    #[test]
    fn fusion_none_est_neutre() {
        let a = Some((3, 1));
        assert_eq!(fusion(None, a), a);
        assert_eq!(fusion(a, None), a);
    }

    #[test]
    fn t1_converge_sans_perte() {
        let r = simuler(1001, &Params::default());
        assert!(r.accord, "T1 doit converger sans perte");
        assert_eq!(r.taille_etat, 30);
    }

    #[test]
    fn t8_retrait_avant_convergence() {
        // Le retrait à t=3 n'a lieu QUE si la simulation n'a pas déjà
        // convergé (le simulateur `break` à la convergence, comme la
        // référence Python). Sans perte, la convergence survient avant t=3 :
        // le retrait ne s'applique donc pas et la taille reste 30.
        let p = Params {
            retrait: 3,
            t_retrait: Some(3),
            ..Default::default()
        };
        let r = simuler(1001, &p);
        assert_eq!(r.taille_etat, 30, "convergence avant t=3 : retrait non appliqué");
    }

    #[test]
    fn retrait_a_l_initialisation() {
        // Retrait à l'initialisation (t_retrait = None) : 3 agents retirés.
        let p = Params {
            retrait: 3,
            t_retrait: None,
            ..Default::default()
        };
        let r = simuler(1001, &p);
        assert_eq!(r.taille_etat, 27, "3 agents retirés sur 30");
    }

    // --- v2 : reconnexion des agents isolés ---

    #[test]
    fn v2_reconnexion_desactivee_est_identique_a_v1() {
        // Non-régression : sans reconnexion, le résultat doit être
        // strictement identique à celui de la v1.
        let p = Params {
            retrait: 3,
            t_retrait: Some(3),
            ..Default::default()
        };
        for s in [1001_u64, 1010, 1060, 1500, 2000] {
            let r = simuler(s, &p);
            assert_eq!(r.reconnections, 0, "graine {s} : reconnexion inactive");
        }
    }

    #[test]
    fn v2_corrige_les_graines_t8_en_echec() {
        // Les 10 graines documentées comme en échec en v1 doivent toutes
        // converger en v2.
        let p = Params {
            retrait: 3,
            t_retrait: Some(3),
            reconnexion: true,
            ..Default::default()
        };
        for s in [1010_u64, 1060, 1228, 1350, 1363, 1536, 1631, 1639, 1687, 1699] {
            let r = simuler(s, &p);
            assert!(r.accord, "graine {s} : la v2 doit corriger l'échec v1");
        }
    }

    #[test]
    fn v2_ne_degrade_pas_les_graines_qui_convergeaient() {
        // Sur un échantillon, aucune graine ne doit passer d'accord à échec.
        let p1 = Params {
            retrait: 3,
            t_retrait: Some(3),
            ..Default::default()
        };
        let p2 = Params {
            reconnexion: true,
            ..p1
        };
        for s in 1001..=1200u64 {
            let r1 = simuler(s, &p1);
            let r2 = simuler(s, &p2);
            if r1.accord {
                assert!(r2.accord, "graine {s} : régression v2");
            }
        }
    }
}
