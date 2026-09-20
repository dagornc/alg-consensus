//! **v4 — Couche opérationnelle : état de consensus complet, persistance,
//! reconfiguration.**
//!
//! La v3 (quiescence) produit des **compteurs** : convergence, messages,
//! émissions évitées. Un système opérationnel doit produire un **état
//! exploitable** — qui sait quoi, qui est d'accord avec qui —, le
//! **persister**, se **reconfigurer** à chaud et **vérifier** ses invariants.
//!
//! # Différence avec `task_alloc_rs`
//!
//! Ici l'état n'est pas une affectation tâche→agent mais un **état de
//! connaissance** : chaque agent détient un couple `(valeur, horloge)`. L'état
//! opérationnel expose donc la **vue de chaque agent**, le **degré d'accord**
//! et la **structure de connaissance** (qui partage la valeur dominante).

use crate::sim::{Etat, N};

/// Vue d'un agent : son état de connaissance et son statut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VueAgent {
    /// Indice de l'agent.
    pub agent: usize,
    /// État détenu (`None` = aucune information).
    pub etat: Option<Etat>,
    /// L'agent est-il actif (non retiré) ?
    pub actif: bool,
    /// L'agent est-il quiescent (n'émet plus) ?
    pub quiescent: bool,
}

/// **État opérationnel complet** d'un essaim après convergence.
#[derive(Debug, Clone, PartialEq)]
pub struct EtatConsensus {
    /// Période à laquelle l'état a été capturé.
    pub periode: usize,
    /// Nombre d'agents dans la topologie.
    pub n_agents: usize,
    /// Vue de chaque agent.
    pub vues: Vec<VueAgent>,
    /// Valeur dominante (celle détenue par le plus d'agents actifs).
    pub valeur_dominante: Option<i32>,
    /// Nombre d'agents actifs détenant la valeur dominante.
    pub porteurs_dominante: usize,
    /// Nombre d'agents actifs.
    pub actifs: usize,
    /// Degré d'accord : fraction d'agents actifs détenant la valeur dominante.
    pub accord: f64,
}

impl EtatConsensus {
    /// État vide (aucune information).
    ///
    /// Les agents existent dans la topologie mais ne sont **pas actifs** :
    /// un agent actif sans état serait incohérent (invariant 5).
    pub fn vide(n_agents: usize) -> Self {
        EtatConsensus {
            periode: 0,
            n_agents,
            vues: (0..n_agents)
                .map(|agent| VueAgent { agent, etat: None, actif: false, quiescent: false })
                .collect(),
            valeur_dominante: None,
            porteurs_dominante: 0,
            actifs: 0,
            accord: 0.0,
        }
    }

    /// Construit l'état à partir des vues brutes.
    pub fn depuis_vues(periode: usize, vues: Vec<VueAgent>) -> Self {
        let n_agents = vues.len();
        let actifs = vues.iter().filter(|v| v.actif).count();

        // Valeur dominante : la plus fréquente parmi les agents actifs informés.
        let mut comptes: Vec<(i32, usize)> = Vec::new();
        for v in vues.iter().filter(|v| v.actif) {
            if let Some((valeur, _)) = v.etat {
                match comptes.iter_mut().find(|(x, _)| *x == valeur) {
                    Some((_, c)) => *c += 1,
                    None => comptes.push((valeur, 1)),
                }
            }
        }
        // Tri déterministe : par compte décroissant, puis valeur croissante.
        comptes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let (valeur_dominante, porteurs_dominante) =
            comptes.first().map(|(v, c)| (Some(*v), *c)).unwrap_or((None, 0));

        let accord = if actifs == 0 {
            0.0
        } else {
            porteurs_dominante as f64 / actifs as f64
        };

        EtatConsensus {
            periode,
            n_agents,
            vues,
            valeur_dominante,
            porteurs_dominante,
            actifs,
            accord,
        }
    }

