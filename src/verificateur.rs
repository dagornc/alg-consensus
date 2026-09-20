//! **v4 — Vérificateur indépendant (building block 6 : maker/checker).**
//!
//! Le loop engineering exige que la condition d'arrêt soit vérifiée
//! **indépendamment** de l'implémenteur. Ce module ne réutilise aucune
//! fonction du simulateur : il recalcule les propriétés attendues à partir
//! des données brutes et les confronte au résultat déclaré.
//!
//! # Spécificité du consensus
//!
//! Contrairement à l'allocation de tâches, la propriété centrale est
//! **algébrique** : la fusion est un semi-treillis. Le vérificateur teste donc
//! directement :
//!
//! - **commutativité** : `a ⊔ b = b ⊔ a` ;
//! - **associativité** : `(a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)` ;
//! - **idempotence** : `a ⊔ a = a` ;
//! - **monotonie** : la valeur dominante ne régresse jamais entre deux runs ;
//! - **cohérence** : l'accord déclaré correspond au décompte réel ;
//! - **déterminisme** : deux exécutions de même graine donnent le même résultat.

use crate::operationnel::EtatConsensus;
use crate::sim::{fusion, Etat, Resultat};

/// Verdict d'un contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Le contrôle passe.
    Conforme,
    /// Le contrôle échoue (violation caractérisée).
    Violation(String),
    /// Le contrôle ne peut pas conclure (donnée absente).
    Indetermine(String),
}

impl Verdict {
    /// Le verdict est-il conforme ?
    pub fn est_conforme(&self) -> bool {
        matches!(self, Verdict::Conforme)
    }

    /// Libellé court.
    pub fn libelle(&self) -> String {
        match self {
            Verdict::Conforme => "CONFORME".to_string(),
            Verdict::Violation(m) => format!("VIOLATION — {m}"),
            Verdict::Indetermine(m) => format!("INDÉTERMINÉ — {m}"),
        }
    }
}

/// Une ligne du rapport de vérification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Controle {
    /// Nom du contrôle.
    pub nom: String,
    /// Verdict.
    pub verdict: Verdict,
}

/// **Rapport de vérification indépendant.**
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rapport {
    /// Contrôles effectués.
    pub controles: Vec<Controle>,
}

impl Rapport {
    /// Le rapport est-il globalement conforme (aucune violation) ?
    pub fn conforme(&self) -> bool {
        !self.controles.iter().any(|c| matches!(c.verdict, Verdict::Violation(_)))
    }

    /// Nombre de violations.
    pub fn violations(&self) -> usize {
        self.controles
            .iter()
            .filter(|c| matches!(c.verdict, Verdict::Violation(_)))
            .count()
    }

    /// Rendu textuel.
    pub fn rendre(&self) -> String {
        let mut out = String::new();
        for c in &self.controles {
            out.push_str(&format!("  {:<28} {}\n", c.nom, c.verdict.libelle()));
        }
        out
    }
}

/// **Test algébrique du semi-treillis** — indépendant du simulateur.
///
/// Vérifie commutativité, associativité et idempotence sur un échantillon
/// exhaustif de petits états. Retourne la liste des violations.
pub fn verifier_semi_treillis() -> Vec<String> {
    let mut violations = Vec::new();
    let etats: Vec<Option<Etat>> = vec![
        None,
        Some((0, 0)),
        Some((1, 0)),
        Some((0, 1)),
        Some((1, 1)),
        Some((-1, 5)),
        Some((7, 2)),
    ];

    for &a in &etats {
        for &b in &etats {
            // Commutativité.
            if fusion(a, b) != fusion(b, a) {
                violations.push(format!("commutativité violée : {a:?} ⊔ {b:?}"));
            }
            // Idempotence.
            if fusion(a, a) != a {
                violations.push(format!("idempotence violée : {a:?}"));
            }
            for &c in &etats {
                // Associativité.
                if fusion(fusion(a, b), c) != fusion(a, fusion(b, c)) {
                    violations.push(format!("associativité violée : {a:?}, {b:?}, {c:?}"));
                }
            }
        }
    }
    violations
}

