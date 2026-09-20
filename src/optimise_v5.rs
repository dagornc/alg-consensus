//! Rust v5 / spécification v6 : moteur optimisé, sémantique historique conservée.
//! La reconnexion reste un modèle idéalisé ; voir SPECIFICATION_V6.md.
use crate::sim::{Etat, FANOUT, GRID_H, GRID_W, N, fusion, voisins};
use crate::{Params, PyRandom, Resultat};

pub type Vues = (Resultat, Vec<Option<Etat>>, Vec<bool>, Vec<bool>);

/// Rejette les pertes non probabilistes et les compteurs potentiellement débordants.
/// Ce contrôle n'est pas un budget de temps ou de mémoire d'un service exposé.
pub fn valider(p: &Params) -> Result<(), &'static str> {
    if !p.perte.is_finite() || !(0.0..=1.0).contains(&p.perte) {
        return Err("perte doit être finie dans [0,1]");
    }
    let bound = (N as u128)
        .checked_mul(FANOUT as u128)
        .and_then(|n| n.checked_mul(p.duplication as u128))
        .and_then(|n| n.checked_mul(p.max_periodes as u128));
    if p.max_periodes == usize::MAX || bound.is_none_or(|n| n > u64::MAX as u128) {
        return Err("horizon ou compteur de messages hors limites");
    }
    Ok(())
}

pub fn simuler_v5(seed: u64, p: &Params) -> Result<Resultat, &'static str> {
    simuler_avec_vues_v5(seed, p).map(|v| v.0)
}

pub fn simuler_avec_vues_v5(seed: u64, p: &Params) -> Result<Vues, &'static str> {
    valider(p)?;
    Ok(executer(seed, p))
}

// Empty is not agreement. Early exit avoids allocation and hashing.
fn valeur_commune(mut values: impl Iterator<Item = i32>) -> Option<i32> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

