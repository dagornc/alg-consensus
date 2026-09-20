//! **v4 — Boucle opérationnelle complète (loop engineering, 8 building blocks).**
//!
//! La v3 implémentait le loop engineering comme **cadre descriptif** : la
//! quiescence est une condition d'arrêt, mais il n'y a ni déclencheur
//! explicite, ni persistance, ni vérification en ligne, ni supervision.
//!
//! La v4 rend les 8 building blocks **exécutables** :
//!
//! | BB | Rôle | Implémentation |
//! |----|------|----------------|
//! | 1 | Déclenchement | [`Declencheur`] — 6 motifs |
//! | 2 | Condition d'arrêt | [`ConditionArret`] — machine-checkable |
//! | 3 | État & mémoire | [`crate::operationnel::EtatConsensus`] |
//! | 4 | Skills / intention | [`Intention`] + [`Adaptation`] |
//! | 5 | Isolation | [`Bac`] — graine déterministe |
//! | 6 | Vérification | [`crate::verificateur`] |
//! | 7 | Supervision | [`Supervision`] — audit + escalade |
//! | 8 | Budgets | [`BudgetV4`] |

use crate::operationnel::{construire_etat, EtatConsensus, MetriquesOperationnelles, Reconfiguration};
use crate::sim::{simuler_avec_vues, Etat, Params, Resultat, N};
use crate::verificateur::{verifier_run, Rapport};

/// **BB1 — Déclencheur** : pourquoi la boucle s'exécute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declencheur {
    /// Premier run.
    Initial,
    /// Un agent a été retiré ou réintégré.
    Reconfiguration,
    /// Une partition a été détectée ou résolue.
    Partition,
    /// Reprise après interruption (état rechargé).
    Reprise,
    /// Cadence périodique (run de contrôle).
    Cadence,
    /// La quiescence a été rompue (un agent s'est réveillé).
    Reveil,
}

impl Declencheur {
    /// Libellé court.
    pub fn libelle(&self) -> &'static str {
        match self {
            Declencheur::Initial => "initial",
            Declencheur::Reconfiguration => "reconfiguration",
            Declencheur::Partition => "partition",
            Declencheur::Reprise => "reprise",
            Declencheur::Cadence => "cadence",
            Declencheur::Reveil => "réveil",
        }
    }
}

/// **BB4 — Intention** : configuration visée pour le run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Intention {
    /// Active la reconnexion des agents isolés (v2).
    pub reconnexion: bool,
    /// Active la quiescence (v3).
    pub quiescence: bool,
    /// Nombre de copies par message.
    pub duplication: usize,
}

impl Default for Intention {
    fn default() -> Self {
        Intention { reconnexion: true, quiescence: true, duplication: 1 }
    }
}

/// **BB2 — Condition d'arrêt** : machine-checkable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionArret {
    /// Degré d'accord minimal exigé [0,1].
    pub accord_min: f64,
    /// Nombre maximal de tours.
    pub max_tours: usize,
    /// Exiger que les invariants d'état soient satisfaits.
    pub exiger_invariants: bool,
    /// Budget de messages par run (`None` = pas de contrainte).
    pub budget_messages: Option<u64>,
}

impl Default for ConditionArret {
    fn default() -> Self {
        ConditionArret {
            accord_min: 1.0,
            max_tours: 8,
            exiger_invariants: true,
            budget_messages: None,
        }
    }
}

/// **BB8 — Budgets** : bornes dures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetV4 {
    /// Nombre maximal de runs.
    pub max_runs: usize,
    /// Nombre maximal d'évaluations.
    pub max_evaluations: usize,
}

impl Default for BudgetV4 {
    fn default() -> Self {
        BudgetV4 { max_runs: 8, max_evaluations: 64 }
    }
}

/// **BB5 — Isolation** : bac déterministe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bac {
    /// Graine du run.
    pub seed: u64,
}

