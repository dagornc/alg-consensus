# consensus_rs — ALG_CONSENSUS en Rust

> **Implémentation Rust de référence de l'algorithme ALG_CONSENSUS**
> (CRDT semi-treillis + gossip), portage à **parité bit-à-bit** du simulateur
> Python de la spécification v5 (§4.7.1).
>
> Ce document est le **standard de documentation** des algorithmes publiés sur
> ce compte : il couvre le fonctionnement, les prérequis, l'installation, la
> configuration, les paramètres d'entrée/sortie, la vérification et les
> limites connues.

---

## Table des matières

1. [En bref](#1-en-bref)
2. [Le problème résolu](#2-le-problème-résolu)
3. [Fonctionnement de l'algorithme](#3-fonctionnement-de-lalgorithme)
4. [Prérequis](#4-prérequis)
5. [Installation](#5-installation)
6. [Configuration](#6-configuration)
7. [Paramètres d'entrée](#7-paramètres-dentrée)
8. [Paramètres de sortie](#8-paramètres-de-sortie)
9. [Utilisation — CLI](#9-utilisation--cli)
10. [Utilisation — bibliothèque Rust](#10-utilisation--bibliothèque-rust)
11. [Le harnais de tests](#11-le-harnais-de-tests)
12. [Vérification et parité](#12-vérification-et-parité)
13. [Performances mesurées](#13-performances-mesurées)
14. [Résultats de référence](#14-résultats-de-référence)
15. [Limites connues et comportements attendus](#15-limites-connues-et-comportements-attendus)
16. [Structure du dépôt](#16-structure-du-dépôt)
17. [Dépannage](#17-dépannage)
18. [Glossaire](#18-glossaire)
19. [Licence](#19-licence)
20. [Version 2 — reconnexion des agents isolés](#20-version-2--reconnexion-des-agents-isolés)
21. [Version 3 — quiescence (loop engineering)](#21-version-3--quiescence-loop-engineering)

---

## 1. En bref

| | |
|---|---|
| **Nom** | `consensus_rs` |
| **Objet** | Simulateur de consensus distribué (CRDT semi-treillis + gossip) |
| **Langage** | Rust, édition 2024 |
| **Dépendances** | **aucune** (bibliothèque standard uniquement) |
| **Lignes de code** | ~900 (dont ~250 de tests) |
| **Parité** | bit-à-bit avec la référence Python — **9 000 comparaisons, 0 écart** |
| **Tests** | **18/18** (13 unitaires + 5 d'intégration) |
| **Warnings** | **0** |
| **Licence** | MIT |

**En une phrase** : trente agents communicant par gossip sur une grille 6×5
convergent vers une valeur commune *même si* des messages sont perdus,
dupliqués, retardés, ou si des agents disparaissent — parce que la fusion
d'état est un semi-treillis.

---

## 2. Le problème résolu

Dans un essaim de drones, chaque agent détient une information partielle
(position d'une cible, décision de mission, identifiant d'un leader). Il faut
que tous finissent par partager la **même** valeur, sans coordinateur central,
sur un réseau qui :

- **perd** des messages (liaison radio instable) ;
- **duplique** des messages (retransmissions) ;
- **retarde** des messages (latence variable) ;
- **réordonne** les messages (chemins multiples) ;
- **perd des agents** (batterie, crash, sortie de zone).

Un protocole naïf (« le dernier message reçu gagne ») échoue sur tous ces
points : il dépend de l'ordre d'arrivée. La solution retenue est un **CRDT**
(Conflict-free Replicated Data Type) dont l'opération de fusion est un
**semi-treillis** — une structure algébrique qui garantit la convergence
*indépendamment de l'ordre*.

---

## 3. Fonctionnement de l'algorithme

### 3.1 Vue d'ensemble

```
   ┌──────────────────────────────────────────────────────────┐
   │  Boucle de gossip — répétée à chaque période t = 1, 2, … │
   │                                                          │
   │   1. LIVRAISON   les messages en vol arrivés à t sont    │
   │                  fusionnés dans l'état local             │
   │                                                          │
   │   2. ÉMISSION    chaque agent actif choisit un           │
   │                  sous-ensemble de ses voisins et leur    │
   │                  envoie son état                         │
   │                                                          │
   │   3. FUSION      le destinataire fusionne l'état reçu    │
   │                  avec le sien                            │
   │                                                          │
   │   4. CONTRÔLE    si tous les agents actifs partagent la  │
   │                  même valeur → convergence, on s'arrête  │
   └──────────────────────────────────────────────────────────┘
```

### 3.2 L'état d'un agent

Chaque agent détient un couple :

```
état = (valeur, horloge_logique)
```

- **`valeur`** — l'information à propager (ici un entier de 0 à 9).
- **`horloge_logique`** — un compteur qui départage deux valeurs égales.

L'ordre est **lexicographique** : on compare d'abord `valeur`, puis
`horloge_logique` en cas d'égalité.

### 3.3 La fusion — cœur de l'algorithme

```rust
fusion(a, b) = max(a, b)   // ordre lexicographique sur (valeur, horloge)
```

Cette opération possède trois propriétés qui font **toute** la robustesse du
protocole :

- **Commutativité** — `a ⊔ b = b ⊔ a`
  → l'ordre d'arrivée des messages n'importe pas.
- **Associativité** — `(a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)`
  → le regroupement des messages n'importe pas.
- **Idempotence** — `a ⊔ a = a`
  → recevoir deux fois le même message est sans effet.

**Conséquence directe** : perte, duplication et réordonnancement ne peuvent
pas empêcher la convergence. Ils ne changent que le **nombre de messages**
nécessaires pour y parvenir.

> **Pourquoi `max` et pas une moyenne ?** Une moyenne n'est ni idempotente ni
> associative sur des flux asynchrones : deux agents pourraient osciller
> indéfiniment. `max` converge vers une valeur **déjà présente** dans le
> système — c'est un choix conservateur : on ne fabrique jamais d'information.

### 3.4 La topologie

- **Grille 2D 6×5** → `N = 30` agents.
- **Voisinage 4-connexe** : chaque agent voit ses voisins haut, bas, gauche,
  droite (2 voisins dans un coin, 3 sur un bord, 4 au centre).
- **Fanout `f = 6`** : à chaque période, un agent contacte jusqu'à 6 voisins.
  Comme le degré maximal est 4, **le fanout couvre en pratique tout le
  voisinage** — le tirage aléatoire ne sert qu'à l'ordre de contact.

```
   index = y * 6 + x          grille 6 × 5

     x→   0    1    2    3    4    5
   y=0    0    1    2    3    4    5
   y=1    6    7    8    9   10   11
   y=2   12   13   14   15   16   17
   y=3   18   19   20   21   22   23
   y=4   24   25   26   27   28   29
```

### 3.5 Le cycle d'une période

Pour chaque agent actif `i`, dans l'ordre croissant des indices :

1. **Cibles** — on calcule `voisins(i)`. Si une partition est active et que
   `t ≤ fin_partition`, on retire les voisins de l'autre moitié.
2. **Tirage** — `dests = sample(cibles, min(FANOUT, len(cibles)))`.
3. **Émission** — pour chaque destinataire `d`, et pour chaque copie
   (`duplication`) :
   - on incrémente le compteur de messages ;
   - on tire un nombre aléatoire : si `< perte`, le message est **perdu** ;
   - sinon, si `latence > 0`, le message est **mis en file** pour arriver à
     `t + latence` ; sinon il est fusionné **immédiatement** dans un tampon
     `nouveaux[d]`.
4. **Application** — après la boucle sur tous les agents, les tampons
   `nouveaux` sont fusionnés dans les états. Ce décalage garantit que tous les
   agents d'une période émettent à partir de l'état **du début de période**
   (modèle à mémoire partagée par période, pas de propagation intra-période).

### 3.6 Convergence et arrêt

À la fin de chaque période, on calcule l'ensemble des valeurs des agents
**actifs**. Si cet ensemble est de cardinal 1, la convergence est atteinte :

- on enregistre `periode_convergence = t` ;
- si une partition était active et qu'une divergence réelle a été observée,
  on enregistre `periode_refusion = t - fin_partition` ;
- on **sort de la boucle** (`break`).

> **Point important** : la simulation s'arrête **dès** la convergence. Un
> retrait programmé à `t = 3` n'a donc **pas lieu** si la convergence survient
> à `t = 2`. C'est un comportement volontaire, identique dans les deux
> implémentations, et c'est ce qui explique les cas `taille_etat = 30` du test
> T8 (voir §15).

### 3.7 La partition

Une partition coupe la grille en **deux moitiés verticales** :

```
   cote(i) = (i % 6) < 3        colonnes 0,1,2 → gauche
                                colonnes 3,4,5 → droite
```

Pendant `t ≤ fin_partition` (avec `fin_partition = max_periodes / 2 = 100`),
les agents ne peuvent contacter que les voisins de leur propre moitié. Les
deux moitiés évoluent alors **indépendamment** et peuvent converger vers des
valeurs différentes. À `t > fin_partition`, la coupure est levée et les
messages traversent à nouveau : c'est la **re-fusion**.

### 3.8 La divergence forcée (T5D)

En partition simple, les deux moitiés convergent souvent vers la **même**
valeur par hasard (le maximum global est fréquemment présent des deux côtés).
Pour **garantir** une divergence réelle et donc **exercer** la re-fusion, le
mode `forcer_divergence` plafonne les agents de droite sous le maximum de
gauche :

```rust
max_gauche = max(valeur des agents de gauche)
pour tout agent de droite : valeur = min(valeur, max_gauche - 1)
```

La moitié droite ne peut alors **pas** atteindre la valeur maximale tant que
la partition est active : la divergence est certaine, et la re-fusion est
réellement mesurée.

### 3.9 Le retrait d'agents

Deux modes :

- **Retrait à l'initialisation** (`t_retrait = None`) — les agents sont
  retirés avant la première période.
- **Retrait programmé** (`t_retrait = Some(t)`) — les agents sont retirés au
  début de la période `t`, **avant** la livraison des messages.

Un agent retiré n'émet plus et n'est plus compté dans le contrôle de
convergence. Ses voisins peuvent se retrouver **isolés** s'il était leur seul
lien — c'est la cause des rares échecs de T8 (§15).

### 3.10 Le générateur pseudo-aléatoire

C'est le point **le plus délicat** du portage. Pour obtenir des résultats
identiques, il ne suffit pas d'utiliser « un bon RNG » : il faut reproduire
**exactement** la séquence de CPython. `src/rng.rs` embarque donc :

- **MT19937** (Mersenne Twister, période 2^19937 − 1) avec l'initialisation
  `init_by_array` utilisée par `random.Random(seed)` ;
- **`random()`** — combinaison de **deux** tirages 32 bits pour obtenir 53
  bits de mantisse :
  `(a * 67108864.0 + b) / 9007199254740992.0` ;
- **`randint(a, b)`** → `randrange` → `_randbelow(n)` par **rejet** avec
  `getrandbits(k)`, où `k = n.bit_length()` ;
- **`sample(population, k)`** — algorithme exact de `Lib/random.py`, y compris
  le calcul de `setsize = 21 + 4^ceil(log(k·3, 4))` et surtout **l'ordre de
  sélection** (CPython ne trie pas le résultat).

Toute divergence sur l'un de ces quatre points casse la parité. C'est
pourquoi `tests/parite.rs` les teste séparément contre des valeurs de
référence produites par CPython.

---

## 4. Prérequis

### 4.1 Pour compiler et exécuter le simulateur Rust

| Composant | Version minimale | Version testée | Notes |
|---|---|---|---|
| **Rust** (`rustc`) | 1.85 | **1.98.1** (2026-09-01) | édition 2024 requise |
| **Cargo** | 1.85 | **1.98.1** (2026-08-05) | fourni avec Rust |
| **Système** | Linux, macOS, Windows | Linux 6.8.0 | aucune dépendance native |

**Aucune dépendance externe.** Le crate n'utilise que la bibliothèque standard
(`std`). Il n'y a donc pas de `cargo fetch` à faire, pas de bibliothèque
C à installer, pas de `build.rs`.

L'édition 2024 impose `rustc >= 1.85`. Pour installer ou mettre à jour Rust :

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable
rustc --version    # doit afficher >= 1.85
```

### 4.2 Pour vérifier la parité (optionnel)

| Composant | Version minimale | Version testée |
|---|---|---|
| **Python** | 3.8 | **3.11.16** |

Python n'est **pas** nécessaire pour compiler ni pour exécuter le simulateur.
Il n'est requis que pour lancer `verify_parite_rust.py`, qui compare les
sorties Rust aux sorties de la référence Python embarquée.

La référence Python (`reference/sim_consensus.py`) n'utilise que la
bibliothèque standard (`argparse`, `csv`, `random`, `statistics`, `sys`) :
aucun `pip install` n'est nécessaire.

> **Note de reproductibilité** : la parité bit-à-bit dépend de la version de
> CPython, car `random.Random` a une séquence stable mais l'algorithme de
> `sample` a évolué historiquement. Les valeurs de référence du dépôt ont été
> produites avec **Python 3.11.16**. Sur une version très différente, relancer
> `tests/REFERENCE_PYTHON.txt` avant de conclure à une régression.

### 4.3 Ressources

- **Disque** : ~50 Mo (dont ~40 Mo de `target/` après compilation).
- **Mémoire** : < 10 Mo à l'exécution (30 agents, état en `Vec`).
- **CPU** : mono-thread. Aucun parallélisme, aucune dépendance à `rayon`.

---

## 5. Installation

### 5.1 Depuis les sources (recommandé)

```bash
git clone https://github.com/dagornc/SwarmDrones.git
cd SwarmDrones/consensus_rs

# Compilation optimisée (obligatoire pour les performances annoncées)
cargo build --release

# Vérification que tout fonctionne
cargo test --release
```

Le binaire est produit dans `target/release/consensus_rs`.

> **Pourquoi `--release` ?** En mode debug, le simulateur est **10 à 30 fois
> plus lent** et les performances annoncées en §13 ne s'appliquent pas. Toutes
> les mesures de ce document sont faites en `--release`.

### 5.2 Vérification de l'installation

```bash
./target/release/consensus_rs --help
# Usage: consensus_rs [--test T1] [--seeds 1001-1100] [--out f.csv] [--all]

./target/release/consensus_rs --test T1 --seeds 1001-1010 --out /tmp/essai.csv
# T1: n=10 accord=10/10 mediane_conv=5 messages_moy=460.6
# -> /tmp/essai.csv
```

Si ces deux commandes produisent ces sorties, l'installation est correcte.

### 5.3 Utilisation comme bibliothèque

Ajouter au `Cargo.toml` du projet consommateur :

```toml
[dependencies]
consensus_rs = { git = "https://github.com/dagornc/SwarmDrones", branch = "master" }
```

Ou, en local, avec un chemin relatif :

```toml
[dependencies]
consensus_rs = { path = "../SwarmDrones/consensus_rs" }
```

---

## 6. Configuration

### 6.1 Principe

Le simulateur est **configuré par le code**, pas par un fichier externe. Il n'y
a ni `.env`, ni `.ini`, ni `.yaml` : les paramètres sont des constantes
publiques (`src/sim.rs`) et des champs de la structure `Params`.

Ce choix est délibéré : il rend la configuration **vérifiable par le
compilateur** et **traçable dans Git**. Un fichier de configuration externe
aurait introduit un risque de divergence silencieuse entre ce qui est
documenté et ce qui est exécuté.

### 6.2 Constantes de topologie (`src/sim.rs`)

| Constante | Valeur | Rôle | Modifiable ? |
|---|---|---|---|
| `T_G` | `1.0` | Période de gossip (s) | non (référence) |
| `GRID_W` | `6` | Largeur de la grille | non (référence) |
| `GRID_H` | `5` | Hauteur de la grille | non (référence) |
| `N` | `30` | Nombre d'agents (`GRID_W × GRID_H`) | dérivé |
| `FANOUT` | `6` | Voisins contactés par période | non (référence) |
| `LATENCE_MAX` | `2.0 * T_G` | Latence maximale (s) | non (référence) |

> Ces constantes sont **figées** parce qu'elles définissent le harnais de la
> spécification v5 §4.7.1. Les modifier romprait la parité avec la référence
> Python et invaliderait les résultats publiés. Pour explorer d'autres
> topologies, il faut modifier **les deux** implémentations et régénérer les
> valeurs de référence.

### 6.3 Paramètres d'exécution (`Params`)

Voir §7 pour le détail de chaque champ. Les valeurs par défaut sont :

```rust
Params {
    perte: 0.0,              // aucune perte
    duplication: 1,          // une copie par message
    retrait: 0,              // aucun agent retiré
    partition: false,        // pas de partition
    max_periodes: 200,       // 200 périodes maximum
    latence: 0,              // arrivée dans la même période
    t_retrait: None,         // retrait à l'initialisation si retrait > 0
    forcer_divergence: false // pas de divergence forcée
}
```

### 6.4 Configuration des tests

Les paramètres des 9 tests du harnais sont définis **en dur** dans
`src/main.rs` (fonction `params_pour`) et dans `reference/sim_consensus.py`
(dictionnaire `params`). Les deux tables doivent rester **identiques** — c'est
ce que vérifie la parité.

---

## 7. Paramètres d'entrée

### 7.1 Entrées de la ligne de commande

| Option | Type | Défaut | Description |
|---|---|---|---|
| `--test <ID>` | chaîne | `T1` | Identifiant du test à exécuter (`T1`…`T9`) |
| `--seeds <A-B>` | `A-B` | `1001-1100` | Plage de graines, **bornes incluses** |
| `--out <fichier>` | chemin | `resultats.csv` | Fichier CSV de sortie |
| `--all` | drapeau | absent | Exécute **les 9 tests** au lieu d'un seul |
| `--help`, `-h` | drapeau | — | Affiche l'usage et quitte |

**Règles de validation** :

- `--seeds` doit être au format `A-B` (exactement un tiret). Sinon :
  `--seeds attendu au format A-B (ex. 1001-1100)` et code de sortie `2`.
- `A` et `B` doivent être des entiers non signés valides. Sinon :
  `graine de début invalide` / `graine de fin invalide`.
- Un argument inconnu provoque `argument inconnu : <arg>` et un code de
  sortie `2`.
- `--all` **prime** sur `--test` : si les deux sont fournis, les 9 tests sont
  exécutés.

### 7.2 Paramètres de simulation (`Params`)

| Champ | Type | Défaut | Plage utile | Description |
|---|---|---|---|---|
| `perte` | `f64` | `0.0` | `[0.0, 1.0]` | Probabilité de perte **par message** (i.i.d.) |
| `duplication` | `usize` | `1` | `≥ 1` | Nombre de copies émises par message |
| `retrait` | `usize` | `0` | `[0, N-1]` | Nombre d'agents retirés |
| `partition` | `bool` | `false` | — | Active la coupure gauche/droite |
| `max_periodes` | `usize` | `200` | `≥ 1` | Nombre maximal de périodes simulées |
| `latence` | `usize` | `0` | `≥ 0` | Retard en périodes (`0` = même période) |
| `t_retrait` | `Option<usize>` | `None` | `≥ 1` | Période du retrait (`None` = initialisation) |
| `forcer_divergence` | `bool` | `false` | — | Plafonne la moitié droite sous le max gauche |

**Sémantique détaillée** :

- **`perte`** — tirée **par message**, pas par période ni par agent. À
  `perte = 0.25`, un message sur quatre est perdu en moyenne. Le compteur
  `messages` **inclut** les messages perdus (ils ont été émis).
- **`duplication`** — chaque copie est soumise **indépendamment** au tirage de
  perte. À `duplication = 2` et `perte = 0.25`, la probabilité qu'un message
  soit perdu **deux fois** est `0.0625`.
- **`retrait`** — borné à `N - 1` : on ne peut pas retirer tous les agents
  (il faut au moins un agent actif pour définir la convergence).
- **`max_periodes`** — sert aussi à calculer `fin_partition = max_periodes / 2`.
  Changer `max_periodes` change donc **aussi** la durée de la partition.
- **`latence`** — les messages sont mis en file par agent et livrés à
  `t + latence`. Une latence non nulle **désynchronise** les agents et
  augmente le nombre de messages nécessaires.
- **`forcer_divergence`** — sans effet si `partition = false`.

### 7.3 Entrées du programme Python de référence

`reference/sim_consensus.py` accepte les mêmes options de ligne de commande
(`--test`, `--seeds`, `--out`, `--all`) et produit le **même format CSV**. Sa
fonction `simuler()` accepte les mêmes paramètres nommés que `Params`.

---

## 8. Paramètres de sortie

### 8.1 Sortie standard (résumé par test)

Une ligne par test exécuté :

```
T1: n=100 accord=100/100 mediane_conv=5 messages_moy=477.3
T5D: n=100 accord=100/100 mediane_conv=103 messages_moy=9094.0 refusion_med=3 refusion_max=3 graines_divergentes=100/100
```

| Champ | Signification |
|---|---|
| `n` | Nombre de graines simulées |
| `accord` | `graines convergées / total` |
| `mediane_conv` | Médiane de `periode_convergence` (sur les graines convergées) |
| `messages_moy` | Moyenne de `messages` sur toutes les graines |
| `refusion_med` | *(si partition)* Médiane de `periode_refusion` |
| `refusion_max` | *(si partition)* Maximum de `periode_refusion` |
| `graines_divergentes` | *(si partition)* Graines ayant réellement divergé |

La dernière ligne affiche `-> <fichier>` avec le chemin du CSV écrit.

### 8.2 Fichier CSV

**En-tête** (8 colonnes, ordre fixe) :

```
test,seed,periode_convergence,accord,messages,taille_etat,periode_refusion,divergence_reelle
```

| Colonne | Type | Vide si | Description |
|---|---|---|---|
| `test` | chaîne | — | Identifiant du test (`T1`…`T9`) |
| `seed` | entier | — | Graine du générateur |
| `periode_convergence` | entier | non convergé | Période où l'accord est atteint |
| `accord` | `0`/`1` | — | `1` si tous les agents actifs partagent la même valeur |
| `messages` | entier | — | Messages **émis** (perdus inclus) |
| `taille_etat` | entier | — | Agents **actifs** en fin de simulation |
| `periode_refusion` | entier | pas de re-fusion mesurée | `periode_convergence - fin_partition` |
| `divergence_reelle` | `0`/`1` | — | `1` si les deux moitiés ont divergé à `fin_partition` |

**Exemple** (extrait réel, `--test T1 --seeds 1001-1005`) :

```csv
test,seed,periode_convergence,accord,messages,taille_etat,periode_refusion,divergence_reelle
T1,1001,2,1,196,30,,0
T1,1002,3,1,294,30,,0
T1,1003,7,1,686,30,,0
T1,1004,6,1,588,30,,0
T1,1005,6,1,588,30,,0
```

> **Format identique à la référence Python.** Un même CSV peut donc être
> produit par l'une ou l'autre implémentation et comparé ligne à ligne — c'est
> exactement ce que fait `verify_parite_rust.py`.

### 8.3 Codes de sortie

| Code | Signification |
|---|---|
| `0` | Succès |
| `2` | Argument invalide (`--seeds` mal formé, argument inconnu) |
| `1` | Échec d'écriture du CSV (`création du CSV impossible`) |
| `101` | Panique Rust (graine non numérique, `sample` avec `k > n`) |

### 8.4 API bibliothèque

```rust
pub fn simuler(seed: u64, p: &Params) -> Resultat
```

```rust
pub struct Resultat {
    pub periode_convergence: Option<usize>,
    pub accord: bool,
    pub messages: u64,
    pub taille_etat: usize,
    pub periode_refusion: Option<usize>,
    pub divergence_reelle: bool,
}
```

Également publics dans le module `sim` : `fusion`, `voisins(i, w, h)`, `Etat`
(alias `(i32, u32)`), et les constantes `T_G`, `GRID_W`, `GRID_H`, `N`,
`FANOUT`, `LATENCE_MAX`.

> **Note sur les ré-exports** : `lib.rs` ne ré-exporte à la racine que
> `PyRandom`, `simuler`, `Params`, `Resultat`, `GRID_H`, `GRID_W` et `N`.
> Pour `fusion`, `voisins` et `Etat`, il faut passer par le module :
> `use consensus_rs::sim::{fusion, voisins, Etat};`

---

## 9. Utilisation — CLI

### 9.1 Exemples de base

```bash
# Compilation
cargo build --release

# Un test, 100 graines
./target/release/consensus_rs --test T1 --seeds 1001-1100 --out resultats.csv

# Un test, 1 000 graines
./target/release/consensus_rs --test T5D --seeds 1001-2000 --out refusion.csv

# Les 9 tests, 1 000 graines (9 000 simulations)
./target/release/consensus_rs --all --seeds 1001-2000 --out resultats.csv

# Une seule graine
./target/release/consensus_rs --test T8 --seeds 1010-1010 --out cas_limite.csv
```

### 9.2 Reproduction des résultats publiés

```bash
# Parité complète : 9 000 comparaisons Rust vs Python
python3 verify_parite_rust.py --seeds 1001-2000
# Graines comparées : 9000
# Écarts             : 0
# VERDICT : PARITÉ BIT-À-BIT CONFORME
```

### 9.3 Enchaînement type

```bash
cargo build --release
cargo test --release
./target/release/consensus_rs --all --seeds 1001-2000 --out resultats.csv
python3 verify_parite_rust.py --seeds 1001-2000
```

---

## 10. Utilisation — bibliothèque Rust

### 10.1 Simulation simple

```rust
use consensus_rs::{simuler, Params};

fn main() {
    // T1 : convergence sans perte
    let r = simuler(1001, &Params::default());
    println!("convergé en {} périodes, {} messages",
             r.periode_convergence.unwrap(), r.messages);
    assert!(r.accord);
}
```

### 10.2 Simulation avec perte et partition

```rust
use consensus_rs::{simuler, Params};

let p = Params {
    perte: 0.25,
    partition: true,
    forcer_divergence: true,
    ..Default::default()
};
let r = simuler(1001, &p);
println!("accord={} refusion={:?}", r.accord, r.periode_refusion);
```

### 10.3 Utilisation directe de la fusion

```rust
use consensus_rs::sim::fusion;

let a = Some((3, 1));
let b = Some((5, 0));
assert_eq!(fusion(a, b), Some((5, 0)));  // max lexicographique
assert_eq!(fusion(a, a), a);             // idempotence
```

### 10.4 Utilisation du RNG compatible CPython

```rust
use consensus_rs::PyRandom;

let mut r = PyRandom::new(1001);
let x = r.random();          // identique à random.Random(1001).random()
let n = r.randint(0, 9);     // identique à randint(0, 9)
let s = r.sample(&(0..30).collect::<Vec<_>>(), 6);  // identique à sample(range(30), 6)
```

---

## 11. Le harnais de tests

### 11.1 Les 9 identifiants

| ID | Paramètres | Ce qui est mesuré |
|---|---|---|
| **T1** | `perte=0.0` | Convergence nominale, sans perturbation |
| **T2** | `perte=0.0` | *Identique à T1* (traçabilité §4.7.1) |
| **T3** | `perte=0.25` | Robustesse à la perte de messages |
| **T4** | `duplication=2` | Effet de la duplication |
| **T5** | `partition=true` | Partition simple + re-fusion |
| **T5D** | `partition=true, forcer_divergence=true` | Re-fusion **garantie** |
| **T6** | `perte=0.0` | *Identique à T1* (traçabilité §4.7.1) |
| **T8** | `retrait=3, t_retrait=3` | Robustesse au retrait d'agents |
| **T9** | `partition=true` | *Identique à T5* (traçabilité §4.7.1) |

> **T2, T6 et T9 sont des doublons fonctionnels** de T1 et T5. Ils sont
> conservés parce qu'ils figurent au tableau §4.7.1 de la spécification, et
> parce que le harnais de parité les compare tous les neuf — ce qui porte la
> couverture à **9 000 comparaisons** au lieu de 6 000. Ils produisent des
> résultats **bit-identiques** à T1/T5 pour une graine donnée (vérifié).

### 11.2 Couverture des tests Rust

**13 tests unitaires** — **4** dans `src/rng.rs`, **9** dans `src/sim.rs` :

- **RNG** (`rng.rs`) : `mt19937_graine_5489_premier_tirage` (valeur de
  référence MT19937), `random_dans_intervalle`, `randint_borne`,
  `sample_sans_remise`.
- **Topologie** (`sim.rs`) : `voisins_coin` (2 voisins), `voisins_centre`
  (4 voisins).
- **Algèbre** (`sim.rs`) : `fusion_est_commutative`, `fusion_est_idempotente`,
  `fusion_est_associative`, `fusion_none_est_neutre`.
- **Simulation** (`sim.rs`) : `t1_converge_sans_perte`,
  `t8_retrait_avant_convergence`, `retrait_a_l_initialisation`.

**5 tests d'intégration** (`tests/parite.rs`) :

- `parite_random_cpython` — `random()` vs CPython (tolérance `1e-15`)
- `parite_randint_cpython` — `randint(0,9)` vs CPython
- `parite_sample_cpython` — `sample(range(30), 6)`, ordre exact
- `parite_getrandbits_cpython` — `getrandbits(4)` vs CPython
- `parite_multi_graines` — déterminisme sur 1, 42, 1001, 2026, 999983

```bash
cargo test --release
# 13 passed  (unitaires)
#  5 passed  (intégration)
```

---

## 12. Vérification et parité

### 12.1 Ce que garantit la parité

`verify_parite_rust.py` compare, **pour chaque test et chaque graine**, le
sextuplet complet :

```
(période_convergence, accord, messages, taille_etat, période_refusion, divergence_reelle)
```

Une égalité sur ce sextuplet signifie que les deux implémentations ont suivi
**exactement** la même trajectoire : mêmes tirages aléatoires, mêmes décisions
de perte, mêmes ordres de contact, mêmes fusions. Ce n'est pas une
approximation statistique — c'est une **identité bit-à-bit**.

### 12.2 Résultat

```
Graines comparées : 9000
Écarts             : 0
VERDICT : PARITÉ BIT-À-BIT CONFORME
```

9 tests × 1 000 graines = **9 000 comparaisons**, **0 écart**.

### 12.3 Reproductibilité

La référence Python est **embarquée** dans le dépôt
(`reference/sim_consensus.py`, 204 lignes, bibliothèque standard uniquement).
`verify_parite_rust.py` l'importe par un chemin **relatif** :

```python
RACINE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(RACINE, "reference"))
```

Le dépôt est donc **autonome** : aucune dépendance à un chemin absolu, à un
fichier externe ou à un service. Cloner, compiler, vérifier — rien d'autre.

### 12.4 En cas d'écart

Si `verify_parite_rust.py` signale un écart, la cause est **presque toujours**
le RNG. Ordre de diagnostic :

1. Vérifier la version de CPython (`python3 --version`). Les valeurs de
   référence ont été produites avec **3.11.16**.
2. Comparer `tests/REFERENCE_PYTHON.txt` aux sorties réelles de CPython sur
   cette machine.
3. Si les valeurs de référence diffèrent, régénérer le fichier — ce n'est pas
   une régression du code Rust, mais un changement de CPython.
4. Si les valeurs de référence **concordent** mais que la parité échoue, alors
   le défaut est dans `src/rng.rs` : vérifier `init_by_array`, `random()`,
   `_randbelow` et `sample` dans cet ordre.

---

## 13. Performances mesurées

### 13.1 Protocole

- Machine : **AMD EPYC 9354P**, 2 vCPU alloués, 7,8 Go de RAM, Linux 6.8.0.
- Rust : `cargo build --release`, `rustc 1.98.1`.
- Python : **3.11.16**, `python3 reference/sim_consensus.py`.
- Charge : `--all` (9 tests). Rust sur **9 000** simulations (1 000 graines),
  Python sur **900** simulations (100 graines), puis extrapolation linéaire.
- Mesure : `/usr/bin/time -f "%e s"`, **médiane de 4 exécutions**.

### 13.2 Résultats

| Implémentation | Simulations | Temps médian | Débit |
|---|---|---|---|
| **Rust** (`--release`) | 9 000 | **1,27 s** | **~7 060 sim/s** |
| **Python** 3.11.16 | 900 | 2,83 s | ~318 sim/s |

**Accélération : ~22×** (7 060 / 318).

Extrapolation : les 9 000 simulations prendraient **~28,3 s** en Python contre
**1,27 s** en Rust.

### 13.3 Variabilité observée

| Exécution | Rust (9 000) | Python (900) |
|---|---|---|
| 1 | 1,14 s | 2,82 s |
| 2 | 1,21 s | 3,41 s |
| 3 | 1,34 s | 2,75 s |
| 4 | 1,40 s | 2,84 s |

La dispersion Rust (~±10 %) provient de la charge de la machine virtuelle, pas
du code. Le rapport reste stable entre **22× et 25×**.

> **Ces chiffres remplacent les valeurs antérieures du README** (« 13× »,
> « 6 977 sim/s », « 535 sim/s », « 1,29 s »), qui étaient inexactes. Toute
> mesure publiée doit être reproductible par le protocole ci-dessus.

---

## 14. Résultats de référence

Résultats obtenus avec `--all --seeds 1001-1100` (100 graines par test).
Ils servent de **point de comparaison** : toute divergence signale une
régression.

| Test | Accord | Médiane conv. | Messages moy. | Re-fusion | Divergence |
|---|---|---|---|---|---|
| T1 | 100/100 | 5 | 477,3 | — | — |
| T2 | 100/100 | 5 | 477,3 | — | — |
| T3 | 100/100 | 5 | 533,1 | — | — |
| T4 | 100/100 | 5 | 954,5 | — | — |
| T5 | 100/100 | 5 | 3 439,9 | méd. 3 | 35/100 |
| T5D | 100/100 | 103 | 9 094,0 | méd. 3 | **100/100** |
| T6 | 100/100 | 5 | 477,3 | — | — |
| T8 | **98/100** | 5 | 797,9 | — | — |
| T9 | 100/100 | 5 | 3 439,9 | méd. 3 | 35/100 |

### 14.1 Lecture des résultats

- **T1/T2/T6** — identiques (mêmes paramètres). Convergence en 5 périodes
  médianes, ~477 messages.
- **T3** (perte 25 %) — convergence **toujours** atteinte, mais avec ~12 % de
  messages en plus : la perte coûte du trafic, pas la convergence.
- **T4** (duplication ×2) — messages **doublés** (954,5 ≈ 2 × 477,3), comme
  attendu. La convergence n'est pas plus rapide : la duplication n'apporte
  rien ici, elle coûte.
- **T5** — 35/100 graines divergent réellement, avec une re-fusion en
  **3 périodes** dans tous les cas. Les 65 autres convergent avant la fin de
  la partition (le maximum était présent des deux côtés).
- **T5D** — divergence **garantie** sur 100/100 graines, re-fusion en
  **3 périodes** systématiquement. C'est le test qui exerce réellement la
  re-fusion.
- **T8** — 98/100 convergent. Les 2 échecs sont expliqués en §15.

### 14.2 Sur 1 000 graines

| Test | Accord | Observations |
|---|---|---|
| T3 | 1000/1000 | convergence max en 11 périodes |
| T5 | 1000/1000 | 339/1000 divergent réellement (re-fusion en 3) |
| T5D | 1000/1000 | **résultat unique** : 103 / 9 094 / 30 / 3 |
| T8 | **990/1000** | 10 échecs, tous expliqués par un agent isolé |

**T5D produit un résultat identique pour les 1 000 graines** (103, 9 094, 30,
3). Ce n'est pas un bug : `forcer_divergence` plafonne la moitié droite de
façon déterministe, et la partition est symétrique. La trajectoire devient
indépendante de la graine. C'est précisément l'objectif du test — mesurer la
re-fusion dans le cas le plus défavorable, de façon reproductible.

---

## 15. Limites connues et comportements attendus

### 15.1 Les échecs de T8 sont des agents isolés, pas des bugs

Sur 1 000 graines, **10 échecs** (1 %). Tous ont la **même cause** :

> Le retrait à `t = 3` supprime les **deux** voisins d'un agent situé dans un
> **coin** de la grille. Cet agent devient **isolé** : il ne peut plus ni
> émettre ni recevoir. Il conserve sa valeur initiale, et l'accord n'est
> jamais atteint.

Vérification exhaustive sur les 10 graines concernées :

| Graine | Agents retirés | Agent isolé | Ses voisins |
|---|---|---|---|
| 1010 | 4, 11, 13 | **5** | 4, 11 |
| 1060 | 4, 18, 25 | **24** | 25, 18 |
| 1228 | 1, 6, 11 | **0** | 1, 6 |
| 1350 | 11, 23, 28 | **29** | 28, 23 |
| 1363 | 4, 11, 13 | **5** | 4, 11 |
| 1536 | 1, 6, 25 | **0** | 1, 6 |
| 1631 | 13, 18, 25 | **24** | 25, 18 |
| 1639 | 1, 6, 14 | **0** | 1, 6 |
| 1687 | 3, 18, 25 | **24** | 25, 18 |
| 1699 | 14, 18, 25 | **24** | 25, 18 |

**10/10 échecs expliqués.** Les agents isolés sont **toujours** des coins
(0, 5, 24, 29) — les seuls nœuds à 2 voisins, donc les seuls qu'un retrait de
2 agents peut couper du réseau.

**Ce n'est pas un défaut de l'algorithme** : c'est une propriété de la
topologie. Un agent sans aucun lien ne peut pas participer à un consensus
distribué. Le résultat est **correct** et **documenté**.

> **La v2 corrige ce cas.** Voir la section 20 — la reconnexion des agents
> isolés ramène T8 à **1000/1000** sans aucune régression.

### 15.2 Le retrait peut ne pas avoir lieu

Si la convergence survient **avant** `t_retrait`, la simulation s'arrête
(`break`) et le retrait n'est **jamais appliqué**. Sur 1 000 graines T8 :

- **974** ont `taille_etat = 27` (retrait appliqué) ;
- **26** ont `taille_etat = 30` (convergence à `t = 2`, avant `t = 3`).

Ces 26 graines sont **toutes** convergées. C'est un comportement volontaire,
identique dans les deux implémentations, et c'est ce qui rend la parité
possible.

### 15.3 `max_periodes` contrôle aussi la partition

`fin_partition = max_periodes / 2`. Modifier `max_periodes` change donc la
**durée** de la partition, pas seulement le nombre d'itérations maximales.
Pour allonger la simulation sans changer la partition, il faut modifier la
formule dans **les deux** implémentations.

### 15.4 Le fanout ne limite rien en pratique

`FANOUT = 6` alors que le degré maximal de la grille est **4**. Le tirage
`sample(cibles, min(6, len(cibles)))` sélectionne donc **toujours tous les
voisins**. Le fanout n'a d'effet que sur l'**ordre** de contact (qui influence
la consommation du RNG, donc les résultats). Il ne modélise pas une limitation
de bande passante.

### 15.5 La latence n'est pas exercée par le harnais

Le paramètre `latence` existe et fonctionne, mais **aucun des 9 tests** ne
l'utilise (`latence = 0` partout). Les résultats publiés ne couvrent donc pas
le cas des messages retardés. C'est une **lacune de couverture** assumée : le
harnais suit le tableau §4.7.1 de la spécification v5.

### 15.6 La valeur est un entier de 0 à 9

`randint(0, 9)` fixe le domaine des valeurs initiales. L'algorithme lui-même
ne dépend pas de ce domaine (le `max` fonctionne sur tout type totalement
ordonné), mais les résultats publiés sont spécifiques à ce domaine.

---

## 16. Structure du dépôt

```
consensus_rs/
├── Cargo.toml                  # métadonnées du crate (MIT, édition 2024)
├── Cargo.lock                  # verrouillage (aucune dépendance)
├── LICENSE                     # MIT — Copyright (c) 2026 Christophe Dagorn
├── README.md                   # ce document
├── .gitignore                  # /target, __pycache__, *.csv
│
├── src/
│   ├── lib.rs                  # racine du crate, ré-exports, documentation
│   ├── rng.rs                  # MT19937 compatible CPython (random, randint, sample)
│   ├── sim.rs                  # simulateur : état, fusion, gossip, topologie, tests
│   └── main.rs                 # CLI compatible avec sim_consensus.py
│
├── tests/
│   ├── parite.rs               # tests de parité avec CPython
│   └── REFERENCE_PYTHON.txt    # valeurs de référence produites par CPython 3.11.16
│
├── reference/
│   └── sim_consensus.py        # simulateur Python de référence (embarqué, 204 lignes)
│
└── verify_parite_rust.py       # comparaison automatique Rust vs Python
```

### 16.1 Rôle de chaque fichier

| Fichier | Lignes | Rôle |
|---|---|---|
| `src/lib.rs` | 30 | Point d'entrée du crate, documentation du modèle |
| `src/rng.rs` | 311 | RNG compatible CPython — **le fichier critique** |
| `src/sim.rs` | 355 | Simulateur complet + 8 tests unitaires |
| `src/main.rs` | 210 | CLI, table des 9 tests, écriture CSV, résumé |
| `tests/parite.rs` | 81 | 5 tests d'intégration contre CPython |
| `reference/sim_consensus.py` | 204 | Référence Python, bibliothèque standard |
| `verify_parite_rust.py` | 103 | Harnais de comparaison automatique |

---

## 17. Dépannage

### `error: edition 2024 is unstable` ou erreur de compilation

**Cause** : `rustc` trop ancien. **Solution** : `rustup update stable`, puis
vérifier `rustc --version` ≥ 1.85.

### `--seeds attendu au format A-B`

**Cause** : format de plage invalide (`1001`, `1001:1100`, `1001-`).
**Solution** : utiliser exactement `A-B`, par exemple `--seeds 1001-1100`.

### `graine de début invalide`

**Cause** : borne non numérique ou négative. **Solution** : entiers non signés
uniquement (`u64`).

### `création du CSV impossible`

**Cause** : répertoire de sortie inexistant ou non inscriptible.
**Solution** : vérifier le chemin passé à `--out`, ou écrire dans `/tmp`.

### Le simulateur est lent

**Cause** : compilation en mode debug. **Solution** : `cargo build --release`
et exécuter `target/release/consensus_rs` (pas `target/debug/`).

### `verify_parite_rust.py` signale des écarts

**Cause** : version de CPython différente de 3.11.16, ou défaut dans
`src/rng.rs`. **Solution** : suivre la procédure de diagnostic §12.4.

### `FileNotFoundError: target/release/consensus_rs`

**Cause** : le binaire n'a pas été compilé en `--release`.
**Solution** : `cargo build --release` avant de lancer la vérification.

### `panicked at 'sample: k > n'`

**Cause** : appel direct à `PyRandom::sample` avec `k` supérieur à la taille de
la population. **Solution** : respecter `k ≤ population.len()`.

---

## 18. Glossaire

| Terme | Définition |
|---|---|
| **Agent** | Nœud du réseau simulé. Ici, un drone de l'essaim. |
| **CRDT** | *Conflict-free Replicated Data Type* — structure dont la fusion est commutative, associative et idempotente, donc convergente sans coordination. |
| **Semi-treillis** | Ensemble muni d'une opération `⊔` commutative, associative et idempotente. Ici : `max` sur `(valeur, horloge)`. |
| **Gossip** | Protocole de diffusion où chaque nœud échange périodiquement son état avec un sous-ensemble de voisins. |
| **Fanout** | Nombre de voisins contactés par période. |
| **Horloge logique** | Compteur par agent, utilisé pour départager deux valeurs égales. |
| **Convergence** | État où tous les agents actifs partagent la même valeur. |
| **Partition** | Coupure temporaire du réseau en deux moitiés qui ne communiquent plus. |
| **Re-fusion** | Retour à l'accord après la levée d'une partition. |
| **Divergence réelle** | Situation où les deux moitiés ont convergé vers des valeurs **différentes** à la fin de la partition (et non une différence transitoire). |
| **Retrait** | Disparition d'un agent (batterie, crash, sortie de zone). |
| **Parité bit-à-bit** | Identité exacte des sorties entre deux implémentations, pour les mêmes entrées. |
| **MT19937** | Mersenne Twister, générateur pseudo-aléatoire utilisé par CPython. |
| **`init_by_array`** | Initialisation de MT19937 par tableau de clés, utilisée par `random.Random(seed)`. |
| **`_randbelow`** | Méthode de CPython pour tirer un entier uniforme dans `[0, n)` par rejet. |
| **`setsize`** | Seuil de CPython (`21 + 4^ceil(log(k·3,4))`) au-delà duquel `sample` change d'algorithme. |

---

## 19. Licence

**MIT** — voir le fichier [`LICENSE`](LICENSE).

Copyright (c) 2026 Christophe Dagorn.

---

## 20. Version 2 — reconnexion des agents isolés

La **v2** est un **mode additionnel**. Elle ne remplace pas la v1 : sans
l'option `--v2`, le comportement est **strictement identique** à la v1, à la
parité bit-à-bit près.

### 20.1 Le défaut corrigé

La v1 ne peut pas atteindre l'accord quand un agent se retrouve **sans aucun
pair joignable** (section 15.1). Le mécanisme v2 traite exactement ce cas.

### 20.2 Le mécanisme

Trois éléments, dans `src/reconnexion.rs` et `src/sim.rs` :

1. **Détection de l'isolement réel.** La v1 n'exclut pas les agents inactifs
   de ses cibles d'émission : un agent peut « émettre » vers des voisins
   retirés, qui ne reçoivent rien. La v2 ne compte que les pairs
   **effectivement joignables**.
2. **Élargissement monotone du rayon.** Un agent isolé élargit son rayon de
   contact (distance de Chebyshev) d'une unité par période d'isolement,
   plafonné à `RAYON_MAX`. Le rayon **ne redescend jamais** : sans cette
   monotonie, l'agent oscillerait entre rayon 1 (isolé) et rayon 2 (connecté)
   sans jamais rester connecté assez longtemps pour converger.
3. **Réception élargie.** Un agent isolé lit l'état des pairs de son rayon
   élargi. Sans cela, le mécanisme serait **asymétrique** : l'agent isolé
   émettrait vers des pairs éloignés, mais ces pairs ne le compteraient pas
   parmi leurs propres cibles et ne lui répondraient jamais. C'est cette
   asymétrie qui bloquait l'accord.

### 20.3 Résultats mesurés

Sur 1 000 graines (1001–2000) :

| Test | v1 | v2 | Δ accord | Messages v1 | Messages v2 | Δ messages |
|---|---|---|---|---|---|---|
| T1 | 1000/1000 | 1000/1000 | 0 | 470 204 | 470 204 | 0,0 % |
| T2 | 1000/1000 | 1000/1000 | 0 | 470 204 | 470 204 | 0,0 % |
| T3 | 1000/1000 | 1000/1000 | 0 | 529 396 | 529 396 | 0,0 % |
| T4 | 1000/1000 | 1000/1000 | 0 | 940 408 | 940 408 | 0,0 % |
| T5 | 1000/1000 | 1000/1000 | 0 | 3 348 978 | 3 348 978 | 0,0 % |
| T5D | 1000/1000 | 1000/1000 | 0 | 9 094 000 | 9 094 000 | 0,0 % |
| T6 | 1000/1000 | 1000/1000 | 0 | 470 204 | 470 204 | 0,0 % |
| **T8** | **990/1000** | **1000/1000** | **+10** | **619 038** | **420 982** | **−32,0 %** |
| T9 | 1000/1000 | 1000/1000 | 0 | 3 348 978 | 3 348 978 | 0,0 % |

**Lecture.** Le gain est **ciblé sur T8** : c'est le seul test qui produit des
agents isolés. Sur les huit autres tests, le mécanisme ne se déclenche jamais
et le résultat est **strictement identique** — c'est le comportement attendu
d'un mode additionnel.

Le **−32 % de messages** sur T8 est un effet secondaire : en v1, l'agent isolé
émettait chaque période vers des voisins retirés (messages perdus). La v2
supprime ce trafic inutile.

**Aucune régression** : sur les 1 000 graines, aucune graine qui convergeait
en v1 n'échoue en v2.

### 20.4 Utilisation

```bash
# v1 (comportement historique, par défaut)
consensus_rs --test T8 --seeds 1001-2000 --out t8_v1.csv

# v2 (reconnexion active)
consensus_rs --test T8 --seeds 1001-2000 --out t8_v2.csv --v2
```

En bibliothèque :

```rust
use consensus_rs::sim::{simuler, Params};

let p = Params { reconnexion: true, ..Default::default() };
let r = simuler(1010, &p);
assert!(r.accord);
```

### 20.5 Garanties

- **Parité v1 préservée** : 9 000 graines, 0 écart (`verify_parite_rust.py`).
- **26/26 tests** passent (21 unitaires + 5 d'intégration), 0 warning.
- **Non-régression testée** : `v2_ne_degrade_pas_les_graines_qui_convergeaient`
  vérifie sur 200 graines qu'aucune ne passe d'accord à échec.

---

## 21. Version 3 — quiescence (loop engineering)

### 21.1 Le problème laissé par la v2

La v2 corrige l'isolement des agents, mais elle conserve un défaut de fond :
**chaque agent émet à chaque période, indéfiniment**, même quand tous les
agents partagent déjà la même valeur. Le coût en messages est donc
proportionnel au nombre de périodes simulées, pas à l'information réellement
transportée.

Mesures v2 sur 1 000 graines :

- T1 : 470 messages en moyenne, convergence médiane à 5 périodes.
- T5D : **9 094 messages**, convergence à 103 périodes.

T5D transporte une information qui tient en quelques dizaines de messages
utiles, mais en consomme 9 094.

### 21.2 Le mécanisme

Chaque agent tient un compteur `stable[i]` : nombre de périodes consécutives
pendant lesquelles son état n'a pas changé. Au-delà de `SEUIL_QUIESCENCE`
(8 périodes), l'agent cesse d'émettre — il est **quiescent**.

Un agent quiescent **reste récepteur** : s'il reçoit un état différent, son
compteur retombe à zéro et il se réveille.

### 21.3 Le piège : la stabilité est locale

La première implémentation — quiescence pure, sans réveil — **casse la
convergence**. Mesuré : T5D passe de 1000/1000 à **0/1000** d'accord.

La cause est structurelle. Dans T5D, une partition force une divergence entre
deux moitiés. Chaque moitié converge **en interne** bien avant la re-fusion :
tous les agents deviennent donc quiescents vers la période 8. Quand la
partition tombe à t = 103, **plus personne n'émet** : les deux moitiés ne se
reparlent jamais.

La stabilité est une propriété **locale**. Un agent stable dans sa composante
peut appartenir à une composante qui doit encore fusionner avec une autre.

### 21.4 La correction : réveil périodique

Un agent quiescent émet quand même tous les `PERIODE_REVEIL` (16) tours. Cela
garantit qu'une information nouvelle finit toujours par circuler, au prix
d'un coût résiduel borné.

```rust
pub fn doit_emettre(stable: usize) -> bool {
    if stable < SEUIL_QUIESCENCE {
        return true;
    }
    stable % PERIODE_REVEIL == 0
}
```

### 21.5 Résultats mesurés (1 000 graines par test, 9 tests)

| test | v2 accord | v2 messages | v3 accord | v3 messages | delta |
|------|-----------|-------------|-----------|-------------|-------|
| T1   | 1000/1000 | 470 204     | 1000/1000 | 470 166     | −0,0 % |
| T3   | 1000/1000 | 529 396     | 1000/1000 | 529 207     | −0,0 % |
| T4   | 1000/1000 | 940 408     | 1000/1000 | 940 332     | −0,0 % |
| T5   | 1000/1000 | 3 348 978   | 1000/1000 | 795 873     | **−76,2 %** |
| T5D  | 1000/1000 | 9 094 000   | 1000/1000 | 1 541 184   | **−83,1 %** |
| T8   | 1000/1000 | 420 982     | 1000/1000 | 420 875     | −0,0 % |
| T9   | 1000/1000 | 3 348 978   | 1000/1000 | 795 873     | **−76,2 %** |

**Total : −66,3 % de messages par rapport à la v2, −66,6 % par rapport à la
v1, avec un accord de 1000/1000 sur les 9 tests.**

Le seul coût : T5D converge à 117 périodes au lieu de 103 (+14). C'est le prix
du réveil périodique — échange très favorable.

### 21.6 Utilisation

En ligne de commande :

```bash
consensus_rs --test T5D --seeds 1001-2000 --v3 --out resultats.csv
```

En bibliothèque :

```rust
use consensus_rs::sim::{simuler, Params};

let p = Params { reconnexion: true, quiescence: true, ..Default::default() };
let r = simuler(1001, &p);
assert!(r.accord);
```

### 21.7 Garanties

- **Parité v1 préservée** : 9 000 graines, 0 écart (`verify_parite_rust.py`).
- **30/30 tests** passent (25 unitaires + 5 d'intégration), 0 warning.
- **Non-régression mesurée** : accord v3 ≥ accord v2 sur les 9 tests.

### 21.8 Méthode : loop engineering

La v3 a été produite en appliquant les *building blocks* de **loop
engineering** (Lulla et al., 2026, arXiv:2608.21884v2) :

- **Goal & stop condition** — objectif : réduire le coût en messages sans
  dégrader l'accord. Critère d'arrêt machine-checkable : accord v3 ≥ accord v2
  sur les 9 tests.
- **State & memory** — les mesures sont persistées dans des CSV et comparées
  par `compare_v1_v2_v3.py`.
- **Verification (maker/checker)** — le harnais de mesure est indépendant du
  code de simulation ; la parité v1 est vérifiée par un script séparé.
- **Human oversight** — la v3 reste sur sa branche, non fusionnée dans
  `master` : le changement de comportement par défaut est une décision humaine.
- **Budgets** — la boucle est bornée : 9 tests × 1 000 graines par itération.

---

## Références

- Spécification **ALG_CONSENSUS v5**, §4.7.1 (harnais de tests).
- Matsumoto, M. & Nishimura, T. (1997). *Mersenne Twister: A 623-dimensionally
  equidistributed uniform pseudo-random number generator.* ACM TOMACS.
- CPython, `Lib/random.py` et `Modules/_randommodule.c` — sémantique de
  `random.Random`, `randint`, `sample`, `getrandbits`.
- Shapiro, M. et al. (2011). *Conflict-free Replicated Data Types.* SSS 2011.
- Lulla, J., Nersesyan, A., Mohsenimofidi, S., Treude, C. & Baltes, S. (2026).
  *Loop Engineering: A Framework for Automated Control Structures over Coding
  Agents.* arXiv:2608.21884v2. JAWs@ASE 2026.