fn executer(seed: u64, p: &Params) -> (Resultat, Vec<Option<Etat>>, Vec<bool>, Vec<bool>) {
    let mut rng = PyRandom::new(seed);

    let mut etat: Vec<Etat> = (0..N).map(|_| (rng.randint(0, 9) as i32, 0)).collect();
    let mut actifs: Vec<bool> = vec![true; N];

    if p.retrait > 0 && p.t_retrait.is_none() {
        let pop: Vec<usize> = (0..N).collect();
        let n_retrait = p.retrait.min(N - 1);
        for i in rng.sample(&pop, n_retrait) {
            actifs[i] = false;
        }
    }

    let cote = |i: usize| -> bool { i % GRID_W < GRID_W / 2 };

    if p.forcer_divergence && p.partition {
        let max_gauche = (0..N)
            .filter(|&i| cote(i))
            .map(|i| etat[i].0)
            .max()
            .unwrap_or(0);
        for (i, value) in etat.iter_mut().enumerate() {
            if !cote(i) {
                *value = (value.0.min(max_gauche - 1), 0);
            }
        }
    }

    let mut messages: u64 = 0;
    let mut periode_conv: Option<usize> = None;
    let mut periode_refusion: Option<usize> = None;
    let fin_partition = p.max_periodes / 2;
    let mut en_vol: [std::collections::VecDeque<(usize, Etat)>; N] =
        std::array::from_fn(|_| Default::default());
    let voisinages: [Vec<usize>; N] = std::array::from_fn(|i| voisins(i, GRID_W, GRID_H));
    let mut cibles = Vec::with_capacity(N);
    let mut divergence_reelle = false;
    let mut isole: Vec<usize> = vec![0; N];
    let mut reconnections: u64 = 0;
    let mut stable: Vec<usize> = vec![0; N];
    let mut emissions_evitees: u64 = 0;
    let mut etat_precedent: Vec<Etat> = etat.clone();

    for t in 1..=p.max_periodes {
        if p.retrait > 0 && p.t_retrait == Some(t) {
            let pop: Vec<usize> = (0..N).filter(|&i| actifs[i]).collect();
            let n_retrait = p.retrait.min(pop.len().saturating_sub(1));
            for i in rng.sample(&pop, n_retrait) {
                actifs[i] = false;
            }
        }

        for d in 0..N {
            while en_vol[d].front().is_some_and(|(ta, _)| *ta <= t) {
                let (_, v) = en_vol[d].pop_front().unwrap();
                etat[d] = etat[d].max(v);
            }
        }

        let mut nouveaux: [Option<Etat>; N] = [None; N];
        for i in 0..N {
            if !actifs[i] {
                continue;
            }
            cibles.clear();
            cibles.extend_from_slice(&voisinages[i]);
            if p.partition && t <= fin_partition {
                cibles.retain(|&c| cote(c) == cote(i));
            }
            if p.reconnexion {
                cibles.retain(|&c| actifs[c]);
            }
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
            if p.reconnexion && !cibles.is_empty() {
                for &c in &cibles {
                    if isole[c] >= crate::reconnexion::SEUIL_ISOLE {
                        nouveaux[c] = fusion(nouveaux[c], Some(etat[i]));
                    }
                }
            }
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
            if p.quiescence && !crate::quiescence::doit_emettre(stable[i]) {
                emissions_evitees += 1;
                continue;
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
                        continue;
                    }
                    if p.latence > 0 {
                        // Messages beyond the observation horizon cannot affect the result.
                        if p.latence <= p.max_periodes - t {
                            let arrival = t + p.latence;
                            if let Some((last, value)) =
                                en_vol[d].back_mut().filter(|(ta, _)| *ta == arrival)
                            {
                                debug_assert_eq!(*last, arrival);
                                *value = (*value).max(etat[i]);
                            } else {
                                en_vol[d].push_back((arrival, etat[i]));
                            }
                        }
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

        if p.quiescence {
            for i in 0..N {
                if !actifs[i] {
                    continue;
                }
                if etat[i] == etat_precedent[i] {
                    stable[i] += 1;
                } else {
                    stable[i] = 0;
                }
                etat_precedent[i] = etat[i];
            }
        }

        if p.partition && t == fin_partition {
            let g = valeur_commune((0..N).filter(|&i| actifs[i] && cote(i)).map(|i| etat[i].0));
            let dr = valeur_commune((0..N).filter(|&i| actifs[i] && !cote(i)).map(|i| etat[i].0));
            if g.is_some() && dr.is_some() && g != dr {
                divergence_reelle = true;
            }
        }

        let accord = valeur_commune((0..N).filter(|&i| actifs[i]).map(|i| etat[i].0)).is_some();
        if accord && periode_conv.is_none() {
            periode_conv = Some(t);
            if p.partition && divergence_reelle && t > fin_partition {
                periode_refusion = Some(t - fin_partition);
            }
            break;
        }
    }

    let accord = valeur_commune((0..N).filter(|&i| actifs[i]).map(|i| etat[i].0)).is_some();
    let taille_etat = actifs.iter().filter(|&&a| a).count();

    let resultat = Resultat {
        periode_convergence: periode_conv,
        accord,
        messages,
        taille_etat,
        periode_refusion,
        divergence_reelle,
        reconnections,
        emissions_evitees,
    };

    // Vues réelles : un agent inactif n'a pas d'état ; un agent actif est
    // quiescent s'il n'a pas émis à la dernière période (compteur de
    // stabilité au-delà du seuil).
    let etats: Vec<Option<Etat>> = (0..N)
        .map(|i| if actifs[i] { Some(etat[i]) } else { None })
        .collect();
    let quiescents: Vec<bool> = (0..N)
        .map(|i| actifs[i] && p.quiescence && !crate::quiescence::doit_emettre(stable[i]))
        .collect();

    (resultat, etats, actifs, quiescents)
}
