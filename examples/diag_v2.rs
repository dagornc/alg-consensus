//! Diagnostic : etat final reel de la graine 1010 en v2.

use consensus_rs::sim::{simuler, Params};

fn main() {
    let p = Params {
        retrait: 3,
        t_retrait: Some(3),
        reconnexion: true,
        ..Default::default()
    };
    let r = simuler(1010, &p);
    println!("graine 1010 v2 : accord={} reconn={} taille={} periode_conv={:?}",
        r.accord, r.reconnections, r.taille_etat, r.periode_convergence);
    println!("(l'etat final est affiche sur stderr avec DIAG_V2=1)");
}
