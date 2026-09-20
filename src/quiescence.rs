//! **v3** — quiescence : un agent cesse d'émettre quand son état est stable.
//!
//! ## Motivation
//!
//! La v2 (et la v1) émettent à chaque période, indéfiniment, même quand tous
//! les agents partagent déjà la même valeur. Le coût en messages est donc
//! proportionnel au nombre de périodes simulées, pas à l'information
//! réellement transportée.
//!
//! Mesures v2 sur 1000 graines (voir `examples/inventaire_v3.rs`) :
//!
//! | test | messages moyens | convergence médiane |
//! |------|-----------------|---------------------|
//! | T1   | 470             | 5                   |
//! | T4   | 940             | 5                   |
//! | T5   | 3 349           | 5                   |
//! | T5D  | 9 094           | 103                 |
//!
//! T5D est le pire cas : 9 094 messages pour transporter une information qui
//! tient en quelques dizaines de messages utiles.
//!
//! ## Mécanisme
//!
//! Chaque agent tient un compteur `stable[i]` : nombre de périodes
//! consécutives pendant lesquelles son état n'a pas changé. Au-delà d'un
//! seuil `SEUIL_QUIESCENCE`, l'agent cesse d'émettre — il est *quiescent*.
//!
//! Un agent quiescent **reste récepteur** : si un pair lui envoie un état
//! différent, il se réveille (le compteur retombe à zéro). C'est ce qui
//! garantit qu'une information nouvelle circule encore.
//!
//! ## Garantie anti-régression
//!
//! Le mécanisme est **désactivé par défaut** (`quiescence: false`) : le chemin
//! de code v1/v2 est alors strictement inchangé. La parité bit-à-bit est
//! vérifiable par `verify_parite_rust.py`.

/// Nombre de périodes de stabilité avant qu'un agent devienne quiescent.
///
/// Valeur choisie empiriquement : assez grande pour ne pas interrompre une
/// propagation en cours (la convergence médiane est à 5 périodes), assez
/// petite pour couper l'émission dans les régimes longs (T5D converge à 103).
pub const SEUIL_QUIESCENCE: usize = 8;

/// Période du réveil : un agent quiescent émet quand même tous les
/// `PERIODE_REVEIL` tours.
///
/// ## Pourquoi ce réveil est indispensable
///
/// La stabilité est une propriété **locale**. Un agent peut être stable dans
/// sa composante alors que cette composante doit encore fusionner avec une
/// autre (cas de la partition : chaque moitié converge en interne avant la
/// re-fusion). Sans réveil, les deux moitiés se figent sur des valeurs
/// divergentes et ne se reparlent jamais — mesuré : T5D passe de 1000/1000 à
/// **0/1000** d'accord.
///
/// Le réveil périodique garantit qu'une information nouvelle finit toujours
/// par circuler, au prix d'un coût résiduel borné.
pub const PERIODE_REVEIL: usize = 16;

/// Un agent doit-il émettre à cette période ?
///
/// `stable` = nombre de périodes consécutives sans changement d'état.
#[inline]
pub fn doit_emettre(stable: usize) -> bool {
    if stable < SEUIL_QUIESCENCE {
        return true;
    }
    // Quiescent : on n'emet plus qu'au rythme du reveil.
    stable % PERIODE_REVEIL == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emet_tant_que_non_stable() {
        assert!(doit_emettre(0));
        assert!(doit_emettre(SEUIL_QUIESCENCE - 1));
    }

    #[test]
    fn cesse_d_emettre_apres_le_seuil() {
        // Juste apres le seuil, l'agent est quiescent : il n'emet plus a
        // chaque tour (sauf au rythme du reveil).
        assert!(!doit_emettre(SEUIL_QUIESCENCE));
        assert!(!doit_emettre(SEUIL_QUIESCENCE + 1));
    }

    #[test]
    fn reveil_periodique_du_quiescent() {
        // Un agent quiescent DOIT se reveiller periodiquement, sinon une
        // composante isolee ne peut jamais fusionner avec une autre.
        // Le reveil a lieu quand `stable` est un multiple de PERIODE_REVEIL.
        assert!(doit_emettre(PERIODE_REVEIL));
        assert!(doit_emettre(2 * PERIODE_REVEIL));
        assert!(doit_emettre(3 * PERIODE_REVEIL));
        // ... et pas entre deux multiples.
        assert!(!doit_emettre(PERIODE_REVEIL + 1));
        assert!(!doit_emettre(2 * PERIODE_REVEIL + 1));
    }

    #[test]
    fn seuil_est_coherent_avec_la_convergence_mediane() {
        // La convergence médiane observee est de 5 periodes : le seuil doit
        // etre strictement superieur pour ne pas couper une propagation en
        // cours dans le regime nominal.
        assert!(SEUIL_QUIESCENCE > 5);
    }
}
