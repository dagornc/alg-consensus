//! Inventaire des faiblesses residuelles de la v2.
//!
//! Mesure les metriques qui pourraient encore etre ameliorees :
//! - cout en messages (T5D est le pire cas : 9094 messages)
//! - periode de convergence
//! - comportement sous perte et partition

use consensus_rs::sim::{simuler, Params};

fn stats(test: &str, p: &Params, lo: u64, hi: u64) -> (usize, u64, f64, f64) {
    let mut acc = 0usize;
    let mut msg = 0u64;
    let mut convs: Vec<f64> = Vec::new();
    let mut n = 0usize;
    for s in lo..=hi {
        let r = simuler(s, p);
        n += 1;
        if r.accord {
            acc += 1;
        }
        msg += r.messages;
        if let Some(c) = r.periode_convergence {
            convs.push(c as f64);
        }
    }
    convs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = if convs.is_empty() { f64::NAN } else { convs[convs.len() / 2] };
    let p95 = if convs.is_empty() {
        f64::NAN
    } else {
        convs[((convs.len() as f64 * 0.95) as usize).min(convs.len() - 1)]
    };
    println!(
        "{test:5} accord={acc:4}/{n} msg_total={msg:9} msg_moy={:8.1} conv_med={med:5.1} conv_p95={p95:5.1}",
        msg as f64 / n as f64
    );
    (acc, msg, med, p95)
}

fn main() {
    println!("=== Faiblesses residuelles de la v2 (1000 graines) ===");
    let tests: Vec<(&str, Params)> = vec![
        ("T1", Params { ..Default::default() }),
        ("T3", Params { perte: 0.25, ..Default::default() }),
        ("T4", Params { duplication: 2, ..Default::default() }),
        ("T5", Params { partition: true, ..Default::default() }),
        ("T5D", Params { partition: true, forcer_divergence: true, ..Default::default() }),
        ("T8", Params { retrait: 3, t_retrait: Some(3), reconnexion: true, ..Default::default() }),
        ("T9", Params { partition: true, ..Default::default() }),
    ];
    for (nom, p) in &tests {
        stats(nom, p, 1001, 2000);
    }
}