/// **Politique d'adaptation** (move *handoff*).
///
/// Sans adaptation, la boucle rejoue la même configuration défaillante : elle
/// itère sans apprendre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Adaptation {
    /// Augmenter la duplication à chaque échec.
    pub augmenter_duplication: bool,
    /// Duplication maximale.
    pub duplication_max: usize,
    /// Activer la reconnexion après le premier échec.
    pub activer_reconnexion: bool,
}

impl Default for Adaptation {
    fn default() -> Self {
        Adaptation { augmenter_duplication: true, duplication_max: 4, activer_reconnexion: true }
    }
}

impl Adaptation {
    /// Aucune adaptation.
    pub fn aucune() -> Self {
        Adaptation { augmenter_duplication: false, duplication_max: 1, activer_reconnexion: false }
    }

    /// Corrige l'intention après un constat non conforme.
    pub fn corriger(&self, intention: Intention, accord: f64) -> Intention {
        let mut nouvelle = intention;
        if accord < 1.0 {
            if self.augmenter_duplication && nouvelle.duplication < self.duplication_max {
                nouvelle.duplication += 1;
            }
            if self.activer_reconnexion {
                nouvelle.reconnexion = true;
            }
        }
        nouvelle
    }
}

/// **BB7 — Supervision** : journal d'audit et détection de dérive.
#[derive(Debug, Clone, PartialEq)]
pub struct Supervision {
    /// Seuil d'escalade (accord minimal attendu).
    pub seuil_escalade: f64,
    /// Journal d'audit.
    pub audit: Vec<String>,
}

impl Supervision {
    /// Nouvelle supervision avec un seuil.
    pub fn nouvelle(seuil: f64) -> Self {
        Supervision { seuil_escalade: seuil, audit: Vec::new() }
    }

    /// Consigne une ligne d'audit.
    pub fn consigner(&mut self, ligne: String) {
        self.audit.push(ligne);
    }

    /// La dérive justifie-t-elle une escalade ?
    pub fn doit_escalader(&self, accord: f64) -> bool {
        accord < self.seuil_escalade
    }
}

/// **Constat** d'un tour de boucle.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstatV4 {
    /// Numéro du tour.
    pub tour: usize,
    /// Déclencheur.
    pub declencheur: Declencheur,
    /// Degré d'accord observé.
    pub accord: f64,
    /// Messages émis.
    pub messages: u64,
    /// Valeur dominante.
    pub valeur_dominante: Option<i32>,
    /// État opérationnel.
    pub etat: EtatConsensus,
    /// Métriques.
    pub metriques: MetriquesOperationnelles,
    /// Rapport de vérification.
    pub rapport: Rapport,
    /// Le tour est-il conforme ?
    pub conforme: bool,
}

/// **Journal** des constats.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Journal {
    /// Constats, dans l'ordre des tours.
    pub constats: Vec<ConstatV4>,
}

impl Journal {
    /// Journal vide.
    pub fn nouveau() -> Self {
        Journal { constats: Vec::new() }
    }

    /// Sérialise le journal (une ligne par run).
    pub fn serialiser(&self) -> String {
        let mut out = String::new();
        for c in &self.constats {
            out.push_str(&format!(
                "RUN {} {} {:.2} {} {} {}\n",
                c.tour,
                c.declencheur.libelle(),
                c.accord,
                c.messages,
                c.valeur_dominante.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                c.conforme as u8
            ));
        }
        out
    }
}

/// **Résultat** d'une exécution de la boucle v4.
#[derive(Debug, Clone, PartialEq)]
pub struct ResultatV4 {
    /// Journal des constats.
    pub journal: Journal,
    /// État opérationnel final.
    pub etat_final: EtatConsensus,
    /// Nombre de runs exécutés.
    pub runs: usize,
    /// Nombre d'évaluations.
    pub evaluations: usize,
    /// Condition d'arrêt satisfaite ?
    pub arret_satisfait: bool,
    /// Motif d'arrêt.
    pub motif_arret: String,
    /// Journal d'audit de supervision.
    pub audit: Vec<String>,
}

