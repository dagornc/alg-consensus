use consensus_rs::{
    protocole_v6::{Config, Evenement, Moteur},
    sim::N,
};

fn initial() -> [Option<(i32, u32)>; N] {
    std::array::from_fn(|i| Some((i as i32, 0)))
}
fn engine(c: Config) -> Moteur {
    Moteur::nouveau(c, 42, initial(), true).unwrap()
}

#[test]
fn pertes_totales_meme_avec_sondes() {
    let mut m = engine(Config {
        perte: 1.0,
        ..Config::default()
    });
    let o = m.terminer();
    assert_eq!(o.etats, initial());
    assert_eq!(o.compteurs.livrees, 0);
    assert_eq!(o.compteurs.tentatives, o.compteurs.perdues);
    assert!(!o.accord);
}
#[test]
fn delai_et_zero_sans_cascade() {
    let mut m = engine(Config {
        latence: 3,
        ..Config::default()
    });
    for _ in 0..3 {
        assert_eq!(m.pas().etats, initial());
    }
    assert_ne!(m.pas().etats, initial());
    let mut states = [Some((0, 0)); N];
    states[0] = Some((99, 0));
    let mut m = Moteur::nouveau(
        Config {
            latence: 0,
            sonde: 0,
            ..Config::default()
        },
        0,
        states,
        true,
    )
    .unwrap();
    let o = m.pas();
    assert_eq!(o.etats[1], Some((99, 0)));
    assert_eq!(o.etats[2], Some((0, 0)));
}
#[test]
fn messages_ancienne_incarnation_rejetes() {
    let mut states = [None; N];
    states[0] = Some((99, 0));
    states[1] = Some((0, 0));
    let c = Config {
        horizon: 5,
        latence: 3,
        sonde: 0,
        evenements: vec![
            Evenement {
                tour: 2,
                agent: 0,
                etat: None,
            },
            Evenement {
                tour: 2,
                agent: 1,
                etat: None,
            },
            Evenement {
                tour: 3,
                agent: 1,
                etat: Some((0, 0)),
            },
        ],
        ..Config::default()
    };
    let o = Moteur::nouveau(c, 0, states, true).unwrap().terminer();
    assert_eq!(o.etats[1], Some((0, 0)));
    assert!(o.compteurs.incarnation > 0);
}
#[test]
fn sondes_reconnectent_sans_lecture_distante() {
    let mut states = [None; N];
    states[0] = Some((99, 0));
    states[N - 1] = Some((0, 0));
    for probe in [0, 1] {
        let c = Config {
            horizon: 90,
            sonde: probe,
            ..Config::default()
        };
        let o = Moteur::nouveau(c, 0, states, true).unwrap().terminer();
        assert_eq!(o.accord, probe == 1);
    }
}
#[test]
fn accord_complet_et_evenement_futur() {
    let mut states = [Some((1, 0)); N];
    states[0] = Some((1, 1));
    assert!(
        !Moteur::nouveau(Config::default(), 0, states, true)
            .unwrap()
            .observation()
            .accord
    );
    let c = Config {
        evenements: vec![Evenement {
            tour: 20,
            agent: 0,
            etat: Some((999, 2)),
        }],
        ..Config::default()
    };
    let o = Moteur::nouveau(c, 0, [Some((1, 0)); N], true)
        .unwrap()
        .terminer();
    assert_eq!(o.tour, 200);
    assert!(o.accord);
    assert_eq!(o.etats, [Some((999, 2)); N]);
}
#[test]
fn bornes_budget_capacite_et_vide() {
    for budget in [0, 1, 37] {
        let o = engine(Config {
            budget_messages: budget,
            ..Config::default()
        })
        .terminer();
        assert_eq!(o.compteurs.tentatives, budget);
        assert!(o.compteurs.conserves());
    }
    let o = engine(Config {
        capacite_messages: 0,
        ..Config::default()
    })
    .terminer();
    assert_eq!(o.compteurs.tentatives, o.compteurs.saturation);
    assert!(
        !Moteur::nouveau(Config::default(), 0, [None; N], true)
            .unwrap()
            .terminer()
            .accord
    );
    assert_eq!(
        engine(Config {
            duplication: 0,
            ..Config::default()
        })
        .terminer()
        .etats,
        initial()
    );
}
#[test]
fn differentiel_et_reprise_tour_par_tour() {
    for seed in 0..12 {
        for loss in [0.0, 0.237896453214789, 1.0] {
            for delay in [0, 1, 7] {
                for dup in [0, 1, 4] {
                    let c = Config {
                        horizon: 35,
                        perte: loss,
                        latence: delay,
                        duplication: dup,
                        fin_partition: 8,
                        capacite_messages: 200,
                        evenements: vec![
                            Evenement {
                                tour: 10,
                                agent: 0,
                                etat: None,
                            },
                            Evenement {
                                tour: 13,
                                agent: 0,
                                etat: Some((40, 2)),
                            },
                        ],
                        ..Config::default()
                    };
                    let mut a = Moteur::nouveau(c.clone(), seed, initial(), true).unwrap();
                    let mut b = Moteur::nouveau(c, seed, initial(), false).unwrap();
                    for t in 0..35 {
                        assert_eq!(a.pas(), b.pas());
                        assert!(a.observation().compteurs.conserves());
                        if t % 7 == 0 {
                            for m in [&a, &b] {
                                let bytes = m.checkpoint().unwrap();
                                let mut restored = Moteur::reprendre(&bytes).unwrap();
                                assert_eq!(bytes, restored.checkpoint().unwrap());
                                let mut expected = m.clone();
                                assert_eq!(expected.terminer(), restored.terminer());
                                assert_eq!(
                                    expected.checkpoint().unwrap(),
                                    restored.checkpoint().unwrap()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn checkpoint_corrompu_rejete() {
    let mut m = engine(Config::default());
    m.pas();
    let bytes = m.checkpoint().unwrap();
    for key in ["format", "tour", "epochs", "file", "compteurs"] {
        let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v[key] = serde_json::json!(999999);
        assert!(Moteur::reprendre(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    assert!(Moteur::reprendre(b"{}").is_err());
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["inconnu"] = true.into();
    assert!(Moteur::reprendre(&serde_json::to_vec(&v).unwrap()).is_err());
}

#[test]
fn monotonie_et_validation_entrees() {
    for seed in 0..20 {
        let mut m = Moteur::nouveau(
            Config {
                perte: 0.3,
                ..Config::default()
            },
            seed,
            initial(),
            true,
        )
        .unwrap();
        let mut previous = m.observation();
        while !previous.termine {
            let next = m.pas();
            for i in 0..N {
                assert!(next.etats[i] >= previous.etats[i]);
            }
            previous = next;
        }
        assert!(previous.accord);
    }
    for c in [
        Config {
            perte: f64::NAN,
            ..Config::default()
        },
        Config {
            reveil: 0,
            ..Config::default()
        },
        Config {
            duplication: 65,
            ..Config::default()
        },
        Config {
            evenements: vec![Evenement {
                tour: 0,
                agent: 0,
                etat: None,
            }],
            ..Config::default()
        },
    ] {
        assert!(Moteur::nouveau(c, 0, initial(), true).is_err());
    }
}

#[test]
fn compteur_analytique_et_checkpoint_structurel() {
    let mut m = engine(Config {
        latence: 0,
        sonde: 0,
        quiescence: false,
        ..Config::default()
    });
    let o = m.pas(); // 2 * (5*5 + 6*4) arêtes orientées de grille
    assert_eq!(o.compteurs.tentatives, 98);
    assert_eq!(o.compteurs.livrees, 98);
    assert_eq!(o.compteurs.en_vol, 0);
    let mut m = engine(Config::default());
    m.pas();
    let original: serde_json::Value = serde_json::from_slice(&m.checkpoint().unwrap()).unwrap();
    for path in ["epoch", "count", "date", "total"] {
        let mut v = original.clone();
        match path {
            "epoch" => v["epochs"][0] = 1.into(),
            "count" => v["file"][0]["cases"][0]["nombre"] = 0.into(),
            "date" => v["file"][0]["arrivee"] = 0.into(),
            _ => v["compteurs"]["en_vol"] = 0.into(),
        };
        assert!(Moteur::reprendre(&serde_json::to_vec(&v).unwrap()).is_err());
    }
}
