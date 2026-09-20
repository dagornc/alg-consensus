//! ALG_CONSENSUS v2 — reconnexion des agents isolés.
//!
//! # Le défaut de la v1
//!
//! En v1, chaque agent ne contacte que ses **voisins 4-connexes** sur la
//! grille. Quand un agent perd ses voisins par retrait, il devient
//! définitivement muet : il ne peut plus ni émettre ni recevoir, et son état
//! reste figé à sa valeur initiale. La convergence globale est alors
//! impossible.
//!
//! C'est la cause racine des échecs du test T8 : sur 1 000 graines, 10
//! échouent, et **10/10 sont des agents de coin** dont les deux seuls voisins
//! ont été retirés (coins 0, 5, 24, 29).
//!
//! # Le mécanisme v2
//!
//! Un agent qui ne détecte **aucun pair joignable** pendant `SEUIL_ISOLE`
//! périodes consécutives élargit son rayon de contact : il cesse de se
//! limiter au voisinage 4-connexe et cherche des pairs dans un rayon
//! croissant, jusqu'à `RAYON_MAX`.
//!
//! Le mécanisme est **local** : aucun coordinateur, aucune connaissance
//! globale. Chaque agent ne sait que ce qu'il observe — l'absence de réponse.
//! C'est ce qui le rend compatible avec le modèle de la solution A.
//!
//! # Compatibilité v1
//!
//! Le mécanisme est **désactivé par défaut** (`reconnexion: false`). Une
//! simulation v2 avec `reconnexion: false` produit exactement les mêmes
//! résultats qu'une simulation v1 — la parité est préservée par construction,
//! et vérifiée par les tests.

use crate::sim::{GRID_W, N};

/// Nombre de périodes consécutives sans pair joignable avant élargissement.
pub const SEUIL_ISOLE: usize = 2;

/// Rayon de contact maximal (en cases de grille).
pub const RAYON_MAX: usize = 3;

/// Distance de Chebyshev entre deux indices de grille.
///
/// C'est la distance pertinente ici : elle définit un voisinage carré, qui
/// contient le voisinage 4-connexe de la v1 comme cas particulier (rayon 1).
#[inline]
pub fn distance_chebyshev(a: usize, b: usize) -> usize {
    let (xa, ya) = (a % GRID_W, a / GRID_W);
    let (xb, yb) = (b % GRID_W, b / GRID_W);
    let dx = xa.abs_diff(xb);
    let dy = ya.abs_diff(yb);
    dx.max(dy)
}

/// Candidats de contact pour un agent, à un rayon donné.
///
/// Renvoie les agents actifs situés à une distance de Chebyshev **inférieure
/// ou égale** à `rayon`, en excluant l'agent lui-même. Le résultat est trié
/// par index croissant, ce qui garantit un ordre déterministe — condition
/// nécessaire à la reproductibilité.
///
/// **Rayon 1** : le rayon 1 de Chebyshev inclut les diagonales (8 voisins),
/// ce qui serait une rupture avec la v1 (4-connexe). Pour que l'élargissement
/// soit une **extension** du voisinage v1 et non un remplacement, le rayon 1
/// est défini comme le voisinage 4-connexe exact.
pub fn candidats_rayon(i: usize, rayon: usize, actifs: &[bool]) -> Vec<usize> {
    if rayon <= 1 {
        return crate::sim::voisins(i, GRID_W, crate::sim::GRID_H)
            .into_iter()
            .filter(|&j| actifs[j])
            .collect();
    }
    let mut out = Vec::new();
    for j in 0..N {
        if j == i || !actifs[j] {
            continue;
        }
        if distance_chebyshev(i, j) <= rayon {
            out.push(j);
        }
    }
    out
}

/// Rayon effectif d'un agent compte tenu de son isolement.
///
/// Un agent qui a subi `periodes_isole` périodes consécutives sans pair
/// joignable voit son rayon croître linéairement, plafonné à `RAYON_MAX`.
/// Un agent jamais isolé garde le rayon 1 (voisinage 4-connexe de la v1).
///
/// # Pourquoi le rayon ne redescend pas
///
/// Si le rayon redescendait dès qu'un pair est trouvé, l'agent oscillerait :
/// rayon 1 → isolé → rayon 2 → connecté → rayon 1 → isolé → … et ne
/// resterait jamais connecté assez longtemps pour échanger son état. Le
/// rayon est donc **monotone croissant** : une fois élargi, il le reste.
#[inline]
pub fn rayon_effectif(periodes_isole: usize) -> usize {
    if periodes_isole < SEUIL_ISOLE {
        return 1;
    }
    let croissance = periodes_isole - SEUIL_ISOLE + 2;
    croissance.min(RAYON_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_chebyshev_cases_connues() {
        // (0,0) = index 0 ; (1,0) = index 1 ; (0,1) = index 6.
        assert_eq!(distance_chebyshev(0, 0), 0);
        assert_eq!(distance_chebyshev(0, 1), 1);
        assert_eq!(distance_chebyshev(0, 6), 1);
        assert_eq!(distance_chebyshev(0, 7), 1);
        assert_eq!(distance_chebyshev(0, 2), 2);
        assert_eq!(distance_chebyshev(0, 12), 2);
    }

    #[test]
    fn rayon_effectif_croit_puis_plafonne() {
        assert_eq!(rayon_effectif(0), 1, "agent jamais isole : rayon v1");
        assert_eq!(rayon_effectif(1), 1, "sous le seuil : rayon v1");
        assert_eq!(rayon_effectif(2), 2, "seuil atteint : elargissement");
        assert_eq!(rayon_effectif(3), 3, "croissance");
        assert_eq!(rayon_effectif(4), 3, "plafond RAYON_MAX");
        assert_eq!(rayon_effectif(100), 3, "plafond stable");
    }

    #[test]
    fn candidats_rayon_1_est_le_voisinage_4_connexe() {
        // Pour un agent du centre, le rayon 1 doit redonner exactement le
        // voisinage 4-connexe de la v1 (mêmes éléments, ordre v1 préservé).
        let actifs = vec![true; N];
        let attendus = crate::sim::voisins(7, GRID_W, crate::sim::GRID_H);
        assert_eq!(candidats_rayon(7, 1, &actifs), attendus);
        // Et surtout : pas de diagonales (contrairement au rayon 1 brut de
        // Chebyshev, qui en inclurait 4 de plus).
        assert_eq!(candidats_rayon(7, 1, &actifs).len(), 4);
    }

    #[test]
    fn candidats_rayon_exclut_les_inactifs() {
        let mut actifs = vec![true; N];
        actifs[1] = false;
        actifs[6] = false;
        // Coin (0,0) : ses deux voisins v1 sont retires.
        assert!(candidats_rayon(0, 1, &actifs).is_empty());
        // Au rayon 2, il retrouve des pairs.
        assert!(!candidats_rayon(0, 2, &actifs).is_empty());
    }

    #[test]
    fn candidats_rayon_exclut_l_agent_lui_meme() {
        let actifs = vec![true; N];
        assert!(!candidats_rayon(0, 3, &actifs).contains(&0));
    }
}
