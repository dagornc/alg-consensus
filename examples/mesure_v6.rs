use consensus_rs::{
    protocole_v6::{Config, Moteur},
    sim::N,
};
use std::{hint::black_box, time::Instant};
fn main() {
    println!("mode,quiescence,runs,milliseconds,tentatives,accords,pic_lots");
    for q in [false, true] {
        for aggregate in [false, true] {
            let start = Instant::now();
            let mut attempts = 0;
            let mut agreed = 0;
            let mut peak = 0;
            for seed in 0..100 {
                let c = Config {
                    quiescence: q,
                    perte: 0.2,
                    latence: 7,
                    duplication: 4,
                    ..Config::default()
                };
                let mut m = Moteur::nouveau(
                    c,
                    seed,
                    std::array::from_fn::<_, N, _>(|i| Some((i as i32, 0))),
                    aggregate,
                )
                .unwrap();
                while !m.observation().termine {
                    m.pas();
                    peak = peak.max(m.lots_stockes());
                }
                let o = black_box(m.observation());
                attempts += o.compteurs.tentatives;
                agreed += u64::from(o.accord);
            }
            println!(
                "{},{q},100,{:.3},{attempts},{agreed},{peak}",
                if aggregate { "agrege" } else { "oracle_brut" },
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}
