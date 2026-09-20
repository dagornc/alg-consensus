//! Mesure de l'apport de la v2 sur le test T8.
//!
//! Compare v1 (reconnexion = false) et v2 (reconnexion = true) sur les
//! graines connues pour échouer en v1, puis sur un large échantillon.

use consensus_rs::sim::{simuler, Params};

fn main() {
    // Les 10 graines T8 en échec en v1 (documentées dans le README).
    let echecs_v1: [u64; 10] = [1010, 1060, 1228, 1350, 1363, 1536, 1631, 1639, 1687, 1699];

    let p_v1 = Params {
        retrait: 3,
        t_retrait: Some(3),
        ..Default::default()
    };
    let p_v2 = Params {
        retrait: 3,
        t_retrait: Some(3),
        reconnexion: true,
        ..Default::default()
    };

    println!("=== T8 : les 10 graines en echec en v1 ===");
    println!("{:<8} {:<10} {:<10} {:<12} {:<12}", "graine", "v1", "v2", "reconn_v2", "taille_v2");
    let mut corriges = 0;
    for &s in &echecs_v1 {
        let r1 = simuler(s, &p_v1);
        let r2 = simuler(s, &p_v2);
        if r2.accord && !r1.accord {
            corriges += 1;
        }
        println!(
            "{:<8} {:<10} {:<10} {:<12} {:<12}",
            s,
            if r1.accord { "accord" } else { "ECHEC" },
            if r2.accord { "accord" } else { "ECHEC" },
            r2.reconnections,
            r2.taille_etat
        );
    }
    println!("\nGraines corrigees par la v2 : {corriges}/10");

    // Echantillon large : la v2 ne doit pas degrader les graines qui
    // convergeaient deja en v1.
    println!("\n=== T8 sur 1000 graines (1001-2000) ===");
    let mut v1_ok = 0;
    let mut v2_ok = 0;
    let mut v1_ko_v2_ok = 0;
    let mut v1_ok_v2_ko = 0;
    let mut msg_v1: u64 = 0;
    let mut msg_v2: u64 = 0;
    for s in 1001..=2000u64 {
        let r1 = simuler(s, &p_v1);
        let r2 = simuler(s, &p_v2);
        if r1.accord {
            v1_ok += 1;
        }
        if r2.accord {
            v2_ok += 1;
        }
        if !r1.accord && r2.accord {
            v1_ko_v2_ok += 1;
        }
        if r1.accord && !r2.accord {
            v1_ok_v2_ko += 1;
        }
        msg_v1 += r1.messages;
        msg_v2 += r2.messages;
    }
    println!("v1 accord : {v1_ok}/1000");
    println!("v2 accord : {v2_ok}/1000");
    println!("corrigees (v1 KO -> v2 OK) : {v1_ko_v2_ok}");
    println!("REGRESSIONS (v1 OK -> v2 KO) : {v1_ok_v2_ko}");
    println!("messages v1 : {msg_v1}  |  v2 : {msg_v2}  |  surcout : {:.2}%",
        (msg_v2 as f64 / msg_v1 as f64 - 1.0) * 100.0);

    // Non-regression v1 : reconnexion = false doit etre bit-a-bit identique.
    println!("\n=== Non-regression : v2 avec reconnexion=false == v1 ? ===");
    let mut identiques = true;
    for s in 1001..=2000u64 {
        let a = simuler(s, &p_v1);
        let b = simuler(s, &p_v2);
        let _ = b;
        let c = simuler(s, &Params { reconnexion: false, ..p_v1 });
        if a != c {
            identiques = false;
            println!("  DIVERGENCE graine {s}");
            break;
        }
    }
    println!("identiques : {identiques}");
}