    /// **Invariants d'état** — liste des violations (vide = conforme).
    ///
    /// 1. **Cohérence de comptage** : `actifs` = nombre de vues actives.
    /// 2. **Cohérence d'accord** : `accord` = `porteurs_dominante / actifs`.
    /// 3. **Dominante cohérente** : si `porteurs_dominante > 0`, la valeur
    ///    dominante est détenue par exactement ce nombre d'agents actifs.
    /// 4. **Indices valides** : chaque vue porte l'indice de sa position.
    pub fn verifier_invariants(&self) -> Vec<String> {
        let mut violations = Vec::new();

        let actifs_reels = self.vues.iter().filter(|v| v.actif).count();
        if actifs_reels != self.actifs {
            violations.push(format!(
                "invariant 1 (comptage) : actifs={} ≠ vues actives={}",
                self.actifs, actifs_reels
            ));
        }

        let attendu = if self.actifs == 0 {
            0.0
        } else {
            self.porteurs_dominante as f64 / self.actifs as f64
        };
        if (self.accord - attendu).abs() > 1e-12 {
            violations.push(format!(
                "invariant 2 (accord) : accord={:.6} ≠ attendu={:.6}",
                self.accord, attendu
            ));
        }

        if self.porteurs_dominante > 0 {
            match self.valeur_dominante {
                None => violations.push(
                    "invariant 3 (dominante) : porteurs>0 mais aucune valeur dominante".to_string(),
                ),
                Some(v) => {
                    let reels = self
                        .vues
                        .iter()
                        .filter(|x| x.actif && x.etat.map(|(val, _)| val) == Some(v))
                        .count();
                    if reels != self.porteurs_dominante {
                        violations.push(format!(
                            "invariant 3 (dominante) : porteurs={} ≠ décompte réel={}",
                            self.porteurs_dominante, reels
                        ));
                    }
                }
            }
        }

        for (i, v) in self.vues.iter().enumerate() {
            if v.agent != i {
                violations.push(format!(
                    "invariant 4 (indices) : vue {} porte l'indice {}",
                    i, v.agent
                ));
            }
        }

        // 5. **Cohérence activité ↔ état** : un agent inactif ne peut pas
        //    porter d'état, et un agent actif doit en porter un. Sans cet
        //    invariant, un état corrompu (agent retiré mais toujours porteur
        //    d'une valeur) passerait la vérification — ce serait du théâtre.
        for v in &self.vues {
            if !v.actif && v.etat.is_some() {
                violations.push(format!(
                    "invariant 5 (activité) : agent {} inactif mais porteur d'un état {:?}",
                    v.agent, v.etat
                ));
            }
            if v.actif && v.etat.is_none() {
                violations.push(format!(
                    "invariant 5 (activité) : agent {} actif mais sans état",
                    v.agent
                ));
            }
        }

        violations
    }

    /// **Sérialisation** — format texte stable, une ligne par agent.
    ///
    /// Format : `periode|n_agents` puis `agent|actif|quiescent|valeur|horloge`
    /// par ligne (`-` pour un état absent).
    pub fn serialiser(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{}|{}\n", self.periode, self.n_agents));
        for v in &self.vues {
            let (val, hor) = match v.etat {
                Some((a, b)) => (a.to_string(), b.to_string()),
                None => ("-".to_string(), "-".to_string()),
            };
            out.push_str(&format!(
                "{}|{}|{}|{}|{}\n",
                v.agent,
                v.actif as u8,
                v.quiescent as u8,
                val,
                hor
            ));
        }
        out
    }

