#!/usr/bin/env python3
"""Vérifie la parité bit-à-bit entre le simulateur Rust et la référence Python.

Compare, pour chaque test et chaque graine, le tuple
(période_convergence, accord, messages, taille_etat, période_refusion,
divergence_reelle) produit par les deux implémentations.

Usage :
    python3 verify_parite_rust.py [--seeds 1001-1100]
"""
import argparse
import os
import subprocess
import sys
import tempfile

# La référence Python est embarquée dans le dépôt (reference/sim_consensus.py),
# ce qui rend la vérification reproductible sans dépendance externe.
RACINE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(RACINE, "reference"))
import sim_consensus as sc  # noqa: E402

BIN = os.path.join(RACINE, "target", "release", "consensus_rs")

# Paramètres identiques au tableau §4.7.1 de la spécification.
PARAMS = {
    "T1": dict(perte=0.0),
    "T2": dict(perte=0.0),
    "T3": dict(perte=0.25),
    "T4": dict(perte=0.0, duplication=2),
    "T5": dict(perte=0.0, partition=True),
    "T5D": dict(perte=0.0, partition=True, forcer_divergence=True),
    "T6": dict(perte=0.0),
    "T8": dict(perte=0.0, retrait=3, t_retrait=3),
    "T9": dict(perte=0.0, partition=True),
}


def lire_csv(chemin):
    """Lit un CSV de résultats et renvoie un dict (test, seed) -> tuple."""
    out = {}
    with open(chemin) as f:
        header = f.readline().strip().split(",")
        for ligne in f:
            champs = ligne.strip().split(",")
            if len(champs) != len(header):
                continue
            d = dict(zip(header, champs))
            cle = (d["test"], int(d["seed"]))
            out[cle] = (
                int(d["periode_convergence"]) if d["periode_convergence"] else None,
                d["accord"] == "1",
                int(d["messages"]),
                int(d["taille_etat"]),
                int(d["periode_refusion"]) if d["periode_refusion"] else None,
                d["divergence_reelle"] == "1",
            )
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds", default="1001-1100")
    ap.add_argument("--v5", action="store_true", help="Vérifier le moteur Rust optimisé v5")
    args = ap.parse_args()
    lo, hi = (int(x) for x in args.seeds.split("-"))

    total = 0
    ecarts = []
    for test, params in PARAMS.items():
        # Référence Python.
        ref = {}
        for s in range(lo, hi + 1):
            r = sc.simuler(s, **params)
            ref[(test, s)] = (r[0], r[1], r[2], r[3], r[4], r[5])
        # Implémentation Rust.
        fd, csv_rs = tempfile.mkstemp(prefix=f"consensus_parite_{test}_", suffix=".csv")
        os.close(fd)
        subprocess.run(
            [BIN, "--test", test, "--seeds", f"{lo}-{hi}", "--out", csv_rs] + (["--v5"] if args.v5 else []),
            check=True,
            capture_output=True,
        )
        rs = lire_csv(csv_rs)
        # Comparaison.
        for s in range(lo, hi + 1):
            total += 1
            a, b = ref[(test, s)], rs.get((test, s))
            if a != b:
                ecarts.append((test, s, a, b))
        os.unlink(csv_rs)

    print(f"Graines comparées : {total}")
    print(f"Écarts             : {len(ecarts)}")
    if ecarts:
        print("\nDétail des écarts (test, seed, python, rust) :")
        for e in ecarts[:20]:
            print(f"  {e}")
        print("\nVERDICT : NON CONFORME")
        return 1
    print("\nVERDICT : PARITÉ BIT-À-BIT CONFORME")
    return 0


if __name__ == "__main__":
    sys.exit(main())
