//! Diagnostic : pourquoi la quiescence casse-t-elle la convergence ?
//!
//! Hypothese : un agent devient quiescent alors que ses VOISINS n'ont pas
//! encore recu sa valeur. Il se tait, et l'information ne circule plus.

use consensus_rs::sim::{simuler, Params};

fn main() {
    let p = Params { partition: true, forcer_divergence: true, quiescence: true, ..Default::default() };
    for seed in [1001u64, 1002, 1003] {
        let r = simuler(seed, &p);
        println!(
            "graine {seed} : accord={} conv={:?} msg={} emissions_evitees={}",
            r.accord, r.periode_convergence, r.messages, r.emissions_evitees
        );
    }
    println!();
    println!("=== comparaison sans quiescence ===");
    let p2 = Params { partition: true, forcer_divergence: true, ..Default::default() };
    for seed in [1001u64, 1002, 1003] {
        let r = simuler(seed, &p2);
        println!(
            "graine {seed} : accord={} conv={:?} msg={}",
            r.accord, r.periode_convergence, r.messages
        );
    }
}
