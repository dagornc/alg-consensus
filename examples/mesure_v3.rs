//! Mesure v2 vs v3 (quiescence) sur les 9 tests, 1000 graines.
//!
//! Critere de succes : la v3 doit reduire les messages SANS degrader
//! l'accord ni la periode de convergence.

use consensus_rs::sim::{simuler, Params};

fn params(test: &str, v3: bool) -> Params {
    let base = match test {
        "T1" | "T2" | "T6" => Params { ..Default::default() },
        "T3" => Params { perte: 0.25, ..Default::default() },
        "T4" => Params { duplication: 2, ..Default::default() },
        "T5" | "T9" => Params { partition: true, ..Default::default() },
        "T5D" => Params { partition: true, forcer_divergence: true, ..Default::default() },
        "T8" => Params { retrait: 3, t_retrait: Some(3), reconnexion: true, ..Default::default() },
        _ => Params { ..Default::default() },
    };
    Params { quiescence: v3, ..base }
}

struct Agg {
    accord: usize,
    msg: u64,
    conv: Vec<usize>,
    n: usize,
}

fn mesurer(test: &str, v3: bool, lo: u64, hi: u64) -> Agg {
    let p = params(test, v3);
    let mut a = Agg { accord: 0, msg: 0, conv: Vec::new(), n: 0 };
    for s in lo..=hi {
        let r = simuler(s, &p);
        a.n += 1;
        if r.accord {
            a.accord += 1;
        }
        a.msg += r.messages;
        if let Some(c) = r.periode_convergence {
            a.conv.push(c);
        }
    }
    a.conv.sort_unstable();
    a
}

fn main() {
    let tests = ["T1", "T3", "T4", "T5", "T5D", "T8", "T9"];
    println!("test  | v2 accord  msg_moy | v3 accord  msg_moy | delta_msg | conv_med v2/v3");
    println!("------|--------------------|--------------------|-----------|---------------");
    let mut total_v2 = 0u64;
    let mut total_v3 = 0u64;
    let mut regressions = 0;
    for t in &tests {
        let a2 = mesurer(t, false, 1001, 2000);
        let a3 = mesurer(t, true, 1001, 2000);
        total_v2 += a2.msg;
        total_v3 += a3.msg;
        if a3.accord < a2.accord {
            regressions += 1;
        }
        let m2 = a2.msg as f64 / a2.n as f64;
        let m3 = a3.msg as f64 / a3.n as f64;
        let delta = (m3 - m2) / m2 * 100.0;
        let c2 = a2.conv.get(a2.conv.len() / 2).copied().unwrap_or(0);
        let c3 = a3.conv.get(a3.conv.len() / 2).copied().unwrap_or(0);
        println!(
            "{t:5} | {:4}/{} {:8.1} | {:4}/{} {:8.1} | {delta:+8.1}% | {c2:3} / {c3:3}",
            a2.accord, a2.n, m2, a3.accord, a3.n, m3
        );
    }
    println!();
    println!(
        "TOTAL messages : v2={total_v2}  v3={total_v3}  delta={:+.1}%",
        (total_v3 as f64 - total_v2 as f64) / total_v2 as f64 * 100.0
    );
    println!("Tests avec regression d'accord : {regressions}");
}
