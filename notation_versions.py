#!/usr/bin/env python3
"""Notation /20 des 3 versions de ALG_CONSENSUS.

Grille : 5 criteres ponderes, notes a partir des mesures REELLES
(1000 graines par test, 9 tests).

Criteres et ponderations :
  A. Correction (accord)        6 pts  — le critere premier : le consensus converge-t-il ?
  B. Cout en messages           5 pts  — efficacite de la dissemination
  C. Latence de convergence     3 pts  — rapidite
  D. Robustesse (partition)     3 pts  — comportement sous partition/divergence
  E. Qualite d'ingenierie       3 pts  — tests, parite, documentation, non-regression
"""
import csv
import subprocess

BIN = "./target/release/consensus_rs"
TESTS = ["T1", "T2", "T3", "T4", "T5", "T5D", "T6", "T8", "T9"]


def mesure(flag, test):
    out = f"/tmp/_note_{test}_{flag or 'v1'}.csv"
    cmd = [BIN, "--test", test, "--seeds", "1001-2000", "--out", out]
    if flag:
        cmd.append(flag)
    subprocess.run(cmd, check=True, capture_output=True)
    rows = list(csv.DictReader(open(out)))
    acc = sum(1 for r in rows if r["accord"] == "1")
    msg = sum(int(r["messages"]) for r in rows)
    conv = sorted(int(r["periode_convergence"]) for r in rows if r["periode_convergence"])
    med = conv[len(conv) // 2] if conv else 0
    return acc, msg, med, len(rows)


data = {}
for label, flag in [("v1", None), ("v2", "--v2"), ("v3", "--v3")]:
    data[label] = {t: mesure(flag, t) for t in TESTS}

print("=== Mesures brutes (1000 graines/test) ===")
print(f"{'test':5} | {'v1 acc':>8} {'msg':>9} {'conv':>5} | {'v2 acc':>8} {'msg':>9} {'conv':>5} | {'v3 acc':>8} {'msg':>9} {'conv':>5}")
for t in TESTS:
    a1, m1, c1, n = data["v1"][t]
    a2, m2, c2, _ = data["v2"][t]
    a3, m3, c3, _ = data["v3"][t]
    print(f"{t:5} | {a1:5}/{n:<3} {m1:9} {c1:5} | {a2:5}/{n:<3} {m2:9} {c2:5} | {a3:5}/{n:<3} {m3:9} {c3:5}")

# --- Criteres ---
def total_msg(label):
    return sum(data[label][t][1] for t in TESTS)

def total_acc(label):
    return sum(data[label][t][0] for t in TESTS)

def max_conv(label):
    return max(data[label][t][2] for t in TESTS)

N_TOTAL = 9 * 1000

print()
print("=== Agregats ===")
for label in ["v1", "v2", "v3"]:
    print(f"{label} : accord={total_acc(label)}/{N_TOTAL}  messages={total_msg(label)}  conv_max={max_conv(label)}")

# A. Correction (6 pts) : proportion d'accord sur l'ensemble des 9 tests.
#    v1 perd 10 graines sur T8 ; v2 et v3 sont parfaits.
print()
print("=== Notation ===")
notes = {}
for label in ["v1", "v2", "v3"]:
    acc = total_acc(label) / N_TOTAL
    A = 6.0 * acc

    # B. Cout (5 pts) : normalise sur le meilleur (v3).
    best = min(total_msg(l) for l in ["v1", "v2", "v3"])
    B = 5.0 * best / total_msg(label)

    # C. Latence (3 pts) : normalise sur la meilleure convergence max.
    bestc = min(max_conv(l) for l in ["v1", "v2", "v3"])
    C = 3.0 * bestc / max_conv(label)

    # D. Robustesse partition (3 pts) : accord sur T5/T5D/T9.
    part = sum(data[label][t][0] for t in ["T5", "T5D", "T9"]) / 3000
    D = 3.0 * part

    # E. Ingenierie (3 pts) : jugement documente (voir rapport).
    E = {"v1": 2.0, "v2": 2.6, "v3": 2.9}[label]

    total = A + B + C + D + E
    notes[label] = (A, B, C, D, E, total)
    print(f"{label} : A={A:.2f}/6  B={B:.2f}/5  C={C:.2f}/3  D={D:.2f}/3  E={E:.2f}/3  => {total:.1f}/20")

print()
print("=== Classement ===")
for label, (_, _, _, _, _, tot) in sorted(notes.items(), key=lambda kv: -kv[1][5]):
    print(f"  {label} : {tot:.1f}/20")