    /// **Désérialisation** — `None` si le texte est malformé **ou si l'état
    /// reconstruit viole ses invariants**.
    ///
    /// Le second critère est essentiel : accepter un état incohérent
    /// reviendrait à faire de la vérification un théâtre. Un état dont les
    /// vues sont incohérentes (agent inactif porteur d'état, doublons
    /// d'identifiant, valeur dominante non détenue) est rejeté.
    pub fn deserialiser(texte: &str) -> Option<Self> {
        let mut lignes = texte.lines();
        let entete = lignes.next()?;
        let mut parts = entete.split('|');
        let periode: usize = parts.next()?.parse().ok()?;
        let n_agents: usize = parts.next()?.parse().ok()?;
        if n_agents == 0 || n_agents > 10_000 {
            return None;
        }

        let mut vues = Vec::with_capacity(n_agents);
        for ligne in lignes {
            if ligne.trim().is_empty() {
                continue;
            }
            let champs: Vec<&str> = ligne.split('|').collect();
            if champs.len() != 5 {
                return None;
            }
            let agent: usize = champs[0].parse().ok()?;
            let actif = match champs[1] {
                "0" => false,
                "1" => true,
                _ => return None,
            };
            let quiescent = match champs[2] {
                "0" => false,
                "1" => true,
                _ => return None,
            };
            let etat = if champs[3] == "-" && champs[4] == "-" {
                None
            } else {
                Some((champs[3].parse().ok()?, champs[4].parse().ok()?))
            };
            vues.push(VueAgent { agent, etat, actif, quiescent });
        }

        if vues.len() != n_agents {
            return None;
        }

        let etat = EtatConsensus::depuis_vues(periode, vues);

        // Rejet des états incohérents : la vérification n'est pas décorative.
        if !etat.verifier_invariants().is_empty() {
            return None;
        }

        Some(etat)
    }
}

/// **Reconfiguration à chaud** : retrait et réintégration d'agents.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reconfiguration {
    /// Agents retirés (indices).
    pub retraits: Vec<usize>,
    /// Agents réintégrés (indices), avec leur état de connaissance.
    pub reintegrations: Vec<(usize, Option<Etat>)>,
}

impl Reconfiguration {
    /// Applique la reconfiguration à un état, produisant un nouvel état.
    pub fn appliquer(&self, etat: &EtatConsensus) -> EtatConsensus {
        let mut vues = etat.vues.clone();
        for &i in &self.retraits {
            if let Some(v) = vues.get_mut(i) {
                v.actif = false;
                v.quiescent = false;
                // Un agent retiré ne peut plus porter d'état : conserver sa
                // valeur violerait l'invariant 5 (activité ↔ état).
                v.etat = None;
            }
        }
        for &(i, e) in &self.reintegrations {
            if let Some(v) = vues.get_mut(i) {
                v.actif = true;
                v.etat = e;
                v.quiescent = false;
            }
        }
        EtatConsensus::depuis_vues(etat.periode, vues)
    }

    /// Nombre d'agents touchés par la reconfiguration.
    pub fn ampleur(&self) -> usize {
        self.retraits.len() + self.reintegrations.len()
    }
}

/// **Métriques opérationnelles** de l'essaim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetriquesOperationnelles {
    /// Degré d'accord [0,1].
    pub accord: f64,
    /// Nombre d'agents actifs.
    pub actifs: usize,
    /// Nombre d'agents quiescents.
    pub quiescents: usize,
    /// Fraction d'agents actifs quiescents (efficacité de la quiescence).
    pub taux_quiescence: f64,
    /// Nombre d'agents actifs sans information.
    pub ignorants: usize,
    /// Nombre de valeurs distinctes détenues par les agents actifs.
    pub valeurs_distinctes: usize,
}

impl MetriquesOperationnelles {
    /// Calcule les métriques d'un état.
    pub fn calculer(etat: &EtatConsensus) -> Self {
        let actifs = etat.actifs;
        let quiescents = etat.vues.iter().filter(|v| v.actif && v.quiescent).count();
        let ignorants = etat.vues.iter().filter(|v| v.actif && v.etat.is_none()).count();

        let mut valeurs: Vec<i32> = etat
            .vues
            .iter()
            .filter(|v| v.actif)
            .filter_map(|v| v.etat.map(|(val, _)| val))
            .collect();
        valeurs.sort_unstable();
        valeurs.dedup();

        MetriquesOperationnelles {
            accord: etat.accord,
            actifs,
            quiescents,
            taux_quiescence: if actifs == 0 { 0.0 } else { quiescents as f64 / actifs as f64 },
            ignorants,
            valeurs_distinctes: valeurs.len(),
        }
    }
}

