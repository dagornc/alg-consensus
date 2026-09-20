// Mechanical extraction from the preserved v4 reference. Run once when rebasing.
import fs from 'node:fs';
let s=fs.readFileSync('src/sim.rs','utf8');
s=s.slice(s.indexOf('pub fn simuler_avec_vues('),s.indexOf('\n#[cfg(test)]'));
s=s.replace('pub fn simuler_avec_vues(', 'fn executer(');
s=s.replace('for i in 0..N {\n            if !cote(i) {\n                etat[i] = (etat[i].0.min(max_gauche - 1), 0);', 'for (i, value) in etat.iter_mut().enumerate() {\n            if !cote(i) {\n                *value = (value.0.min(max_gauche - 1), 0);');
s=s.replace('let mut en_vol: Vec<Vec<(usize, Etat)>> = vec![Vec::new(); N];','let mut en_vol: [std::collections::VecDeque<(usize, Etat)>; N] = std::array::from_fn(|_| Default::default());\n    let voisinages: [Vec<usize>; N] = std::array::from_fn(|i| voisins(i, GRID_W, GRID_H));\n    let mut cibles = Vec::with_capacity(N);');
s=s.replace(/        for d in 0\.\.N \{\n            let mut restants = Vec::new\(\);[\s\S]*?            en_vol\[d\] = restants;\n        \}/,`        for d in 0..N {
            while en_vol[d].front().is_some_and(|(ta, _)| *ta <= t) {
                let (_, v) = en_vol[d].pop_front().unwrap();
                etat[d] = etat[d].max(v);
            }
        }`);
s=s.replace('let mut nouveaux: Vec<Option<Etat>> = vec![None; N];','let mut nouveaux: [Option<Etat>; N] = [None; N];');
s=s.replace('let mut cibles = voisins(i, GRID_W, GRID_H);','cibles.clear();\n            cibles.extend_from_slice(&voisinages[i]);');
s=s.replace('en_vol[d].push((t + p.latence, etat[i]));',`// Messages beyond the observation horizon cannot affect the result.
                        if p.latence <= p.max_periodes - t {
                            let arrival = t + p.latence;
                            if let Some((last, value)) = en_vol[d].back_mut().filter(|(ta, _)| *ta == arrival) {
                                debug_assert_eq!(*last, arrival);
                                *value = (*value).max(etat[i]);
                            } else {
                                en_vol[d].push_back((arrival, etat[i]));
                            }
                        }`);
s=s.replace(/            let g: std::collections::HashSet<i32>[\s\S]*?            if g.len\(\) == 1 && dr.len\(\) == 1 && g != dr \{/,`            let g = valeur_commune((0..N).filter(|&i| actifs[i] && cote(i)).map(|i| etat[i].0));
            let dr = valeur_commune((0..N).filter(|&i| actifs[i] && !cote(i)).map(|i| etat[i].0));
            if g.is_some() && dr.is_some() && g != dr {`);
s=s.replace(/        let vals: std::collections::HashSet<i32>[\s\S]*?        if vals.len\(\) == 1 && periode_conv.is_none\(\) \{/,`        let accord = valeur_commune((0..N).filter(|&i| actifs[i]).map(|i| etat[i].0)).is_some();
        if accord && periode_conv.is_none() {`);
s=s.replace(/    let vals: std::collections::HashSet<i32>[\s\S]*?    let accord = vals.len\(\) == 1;/,`    let accord = valeur_commune((0..N).filter(|&i| actifs[i]).map(|i| etat[i].0)).is_some();`);
const header=`//! Rust v5 / spécification v6 : moteur optimisé, sémantique historique conservée.
//! La reconnexion reste un modèle idéalisé ; voir SPECIFICATION_V6.md.
use crate::{PyRandom, Params, Resultat};
use crate::sim::{Etat, N, GRID_W, GRID_H, FANOUT, voisins, fusion};

pub type Vues = (Resultat, Vec<Option<Etat>>, Vec<bool>, Vec<bool>);

/// Rejette les pertes non probabilistes et les compteurs potentiellement débordants.
/// Ce contrôle n'est pas un budget de temps ou de mémoire d'un service exposé.
pub fn valider(p: &Params) -> Result<(), &'static str> {
    if !p.perte.is_finite() || !(0.0..=1.0).contains(&p.perte) {
        return Err("perte doit être finie dans [0,1]");
    }
    let bound = (N as u128).checked_mul(FANOUT as u128)
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

`;
fs.writeFileSync('src/optimise_v5.rs',header+s);