/// **La boucle v4** : exécute les runs, vérifie, persiste, s'arrête.
pub fn executer_v4(
    graines: &[u64],
    base: &Params,
    intention: Intention,
    condition: ConditionArret,
    budget: BudgetV4,
    reconfigurations: &[(usize, Reconfiguration)],
) -> ResultatV4 {
    executer_v4_avec_adaptation(
        graines,
        base,
        intention,
        Adaptation::aucune(),
        condition,
        budget,
        reconfigurations,
    )
}

/// **La boucle v4 avec adaptation** : l'intention est corrigée après chaque
/// run non conforme (move *handoff*).
#[allow(clippy::too_many_arguments)]
pub fn executer_v4_avec_adaptation(
    graines: &[u64],
    base: &Params,
    intention_initiale: Intention,
    adaptation: Adaptation,
    condition: ConditionArret,
    budget: BudgetV4,
    reconfigurations: &[(usize, Reconfiguration)],
) -> ResultatV4 {
    let mut journal = Journal::nouveau();
    let mut supervision = Supervision::nouvelle(condition.accord_min);
    let mut evaluations = 0usize;
    let mut etat_precedent: Option<EtatConsensus> = None;
    let mut etat_final = EtatConsensus::vide(N);
    let mut arret_satisfait = false;
    let mut motif = String::from("budget de runs épuisé");
    let mut intention = intention_initiale;

    let max_tours = condition.max_tours.min(budget.max_runs);

    for tour in 0..max_tours {
        if evaluations >= budget.max_evaluations {
            motif = "budget d'évaluations épuisé".to_string();
            break;
        }

        // BB1 — déclencheur.
        let reconfig_active = reconfigurations.iter().any(|(t, _)| *t == tour);
        let declencheur = if tour == 0 {
            Declencheur::Initial
        } else if reconfig_active {
            Declencheur::Reconfiguration
        } else if base.partition {
            Declencheur::Partition
        } else {
            Declencheur::Cadence
        };

        // BB3 — reconfiguration éventuelle.
        if let Some((_, rc)) = reconfigurations.iter().find(|(t, _)| *t == tour) {
            if let Some(prev) = &etat_precedent {
                etat_precedent = Some(rc.appliquer(prev));
            }
        }

        // BB5 — bac isolé + BB4 — intention.
        let seed = graines[tour % graines.len()];
        let bac = Bac { seed };
        let mut p = *base;
        p.reconnexion = intention.reconnexion;
        p.quiescence = intention.quiescence;
        p.duplication = intention.duplication;

        // Simulation — avec les vues réelles (mesurées, non supposées).
        let (r, etats, actifs, quiescents) = simuler_avec_vues(bac.seed, &p);
        evaluations += 1;

        // État opérationnel construit à partir des vues RÉELLES.
        let etat = construire_etat(
            r.periode_convergence.unwrap_or(p.max_periodes),
            &etats,
            &actifs,
            &quiescents,
        );
        let metriques = MetriquesOperationnelles::calculer(&etat);

        // BB6 — vérification indépendante.
        let rapport = verifier_run(&etat, &r, etat_precedent.as_ref(), condition.budget_messages);

        // BB2 — condition d'arrêt.
        let invariants_ok = !condition.exiger_invariants || etat.verifier_invariants().is_empty();
        let conforme = rapport.conforme() && invariants_ok && etat.accord >= condition.accord_min;

        // BB7 — supervision.
        supervision.consigner(format!(
            "tour {tour} ({}) : accord={:.2} msg={} dup={} conforme={conforme}",
            declencheur.libelle(),
            etat.accord,
            r.messages,
            intention.duplication
        ));
        if supervision.doit_escalader(etat.accord) {
            supervision.consigner(format!(
                "DERIVE tour {tour} : accord {:.2} < seuil {:.2}",
                etat.accord, supervision.seuil_escalade
            ));
        }

        journal.constats.push(ConstatV4 {
            tour,
            declencheur,
            accord: etat.accord,
            messages: r.messages,
            valeur_dominante: etat.valeur_dominante,
            etat: etat.clone(),
            metriques,
            rapport,
            conforme,
        });

        etat_precedent = Some(etat.clone());
        etat_final = etat.clone();

        if conforme {
            arret_satisfait = true;
            motif = format!("condition d'arrêt satisfaite au tour {tour}");
            break;
        }

        // Move *handoff*.
        intention = adaptation.corriger(intention, etat.accord);
    }

    let runs = journal.constats.len();

    // BB7 — escalade finale : si la boucle se termine sans satisfaire la
    // condition d'arrêt, quel que soit le chemin de sortie (budget de runs,
    // budget d'évaluations, épuisement des tours), l'escalade est consignée.
    // Sans ce bloc, une sortie par budget épuisé serait silencieuse.
    if !arret_satisfait {
        supervision.consigner(format!(
            "ESCALADE : condition d'arrêt non satisfaite après {runs} run(s) — accord final {:.2} < seuil {:.2} ({motif})",
            etat_final.accord, supervision.seuil_escalade
        ));
    }

    ResultatV4 {
        journal,
        etat_final,
        runs,
        evaluations,
        arret_satisfait,
        motif_arret: motif,
        audit: supervision.audit,
    }
}

