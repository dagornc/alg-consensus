//! **v4 — Démonstration du cycle opérationnel complet.**
//!
//! Exerce les 8 building blocks du loop engineering sur l'algorithme de
//! consensus : déclenchement, condition d'arrêt machine-checkable, état
//! persistant, intention, bac isolé, vérification indépendante, supervision,
//! budgets durs.
//!
//! Exécution : `cargo run --release --example operationnel_v4`

use consensus_rs::boucle_v4::*;
use consensus_rs::operationnel::{EtatConsensus, Reconfiguration};
use consensus_rs::sim::{simuler, Params};
use consensus_rs::verificateur::{verifier_run, verifier_semi_treillis};

fn main() {
    println!("=== v4 — cycle opérationnel du consensus ===\n");

    let graines: Vec<u64> = (1000..1010).collect();

    // ---------------------------------------------------------------- 1. nominal
    println!("--- 1. Cycle nominal (partition + quiescence) ---");
    let p_nominal = Params {
        partition: true,
        forcer_divergence: true,
        max_periodes: 200,
        ..Params::default()
    };
    let condition = ConditionArret {
        accord_min: 1.0,
        max_tours: 8,
        exiger_invariants: true,
        budget_messages: None,
    };
    let budget = BudgetV4 { max_runs: 8, max_evaluations: 64 };

    let r = executer_v4(&graines, &p_nominal, Intention::default(), condition, budget, &[]);
    println!("runs={} evaluations={} arret={} motif={}", r.runs, r.evaluations, r.arret_satisfait, r.motif_arret);
    println!("accord final={:.3} valeur={:?} actifs={}", r.etat_final.accord, r.etat_final.valeur_dominante, r.etat_final.actifs);
    println!("invariants : {:?}", r.etat_final.verifier_invariants());
    for c in &r.journal.constats {
        println!(
            "  tour {} ({}) accord={:.3} msgs={} conforme={}",
            c.tour, c.declencheur.libelle(), c.accord, c.messages, c.conforme
        );
    }

    // ------------------------------------------------------- 2. persistance
    println!("\n--- 2. Persistance et reprise ---");
    let serialise = r.etat_final.serialiser();
    println!("état sérialisé ({} octets) :", serialise.len());
    println!("{}", serialise.lines().take(4).collect::<Vec<_>>().join("\n"));
    match EtatConsensus::deserialiser(&serialise) {
        Some(e) => println!("relecture OK : accord={:.3} actifs={} invariants={:?}", e.accord, e.actifs, e.verifier_invariants()),
        None => println!("ERREUR de relecture"),
    }

    // ------------------------------------------------------- 3. reconfiguration
    println!("\n--- 3. Reconfiguration dynamique ---");
    let rc = Reconfiguration { retraits: vec![0, 1, 2], reintegrations: vec![] };
    let r2 = executer_v4(&graines, &p_nominal, Intention::default(), condition, budget, &[(1, rc)]);
    println!("runs={} arret={} motif={}", r2.runs, r2.arret_satisfait, r2.motif_arret);
    for c in &r2.journal.constats {
        println!(
            "  tour {} ({}) accord={:.3} actifs={}",
            c.tour, c.declencheur.libelle(), c.accord, c.etat.actifs
        );
    }

    // ------------------------------------------------------- 4. mode dégradé
    println!("\n--- 4. Mode dégradé (perte totale) + adaptation ---");
    let p_degrade = Params { perte: 1.0, max_periodes: 60, ..Params::default() };
    let r3 = executer_v4_avec_adaptation(
        &graines,
        &p_degrade,
        Intention { duplication: 1, ..Intention::default() },
        Adaptation::default(),
        ConditionArret { accord_min: 1.0, max_tours: 4, ..Default::default() },
        BudgetV4 { max_runs: 4, max_evaluations: 32 },
        &[],
    );
    println!("runs={} arret={} motif={}", r3.runs, r3.arret_satisfait, r3.motif_arret);
    for c in &r3.journal.constats {
        println!("  tour {} accord={:.3} msgs={} conforme={}", c.tour, c.accord, c.messages, c.conforme);
    }
    println!("--- audit de supervision ---");
    for a in &r3.audit {
        println!("  {}", a);
    }

    // ------------------------------------------------------- 5. vérification
    println!("\n--- 5. Vérification indépendante (maker/checker) ---");
    let rapport = verifier_run(&r.etat_final, &simuler(1000, &p_nominal), None, None);
    println!("conforme={} violations={}", rapport.conforme(), rapport.violations());
    println!("semi-treillis : {:?}", verifier_semi_treillis());

    // ------------------------------------------------------- 6. attaque
    println!("\n--- 6. Attaque : états corrompus ---");

    // 6a. Corruption structurelle : un agent inactif porteur d'un état.
    let mut lignes: Vec<String> = serialise.lines().map(|s| s.to_string()).collect();
    // Ligne 1 = agent 0 : on le marque inactif (0) tout en gardant son état.
    let champs: Vec<&str> = lignes[1].split('|').collect();
    lignes[1] = format!("{}|0|{}|{}|{}", champs[0], champs[2], champs[3], champs[4]);
    let corrompu = lignes.join("\n");
    match EtatConsensus::deserialiser(&corrompu) {
        Some(e) => println!("6a ACCEPTÉ (défaut !) invariants={:?}", e.verifier_invariants()),
        None => println!("6a rejeté comme attendu (agent inactif porteur d'état)"),
    }

    // 6b. Corruption de format : champ non numérique.
    let corrompu2 = serialise.replace("|1|1|", "|1|1|abc|");
    match EtatConsensus::deserialiser(&corrompu2) {
        Some(_) => println!("6b ACCEPTÉ (défaut !)"),
        None => println!("6b rejeté comme attendu (champ non numérique)"),
    }

    // 6c. Corruption de cardinalité : une ligne manquante.
    let mut lignes3: Vec<String> = serialise.lines().map(|s| s.to_string()).collect();
    lignes3.pop();
    match EtatConsensus::deserialiser(&lignes3.join("\n")) {
        Some(_) => println!("6c ACCEPTÉ (défaut !)"),
        None => println!("6c rejeté comme attendu (cardinalité incohérente)"),
    }
}