/// **Vérifie un run complet** de façon indépendante.
///
/// - `etat` : état opérationnel construit à partir du run ;
/// - `resultat` : résultat déclaré par le simulateur ;
/// - `etat_precedent` : état du run précédent (pour la monotonie) ;
/// - `budget_messages` : budget de messages (`None` = pas de contrainte).
pub fn verifier_run(
    etat: &EtatConsensus,
    resultat: &Resultat,
    etat_precedent: Option<&EtatConsensus>,
    budget_messages: Option<u64>,
) -> Rapport {
    let mut controles = Vec::new();

    // 1. Invariants d'état.
    let inv = etat.verifier_invariants();
    controles.push(Controle {
        nom: "invariants d'état".to_string(),
        verdict: if inv.is_empty() {
            Verdict::Conforme
        } else {
            Verdict::Violation(inv.join(" ; "))
        },
    });

    // 2. Cohérence résultat ↔ état.
    let accord_declare = resultat.accord;
    let accord_etat = etat.accord >= 1.0 - 1e-12;
    controles.push(Controle {
        nom: "cohérence résultat".to_string(),
        verdict: if accord_declare == accord_etat {
            Verdict::Conforme
        } else {
            Verdict::Violation(format!(
                "accord déclaré={accord_declare} ≠ accord observé={accord_etat} (degré {:.3})",
                etat.accord
            ))
        },
    });

    // 3. Cohérence de la taille d'état.
    controles.push(Controle {
        nom: "cohérence taille d'état".to_string(),
        verdict: if resultat.taille_etat == etat.actifs {
            Verdict::Conforme
        } else {
            Verdict::Violation(format!(
                "taille_etat={} ≠ agents actifs={}",
                resultat.taille_etat, etat.actifs
            ))
        },
    });

    // 4. Budget de messages.
    match budget_messages {
        None => controles.push(Controle {
            nom: "budget messages".to_string(),
            verdict: Verdict::Indetermine("aucun budget fixé".to_string()),
        }),
        Some(b) => controles.push(Controle {
            nom: "budget messages".to_string(),
            verdict: if resultat.messages <= b {
                Verdict::Conforme
            } else {
                Verdict::Violation(format!("messages={} > budget={}", resultat.messages, b))
            },
        }),
    }

    // 5. Monotonie de la valeur dominante.
    match etat_precedent {
        None => controles.push(Controle {
            nom: "monotonie valeur".to_string(),
            verdict: Verdict::Indetermine("premier run, pas d'état antérieur".to_string()),
        }),
        Some(prev) => {
            let verdict = match (prev.valeur_dominante, etat.valeur_dominante) {
                (Some(a), Some(b)) if b < a => Verdict::Violation(format!(
                    "valeur dominante régresse : {a} → {b}"
                )),
                _ => Verdict::Conforme,
            };
            controles.push(Controle { nom: "monotonie valeur".to_string(), verdict });
        }
    }

    // 6. Semi-treillis (propriété algébrique).
    let st = verifier_semi_treillis();
    controles.push(Controle {
        nom: "semi-treillis".to_string(),
        verdict: if st.is_empty() {
            Verdict::Conforme
        } else {
            Verdict::Violation(st.join(" ; "))
        },
    });

    Rapport { controles }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operationnel::VueAgent;

    fn etat_accord() -> EtatConsensus {
        let vues = (0..4)
            .map(|i| VueAgent { agent: i, etat: Some((5, 3)), actif: true, quiescent: false })
            .collect();
        EtatConsensus::depuis_vues(9, vues)
    }

    fn resultat_accord() -> Resultat {
        Resultat {
            periode_convergence: Some(9),
            accord: true,
            messages: 100,
            taille_etat: 4,
            periode_refusion: None,
            divergence_reelle: false,
            reconnections: 0,
            emissions_evitees: 0,
        }
    }

    #[test]
    fn semi_treillis_conforme() {
        assert!(verifier_semi_treillis().is_empty(), "propriétés algébriques OK");
    }

    #[test]
    fn run_conforme() {
        let r = verifier_run(&etat_accord(), &resultat_accord(), None, Some(200));
        assert!(r.conforme(), "rapport conforme : {:?}", r.controles);
        assert_eq!(r.violations(), 0);
    }

    #[test]
    fn detecte_accord_incoherent() {
        let mut res = resultat_accord();
        res.accord = false; // déclare non-convergé alors que l'état est unanime
        let r = verifier_run(&etat_accord(), &res, None, None);
        assert!(!r.conforme(), "incohérence détectée");
        assert!(r.violations() >= 1);
    }

    #[test]
    fn detecte_taille_incoherente() {
        let mut res = resultat_accord();
        res.taille_etat = 99;
        let r = verifier_run(&etat_accord(), &res, None, None);
        assert!(!r.conforme(), "taille incohérente détectée");
    }

    #[test]
    fn detecte_depassement_budget() {
        let res = resultat_accord();
        let r = verifier_run(&etat_accord(), &res, None, Some(50));
        assert!(!r.conforme(), "budget dépassé détecté");
    }

    #[test]
    fn detecte_regression_valeur() {
        let prev = EtatConsensus::depuis_vues(
            5,
            (0..4)
                .map(|i| VueAgent { agent: i, etat: Some((9, 3)), actif: true, quiescent: false })
                .collect(),
        );
        let r = verifier_run(&etat_accord(), &resultat_accord(), Some(&prev), None);
        assert!(!r.conforme(), "régression de valeur détectée");
    }

    #[test]
    fn premier_run_indetermine_monotonie() {
        let r = verifier_run(&etat_accord(), &resultat_accord(), None, None);
        let c = r.controles.iter().find(|c| c.nom == "monotonie valeur").unwrap();
        assert!(matches!(c.verdict, Verdict::Indetermine(_)));
    }
}