/// Construit un état opérationnel depuis les vues brutes du simulateur.
pub fn construire_etat(
    periode: usize,
    etats: &[Option<Etat>],
    actifs: &[bool],
    quiescents: &[bool],
) -> EtatConsensus {
    let n = etats.len().min(N);
    let vues = (0..n)
        .map(|i| VueAgent {
            agent: i,
            etat: etats[i],
            actif: actifs.get(i).copied().unwrap_or(true),
            quiescent: quiescents.get(i).copied().unwrap_or(false),
        })
        .collect();
    EtatConsensus::depuis_vues(periode, vues)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn etat_exemple() -> EtatConsensus {
        let vues = vec![
            VueAgent { agent: 0, etat: Some((5, 3)), actif: true, quiescent: true },
            VueAgent { agent: 1, etat: Some((5, 3)), actif: true, quiescent: false },
            VueAgent { agent: 2, etat: Some((5, 3)), actif: true, quiescent: true },
            VueAgent { agent: 3, etat: Some((2, 1)), actif: true, quiescent: false },
            VueAgent { agent: 4, etat: None, actif: false, quiescent: false },
        ];
        EtatConsensus::depuis_vues(9, vues)
    }

    #[test]
    fn dominante_et_accord() {
        let e = etat_exemple();
        assert_eq!(e.valeur_dominante, Some(5));
        assert_eq!(e.porteurs_dominante, 3);
        assert_eq!(e.actifs, 4);
        assert!((e.accord - 0.75).abs() < 1e-12);
    }

    #[test]
    fn invariants_conformes() {
        assert!(etat_exemple().verifier_invariants().is_empty());
    }

    #[test]
    fn invariants_detectent_incoherence() {
        let mut e = etat_exemple();
        e.porteurs_dominante = 99;
        let v = e.verifier_invariants();
        assert!(v.iter().any(|s| s.contains("invariant 3")), "violation détectée : {v:?}");
    }

    #[test]
    fn invariants_detectent_accord_faux() {
        let mut e = etat_exemple();
        e.accord = 0.1;
        let v = e.verifier_invariants();
        assert!(v.iter().any(|s| s.contains("invariant 2")), "violation détectée : {v:?}");
    }

    #[test]
    fn round_trip_serialisation() {
        let e = etat_exemple();
        let s = e.serialiser();
        let r = EtatConsensus::deserialiser(&s).expect("désérialisation");
        assert_eq!(r, e, "round-trip identique");
    }

    #[test]
    fn deserialisation_rejette_corrompu() {
        assert!(EtatConsensus::deserialiser("").is_none());
        assert!(EtatConsensus::deserialiser("GARBAGE").is_none());
        assert!(EtatConsensus::deserialiser("9|2\n0|1|0|5|3\n").is_none(), "lignes manquantes");
        assert!(EtatConsensus::deserialiser("9|1\n0|X|0|5|3\n").is_none(), "champ invalide");
    }

    #[test]
    fn reconfiguration_retrait() {
        let e = etat_exemple();
        let rc = Reconfiguration { retraits: vec![0, 1], reintegrations: vec![] };
        let apres = rc.appliquer(&e);
        assert_eq!(apres.actifs, 2);
        assert!(apres.verifier_invariants().is_empty());
        assert_eq!(rc.ampleur(), 2);
    }

    #[test]
    fn reconfiguration_reintegration() {
        let e = etat_exemple();
        let rc = Reconfiguration {
            retraits: vec![],
            reintegrations: vec![(4, Some((5, 3)))],
        };
        let apres = rc.appliquer(&e);
        assert_eq!(apres.actifs, 5);
        assert_eq!(apres.valeur_dominante, Some(5));
        assert!(apres.verifier_invariants().is_empty());
    }

    #[test]
    fn metriques_quiescence() {
        let m = MetriquesOperationnelles::calculer(&etat_exemple());
        assert_eq!(m.actifs, 4);
        assert_eq!(m.quiescents, 2);
        assert!((m.taux_quiescence - 0.5).abs() < 1e-12);
        assert_eq!(m.ignorants, 0);
        assert_eq!(m.valeurs_distinctes, 2);
    }

    #[test]
    fn etat_vide_sans_information() {
        let e = EtatConsensus::vide(30);
        // Un état vide n'a aucun agent actif : un agent actif sans état
        // violerait l'invariant 5 (activité ↔ état).
        assert_eq!(e.actifs, 0);
        assert_eq!(e.n_agents, 30);
        assert_eq!(e.valeur_dominante, None);
        assert_eq!(e.accord, 0.0);
        assert!(e.verifier_invariants().is_empty());
    }

    // ---------------------------------------------------------------- attaques
    // Ces tests vérifient que la couche v4 **détecte réellement** les états
    // incohérents. Sans eux, la vérification serait du théâtre.

    #[test]
    fn attaque_agent_inactif_porteur_etat() {
        let e = etat_exemple();
        let mut vues = e.vues.clone();
        vues[0].actif = false; // inactif mais conserve son état
        let corrompu = EtatConsensus::depuis_vues(e.periode, vues);
        let v = corrompu.verifier_invariants();
        assert!(!v.is_empty(), "l'incohérence activité/état doit être détectée");
        assert!(v.iter().any(|s| s.contains("invariant 5")), "violation attendue : {v:?}");
    }

    #[test]
    fn attaque_agent_actif_sans_etat() {
        let e = etat_exemple();
        let mut vues = e.vues.clone();
        vues[0].etat = None; // actif mais sans état
        let corrompu = EtatConsensus::depuis_vues(e.periode, vues);
        let v = corrompu.verifier_invariants();
        assert!(v.iter().any(|s| s.contains("invariant 5")), "violation attendue : {v:?}");
    }

    #[test]
    fn attaque_deserialisation_rejette_incoherent() {
        // Un agent inactif porteur d'état doit être rejeté à la relecture.
        let texte = "9|2\n0|0|0|5|3\n1|1|0|5|3\n";
        assert!(
            EtatConsensus::deserialiser(texte).is_none(),
            "un état incohérent ne doit pas être accepté"
        );
    }

    #[test]
    fn attaque_deserialisation_rejette_cardinalite() {
        // En-tête annonce 3 agents, 2 lignes fournies.
        let texte = "9|3\n0|1|0|5|3\n1|1|0|5|3\n";
        assert!(EtatConsensus::deserialiser(texte).is_none(), "cardinalité incohérente");
    }

    #[test]
    fn attaque_deserialisation_rejette_champ_non_numerique() {
        let texte = "9|1\n0|1|0|abc|3\n";
        assert!(EtatConsensus::deserialiser(texte).is_none(), "champ non numérique");
    }

    #[test]
    fn attaque_reconfiguration_efface_letat_du_retire() {
        // Après retrait, l'agent ne doit plus porter d'état — sinon l'état
        // résultant serait incohérent.
        let e = etat_exemple();
        let rc = Reconfiguration { retraits: vec![0], reintegrations: vec![] };
        let apres = rc.appliquer(&e);
        assert!(apres.vues[0].etat.is_none(), "l'état du retiré doit être effacé");
        assert!(apres.verifier_invariants().is_empty());
    }

    #[test]
    fn serialisation_aller_retour_stable() {
        let e = etat_exemple();
        let s = e.serialiser();
        let relu = EtatConsensus::deserialiser(&s).expect("relecture");
        assert_eq!(relu.serialiser(), s, "l'aller-retour doit être stable");
        assert_eq!(relu.accord, e.accord);
        assert_eq!(relu.actifs, e.actifs);
    }
}
