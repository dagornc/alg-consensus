//! CLI du simulateur ALG_CONSENSUS — interface compatible avec `sim_consensus.py`.
//!
//! Usage :
//! ```text
//! consensus_rs --test T1 --seeds 1001-1100 --out resultats.csv
//! consensus_rs --all --out resultats.csv
//! ```
//!
//! Sortie : CSV `(test, seed, periode_convergence, accord, messages,
//! taille_etat, periode_refusion, divergence_reelle)` — format identique à
//! la référence Python.

use consensus_rs::sim::{simuler, Params};
use std::io::Write;

/// Paramètres par test, identiques au tableau §4.7.1 de la spécification.
fn params_pour(test: &str) -> Params {
    match test {
        "T1" => Params {
            perte: 0.0,
            ..Default::default()
        },
        "T2" => Params {
            perte: 0.0,
            ..Default::default()
        },
        "T3" => Params {
            perte: 0.25,
            ..Default::default()
        },
        "T4" => Params {
            perte: 0.0,
            duplication: 2,
            ..Default::default()
        },
        "T5" => Params {
            perte: 0.0,
            partition: true,
            ..Default::default()
        },
        "T5D" => Params {
            perte: 0.0,
            partition: true,
            forcer_divergence: true,
            ..Default::default()
        },
        "T6" => Params {
            perte: 0.0,
            ..Default::default()
        },
        "T8" => Params {
            perte: 0.0,
            retrait: 3,
            t_retrait: Some(3),
            ..Default::default()
        },
        "T9" => Params {
            perte: 0.0,
            partition: true,
            ..Default::default()
        },
        _ => Params::default(),
    }
}

/// Paramètres v2 : mêmes tests, avec la reconnexion activée.
fn params_pour_v2(test: &str) -> Params {
    Params {
        reconnexion: true,
        ..params_pour(test)
    }
}

/// Paramètres v3 : v2 + quiescence (arrêt de l'émission quand l'état est
/// stable, avec réveil périodique).
fn params_pour_v3(test: &str) -> Params {
    Params {
        reconnexion: true,
        quiescence: true,
        ..params_pour(test)
    }
}

