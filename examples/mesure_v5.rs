//! Benchmark apparié, sans dépendance, hors compilation. Aucun seuil de CI temporel.
use consensus_rs::{Params, optimise_v5::simuler_avec_vues_v5, sim::simuler_avec_vues};
use std::{hint::black_box, time::Instant};

fn measure(p: &Params, fast: bool, seeds: u64) -> u128 {
    let start = Instant::now();
    for seed in 1000..1000 + seeds {
        if fast {
            black_box(simuler_avec_vues_v5(black_box(seed), black_box(p)).unwrap());
        } else {
            black_box(simuler_avec_vues(black_box(seed), black_box(p)));
        }
    }
    start.elapsed().as_nanos()
}
fn main() {
    let cases = [
        ("nominal", Params::default()),
        (
            "partition",
            Params {
                partition: true,
                forcer_divergence: true,
                ..Params::default()
            },
        ),
        (
            "latence_duplication",
            Params {
                latence: 12,
                duplication: 8,
                perte: 0.25,
                ..Params::default()
            },
        ),
        (
            "v3_partition",
            Params {
                partition: true,
                forcer_divergence: true,
                reconnexion: true,
                quiescence: true,
                ..Params::default()
            },
        ),
    ];
    println!("cas;v4_median_ms;v5_median_ms;acceleration;graines;repetitions");
    for (name, p) in cases {
        measure(&p, false, 50);
        measure(&p, true, 50);
        let (mut old, mut new) = (Vec::new(), Vec::new());
        for round in 0..7 {
            if round % 2 == 0 {
                old.push(measure(&p, false, 1000));
                new.push(measure(&p, true, 1000));
            } else {
                new.push(measure(&p, true, 1000));
                old.push(measure(&p, false, 1000));
            }
        }
        old.sort_unstable();
        new.sort_unstable();
        println!(
            "{name};{:.3};{:.3};{:.3};1000;7",
            old[3] as f64 / 1e6,
            new[3] as f64 / 1e6,
            old[3] as f64 / new[3] as f64
        );
    }
}
