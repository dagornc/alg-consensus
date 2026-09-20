# Développement Rust v6

Contrat : [spécification v7](SPECIFICATION_V7.md). Branche `v6`, issue de `v5`.
Les fichiers et commandes historiques restent fonctionnels. Les dépendances
Serde/serde_json ne concernent que la sérialisation du nouveau moteur ; versions
résolues dans Cargo.lock. Rust édition 2024 ; testé localement avec Rust 1.89.0.

## Exécution

```bash
git clone --branch v6 https://github.com/dagornc/alg-consensus.git
cd alg-consensus
cargo run --release --example executer_v6 -- --help
cargo run --release --example executer_v6 -- --default-config
cargo run --release --example executer_v6 -- --seed 42
cargo run --release --example executer_v6 -- --steps 17 --checkpoint etape17.json
cargo run --release --example executer_v6 -- --resume etape17.json
cargo run --release --example executer_v6 -- --config config.json
```

`--steps` compte les périodes supplémentaires, pas une date absolue. Le CLI
initialise les agents par `(identifiant,0)` ; l'API `Moteur::nouveau` accepte
n'importe quel tableau initial de 30 états optionnels. Les sorties JSON donnent
les états, compteurs, accord et statut d'arrêt. Un arrêt normal ne signifie pas
un accord. `--raw` sélectionne l'oracle non agrégé ; la reprise conserve le
mode et la configuration sauvegardés. Une destination de checkpoint doit être
nouvelle. Garder une sauvegarde antérieure en cas d'écriture interrompue.

## API

`Config::valider`, `Moteur::nouveau(config, seed, états, agrégé)`, `pas`,
`observation`, `terminer`, `checkpoint`, `reprendre`, `lots_stockes`.
Les checkpoints ne sont ni chiffrés ni authentifiés ; ne pas les traiter comme
des preuves d'exécution. Ils ne contiennent pas de mots de passe ou de tokens.

## Validation

```bash
cargo test --all-targets
cargo test --release --all-targets
cargo build --release
python3 verify_parite_rust.py --v5 --seeds 1001-1100
cargo run --release --example mesure_v6
```

10 tests v6, dont une matrice de 324 configurations (12 graines × 3 pertes ×
3 délais × 3 duplications), 11 340 comparaisons de périodes, 3 240 reprises
comparées à leur continuation ininterrompue. Chaque reprise compare aussi les
octets du checkpoint final. 68 tests historiques conservés. Les cas directs
couvrent pertes totales, délai et absence de cascade, incarnations obsolètes,
sondes, événement après accord, budgets, saturation, monotonie, compteur
analytique et entrées/checkpoints invalides. Ce n'est pas du fuzzing exhaustif.

## Mesure locale indicative — 20 septembre 2026

macOS arm64, Rust 1.89.0, compilation release, une mesure par variante.
100 graines, 200 tours, perte 20 %, latence 7, duplication 4, états initiaux
identiques. Le chronomètre inclut les observations et le suivi du pic des lots.

| Stockage | Quiescence | Temps total ms | Tentatives | Accords / 100 | Pic de lots |
|---|---|---:|---:|---:|---:|
| Oracle brut | non | 507,997 | 8 381 200 | 100 | 2 468 |
| Agrégé | non | 92,238 | 8 381 200 | 100 | 7 |
| Oracle brut | oui | 140,750 | 2 131 640 | 100 | 2 426 |
| Agrégé | oui | 39,184 | 2 131 640 | 100 | 7 |

Sur ce scénario, la quiescence réduit les tentatives de **74,6 %**, avec accord
final pour les 100 graines. Cela ne mesure pas le délai jusqu'au premier accord.
L'agrégation réduit les lots et le temps face à cet oracle, mais celui-ci alloue
un tableau de 30 cases par copie : **aucune accélération face à un moteur brut
optimal ou à v5 n'est revendiquée**. Les contrats v5/v6 diffèrent. Pas de mesure
RSS, d'intervalle de confiance ou de garantie de performance universelle.

## Limites opérationnelles

Population fixe, transport simulé, latence constante, aucune authentification,
pas de concurrence réseau réelle, pas de détection distribuée de terminaison.
Les sondes supposent les identifiants connus. Sans hypothèses de connectivité
et disponibilité, aucune garantie de convergence finie. Le budget inclut les
tentatives perdues, et l'arrêt laisse explicitement les messages restants en vol.
