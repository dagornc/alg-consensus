use consensus_rs::{Params, optimise_v5::*, sim::simuler_avec_vues};

#[test]
fn differential_5760_executions_et_vues() {
    for perte in [0.0, 0.25, 0.99, 1.0] {
        for latence in [0, 2, 80] {
            for duplication in [0, 3] {
                for (retrait, t_retrait) in [(0, None), (29, None), (3, Some(3))] {
                    for partition in [false, true] {
                        for reconnexion in [false, true] {
                            for quiescence in [false, true] {
                                let p = Params {
                                    perte,
                                    latence,
                                    duplication,
                                    retrait,
                                    t_retrait,
                                    partition,
                                    reconnexion,
                                    quiescence,
                                    forcer_divergence: true,
                                    max_periodes: 40,
                                };
                                for seed in 0..10 {
                                    assert_eq!(
                                        simuler_avec_vues_v5(seed, &p).unwrap(),
                                        simuler_avec_vues(seed, &p),
                                        "{seed}: {p:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn differential_longues_partitions() {
    for seed in 1000..1200 {
        for latence in [0, 1, 7] {
            let p = Params {
                partition: true,
                forcer_divergence: true,
                latence,
                duplication: 8,
                reconnexion: true,
                quiescence: true,
                retrait: 5,
                t_retrait: Some(70),
                ..Params::default()
            };
            assert_eq!(
                simuler_avec_vues_v5(seed, &p).unwrap(),
                simuler_avec_vues(seed, &p)
            );
        }
    }
}

#[test]
fn parametres_adverses() {
    for perte in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
        assert!(
            simuler_v5(
                0,
                &Params {
                    perte,
                    ..Params::default()
                }
            )
            .is_err()
        );
    }
    assert!(
        simuler_v5(
            0,
            &Params {
                duplication: usize::MAX,
                ..Params::default()
            }
        )
        .is_err()
    );
    assert!(
        simuler_v5(
            0,
            &Params {
                max_periodes: usize::MAX,
                duplication: 0,
                ..Params::default()
            }
        )
        .is_err()
    );
    let p = Params {
        latence: usize::MAX,
        max_periodes: 5,
        ..Params::default()
    };
    assert!(!simuler_v5(1001, &p).unwrap().accord); // no overflow, no delivery
    for seed in [0, 1, u64::MAX] {
        let p = Params {
            max_periodes: 0,
            ..Params::default()
        };
        assert_eq!(
            simuler_avec_vues_v5(seed, &p).unwrap(),
            simuler_avec_vues(seed, &p)
        );
    }
}

#[test]
fn fusion_lois_et_monotonie_sur_domaine_borne() {
    use consensus_rs::sim::fusion;
    let mut domain = vec![None];
    for v in -2..=2 {
        for clock in [0, 1, u32::MAX] {
            domain.push(Some((v, clock)));
        }
    }
    for &a in &domain {
        for &b in &domain {
            assert_eq!(fusion(a, b), fusion(b, a));
            assert_eq!(fusion(a, a), a);
            assert!(fusion(a, b) >= a && fusion(a, b) >= b);
            for &c in &domain {
                assert_eq!(fusion(fusion(a, b), c), fusion(a, fusion(b, c)));
            }
        }
    }
}

#[test]
fn perte_totale_sans_reconnexion_ne_produit_pas_de_convergence_magique() {
    let p = Params {
        perte: 1.0,
        ..Params::default()
    };
    assert!(!simuler_v5(1001, &p).unwrap().accord);
}

#[test]
fn temoin_limite_heritee_reconnexion_hors_canal() {
    // Documente, sans l'approuver, une limite de la référence : à perte=1,
    // la reconnexion idéalisée peut encore changer un état actif.
    let p = Params {
        perte: 1.0,
        retrait: 20,
        reconnexion: true,
        ..Params::default()
    };
    let witness = (0..100).any(|seed| {
        let initial = simuler_avec_vues_v5(
            seed,
            &Params {
                max_periodes: 0,
                ..p
            },
        )
        .unwrap();
        let final_state = simuler_avec_vues_v5(seed, &p).unwrap();
        initial.1 != final_state.1
    });
    assert!(
        witness,
        "Si le protocole change, réviser explicitement le contrat de compatibilité"
    );
}