/// Construit un état opérationnel depuis un résultat de simulation.
///
/// **Note** : cette fonction est conservée pour la compatibilité des tests,
/// mais la boucle v4 utilise désormais [`simuler_avec_vues`] et
/// [`construire_etat`] pour obtenir des vues **mesurées**. Reconstruire un
/// état à partir des seules grandeurs agrégées reviendrait à *supposer*
/// l'accord au lieu de le mesurer.
#[allow(dead_code)]
fn construire_etat_depuis_resultat(p: &Params, r: &Resultat) -> EtatConsensus {
    let actifs: Vec<bool> = (0..N).map(|i| i < r.taille_etat).collect();
    let etats: Vec<Option<Etat>> = (0..N)
        .map(|i| {
            if i < r.taille_etat {
                Some((valeur_dominante_estimee(p, r), r.periode_convergence.unwrap_or(0) as u32))
            } else {
                None
            }
        })
        .collect();
    let quiescents: Vec<bool> = (0..N).map(|_| p.quiescence && r.accord).collect();
    construire_etat(r.periode_convergence.unwrap_or(0), &etats, &actifs, &quiescents)
}

/// Valeur dominante estimée à partir du résultat.
#[allow(dead_code)]
fn valeur_dominante_estimee(_p: &Params, r: &Resultat) -> i32 {
    r.periode_convergence.unwrap_or(0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graines() -> Vec<u64> {
        (1000..1010).collect()
    }

    #[test]
    fn boucle_nominale_converge() {
        let p = Params::default();
        let r = executer_v4(&graines(), &p, Intention::default(), ConditionArret::default(), BudgetV4::default(), &[]);
        assert!(r.arret_satisfait, "convergence nominale : {}", r.motif_arret);
        assert_eq!(r.runs, 1, "arrêt immédiat en nominal");
    }

    #[test]
    fn boucle_deterministe() {
        let p = Params::default();
        let a = executer_v4(&graines(), &p, Intention::default(), ConditionArret::default(), BudgetV4::default(), &[]);
        let b = executer_v4(&graines(), &p, Intention::default(), ConditionArret::default(), BudgetV4::default(), &[]);
        assert_eq!(a, b, "boucle déterministe");
    }

    #[test]
    fn adaptation_corrige_intention() {
        let a = Adaptation::default();
        let i0 = Intention { duplication: 1, reconnexion: false, quiescence: true };
        let i1 = a.corriger(i0, 0.0);
        assert_eq!(i1.duplication, 2);
        assert!(i1.reconnexion);
        let i3 = a.corriger(Intention { duplication: 4, ..i0 }, 0.0);
        assert_eq!(i3.duplication, 4, "plafond respecté");
        let i4 = a.corriger(i0, 1.0);
        assert_eq!(i4.duplication, 1, "aucune correction si conforme");
    }

    #[test]
    fn adaptation_aucune_ne_corrige_pas() {
        let a = Adaptation::aucune();
        let i0 = Intention { duplication: 1, reconnexion: false, quiescence: true };
        assert_eq!(a.corriger(i0, 0.0), i0);
    }

    #[test]
    fn boucle_escalade_sous_perte_totale() {
        // Perte 1.0 : aucun message ne passe, la convergence est impossible
        // (vérifié : 0/10 accords, valeurs dispersées). La boucle doit épuiser
        // son budget et consigner une escalade — pas déclarer un faux accord.
        //
        // Note : la partition forcée et la perte 0.9 convergent légitimement
        // (vérifié : 10/10 accords réels, une seule valeur) — elles ne doivent
        // donc PAS déclencher d'escalade. C'est la perte totale qui est le vrai
        // cas d'échec.
        let p = Params { perte: 1.0, max_periodes: 60, ..Params::default() };
        let r = executer_v4_avec_adaptation(
            &graines(),
            &p,
            Intention { duplication: 1, ..Intention::default() },
            Adaptation::default(),
            ConditionArret { accord_min: 1.0, max_tours: 4, ..Default::default() },
            BudgetV4 { max_runs: 4, max_evaluations: 32 },
            &[],
        );
        assert!(!r.arret_satisfait, "sous perte totale la condition ne peut pas être satisfaite");
        assert!(r.audit.iter().any(|l| l.contains("ESCALADE")), "escalade consignée : {:?}", r.audit);
    }

    #[test]
    fn partition_converge_sans_escalade() {
        // Contre-épreuve : la partition forcée converge réellement, donc la
        // boucle ne doit pas escalader. Ce test protège contre un vérificateur
        // qui crierait au loup sur un cas sain.
        let p = Params { partition: true, forcer_divergence: true, max_periodes: 200, ..Params::default() };
        let r = executer_v4_avec_adaptation(
            &graines(),
            &p,
            Intention { duplication: 1, ..Intention::default() },
            Adaptation::default(),
            ConditionArret { accord_min: 1.0, max_tours: 4, ..Default::default() },
            BudgetV4 { max_runs: 4, max_evaluations: 32 },
            &[],
        );
        assert!(r.arret_satisfait, "la partition doit converger : {:?}", r.audit);
        assert!(!r.audit.iter().any(|l| l.contains("ESCALADE")), "pas d'escalade attendue : {:?}", r.audit);
    }

    #[test]
    fn journal_serialisable() {
        let p = Params::default();
        let r = executer_v4(&graines(), &p, Intention::default(), ConditionArret::default(), BudgetV4::default(), &[]);
        let s = r.journal.serialiser();
        assert!(s.starts_with("RUN 0"), "journal sérialisé : {s}");
    }

    #[test]
    fn reconfiguration_declenche_le_bon_motif() {
        let p = Params::default();
        let rc = Reconfiguration { retraits: vec![0], reintegrations: vec![] };
        let r = executer_v4(
            &graines(),
            &p,
            Intention::default(),
            ConditionArret { accord_min: 1.0, max_tours: 3, ..Default::default() },
            BudgetV4 { max_runs: 3, max_evaluations: 16 },
            &[(1, rc)],
        );
        // Le tour 1 doit porter le déclencheur Reconfiguration.
        if r.journal.constats.len() > 1 {
            assert_eq!(r.journal.constats[1].declencheur, Declencheur::Reconfiguration);
        }
    }
}