const TESTS: [&str; 9] = ["T1", "T2", "T3", "T4", "T5", "T5D", "T6", "T8", "T9"];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut test = String::from("T1");
    let mut seeds = String::from("1001-1100");
    let mut out = String::from("resultats.csv");
    let mut all = false;
    let mut v2 = false;
    let mut v3 = false;
    let mut v4 = false;
    let mut etat = false;
    let mut journal = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--test" => {
                i += 1;
                if i < args.len() {
                    test = args[i].clone();
                }
            }
            "--seeds" => {
                i += 1;
                if i < args.len() {
                    seeds = args[i].clone();
                }
            }
            "--out" => {
                i += 1;
                if i < args.len() {
                    out = args[i].clone();
                }
            }
            "--all" => all = true,
            "--v2" => v2 = true,
            "--v3" => v3 = true,
            "--v4" => v4 = true,
            "--etat" => etat = true,
            "--journal" => journal = true,
            "--help" | "-h" => {
                println!("Usage: consensus_rs [--test T1] [--seeds 1001-1100] [--out f.csv] [--all] [--v2] [--v3] [--v4] [--etat] [--journal]");
                println!();
                println!("  --v2       active la reconnexion des agents isoles (ALG_CONSENSUS v2)");
                println!("  --v3       active la quiescence en plus de la v2 (ALG_CONSENSUS v3)");
                println!("  --v4       execute la boucle operationnelle (loop engineering, 8 building blocks)");
                println!("  --etat     affiche l'etat operationnel final serialise (avec --v4)");
                println!("  --journal  affiche le journal d'audit de la boucle (avec --v4)");
                return;
            }
            other => {
                eprintln!("argument inconnu : {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    // Analyse de la plage de graines.
    let parts: Vec<&str> = seeds.split('-').collect();
    if parts.len() != 2 {
        eprintln!("--seeds attendu au format A-B (ex. 1001-1100)");
        std::process::exit(2);
    }
    let lo: u64 = parts[0].parse().expect("graine de début invalide");
    let hi: u64 = parts[1].parse().expect("graine de fin invalide");

    let tests: Vec<&str> = if all {
        TESTS.to_vec()
    } else {
        vec![test.as_str()]
    };

    // ---------------------------------------------------------------- mode v4
    // Boucle opérationnelle (loop engineering) : exécute les 8 building blocks
    // sur la plage de graines demandée, puis affiche l'état et/ou le journal.
    if v4 {
        use consensus_rs::boucle_v4::{
            executer_v4, BudgetV4, ConditionArret, Intention,
        };
        let graines: Vec<u64> = (lo..=hi).collect();
        let base = if v3 {
            params_pour_v3(tests[0])
        } else if v2 {
            params_pour_v2(tests[0])
        } else {
            params_pour(tests[0])
        };
        let condition = ConditionArret {
            accord_min: 1.0,
            max_tours: 8,
            exiger_invariants: true,
            budget_messages: None,
        };
        let budget = BudgetV4 { max_runs: 8, max_evaluations: 64 };
        let r = executer_v4(&graines, &base, Intention::default(), condition, budget, &[]);

        println!("=== v4 — boucle opérationnelle (loop engineering) ===");
        println!("graines        : {} ({} runs)", graines.len(), r.runs);
        println!("évaluations    : {}", r.evaluations);
        println!("arrêt satisfait: {}", r.arret_satisfait);
        println!("motif d'arrêt  : {}", r.motif_arret);
        println!(
            "état final     : accord={:.3} valeur={:?} actifs={}",
            r.etat_final.accord, r.etat_final.valeur_dominante, r.etat_final.actifs
        );
        println!("invariants     : {:?}", r.etat_final.verifier_invariants());

        if journal {
            println!("\n--- journal d'audit ---");
            for a in &r.audit {
                println!("  {a}");
            }
        }
        if etat {
            println!("\n--- état sérialisé ---");
            print!("{}", r.etat_final.serialiser());
        }
        return;
    }

    // Collecte des résultats.
    let mut lignes: Vec<(String, u64, Option<usize>, bool, u64, usize, Option<usize>, bool)> =
        Vec::new();
    for t in &tests {
        let p = if v3 {
            params_pour_v3(t)
        } else if v2 {
            params_pour_v2(t)
        } else {
            params_pour(t)
        };
        for s in lo..=hi {
            let r = simuler(s, &p);
            lignes.push((
                t.to_string(),
                s,
                r.periode_convergence,
                r.accord,
                r.messages,
                r.taille_etat,
                r.periode_refusion,
                r.divergence_reelle,
            ));
        }
    }

    // Écriture du CSV.
    let mut f = std::fs::File::create(&out).expect("création du CSV impossible");
    writeln!(
        f,
        "test,seed,periode_convergence,accord,messages,taille_etat,periode_refusion,divergence_reelle"
    )
    .unwrap();
    for (t, s, conv, accord, msg, taille, refus, div) in &lignes {
        writeln!(
            f,
            "{},{},{},{},{},{},{},{}",
            t,
            s,
            conv.map(|c| c.to_string()).unwrap_or_default(),
            *accord as u8,
            msg,
            taille,
            refus.map(|r| r.to_string()).unwrap_or_default(),
            *div as u8
        )
        .unwrap();
    }

    // Résumé par test (même format que la référence Python).
    for t in &tests {
        let sub: Vec<_> = lignes.iter().filter(|l| l.0 == *t).collect();
        let n = sub.len();
        if n == 0 {
            continue;
        }
        let mut convs: Vec<usize> = sub.iter().filter_map(|l| l.2).collect();
        convs.sort_unstable();
        let accords = sub.iter().filter(|l| l.3).count();
        let med = if convs.is_empty() {
            f64::NAN
        } else if convs.len() % 2 == 1 {
            convs[convs.len() / 2] as f64
        } else {
            (convs[convs.len() / 2 - 1] + convs[convs.len() / 2]) as f64 / 2.0
        };
        let msg_moy = sub.iter().map(|l| l.4).sum::<u64>() as f64 / n as f64;
        let mut refus: Vec<usize> = sub.iter().filter_map(|l| l.6).collect();
        refus.sort_unstable();
        let ndiv = sub.iter().filter(|l| l.7).count();
        let extra = if refus.is_empty() {
            String::new()
        } else {
            let rmed = if refus.len() % 2 == 1 {
                refus[refus.len() / 2] as f64
            } else {
                (refus[refus.len() / 2 - 1] + refus[refus.len() / 2]) as f64 / 2.0
            };
            format!(
                " refusion_med={} refusion_max={} graines_divergentes={}/{}",
                rmed,
                refus.last().unwrap(),
                ndiv,
                n
            )
        };
        println!(
            "{}: n={} accord={}/{} mediane_conv={} messages_moy={:.1}{}",
            t, n, accords, n, med, msg_moy, extra
        );
    }
    println!("-> {out}");
}
