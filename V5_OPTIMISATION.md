# Rust v5 — moteur optimisé, spécification v6

Branche : `v5`, issue de `v4`. Aucun historique de branche n'est réécrit.
Contrat normatif : [SPECIFICATION_V6.md](SPECIFICATION_V6.md).
Audit : [AUDIT_V5.md](AUDIT_V5.md).

## Ce qui change

- File FIFO agrégée par destinataire/date : un maximum au lieu de toutes les copies.
- Aucun stockage des messages qui arriveraient après l'horizon.
- Test d'accord et de divergence sans table de hachage.
- Voisinages pré-calculés, buffer de cibles réutilisé, tableau des nouveaux états.
- Rejet explicite des pertes invalides et des risques de débordement des compteurs.
- API `Result` et sélection CLI `--v5`, optionnelles et sans dépendance ajoutée.

Le moteur de référence `src/sim.rs`, le RNG, les politiques v2/v3 et la boucle
opérationnelle v4 sont conservés. Le nombre de messages et les tours de
convergence ne sont pas réduits : c'est leur **simulation** qui coûte moins cher.

## Mesures locales

20 septembre 2026, Darwin arm64, rustc 1.89.0, profil release. Même binaire,
mêmes 1 000 graines 1000–1999 par scénario, sept répétitions, échauffement de
50 graines par moteur, ordre v4/v5 alterné. Mesure de la simulation avec vues,
hors compilation et I/O, `std::hint::black_box`. Médiane des sept durées.

| Scénario | v4 (ms) | v5 (ms) | Accélération v4/v5 |
|---|---:|---:|---:|
| Nominal | 31,311 | 26,022 | ×1,203 |
| Partition avec divergence | 530,074 | 405,512 | ×1,307 |
| Latence 12, duplication 8, perte 0,25 | 1 792,746 | 685,497 | ×2,615 |
| Partition avec reconnexion et quiescence | 264,642 | 127,877 | ×2,070 |

Ces résultats sont locaux, pas des garanties matérielles ni des intervalles
de confiance. Les bornes mémoire structurelles sont décrites dans la v6 ;
aucun chiffre de réduction RSS n'est revendiqué. Reproduire :

```sh
cargo run --release --example mesure_v5
```

## Utilisation

```sh
git switch v5
cargo build --release
./target/release/consensus_rs --v5 --all --seeds 1001-1100 --out v5.csv
./target/release/consensus_rs --v5 --v3 --test T5D --seeds 1001-1100 --out v5_quiescence.csv
```

API :

```rust
use consensus_rs::{Params, optimise_v5::simuler_avec_vues_v5};
let (resultat, etats, actifs, quiescents) =
    simuler_avec_vues_v5(1001, &Params::default())?;
# Ok::<(), &'static str>(())
```

`--v5` conserve les politiques par défaut ; il n'active pas implicitement
la reconnexion. `--v5 --v4` est une erreur explicite. Les politiques restent
orthogonales au moteur. Le contrat v5 ne couvre pas une reprise complète de
simulation depuis la sérialisation opérationnelle v4.

## Contrôles reproductibles

```sh
cargo test --all-targets
cargo test --release --all-targets
cargo build --release
python3 verify_parite_rust.py --v5 --seeds 1001-1100
cargo clippy --all-targets
git diff --check
```

- 57 tests unitaires historiques et cinq tests RNG conservés.
- Six nouveaux tests : matrice de 5 760 exécutions, 600 partitions longues,
  entrées adverses, lois algébriques, perte totale, témoin de reconnexion idéalisée.
- Parité Python : 900 couples test/graine, zéro écart.
- Clippy : avertissements historiques conservés et documentés, aucun nouvel
  avertissement dans `optimise_v5.rs`. La CI n'utilise pas `-D warnings` sur
  cette base historique et n'impose pas de seuil de benchmark.

La CI exécute tests debug/release et parité Python. Ses résultats doivent être
consultés pour le commit publié ; une mesure locale n'est pas un résultat CI.

## Maintenance

`generate_v5.mjs` est le script d'extraction mécanique initiale depuis le moteur
v4 ; Node est requis seulement pour cette opération, pas pour compiler Rust.
Après régénération, formater `src/optimise_v5.rs` et rejouer tous les tests.
Ne pas modifier le moteur de référence pour faire passer une différence.
Les limites réseau héritées sont explicites en §7 de la spécification v6.
