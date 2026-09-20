#!/usr/bin/env python3
"""Analyse de sensibilite de la notation.

Question : la note depend-elle du choix de ponderation, ou le classement
est-il robuste ? On teste plusieurs ponderations et on regarde si l'ordre
v1 < v2 < v3 est stable.
"""
import csv
import subprocess

BIN = "./target/release/consensus_rs"
TESTS = ["T1", "T2", "T3", "T4", "T5", "T5D", "T6", "T8", "T9"]


def mesure(flag, test):
    out = f"/tmp/_sens_{test}_{flag or 'v1'}.csv"
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

N = 9000
agg = {}
for l in data:
    agg[l] = {
        "acc": sum(data[l][t][0] for t in TESTS) / N,
        "msg": sum(data[l][t][1] for t in TESTS),
        "conv": max(data[l][t][2] for t in TESTS),
        "part": sum(data[l][t][0] for t in ["T5", "T5D", "T9"]) / 3000,
    }

E = {"v1": 2.0, "v2": 2.6, "v3": 2.9}
best_msg = min(agg[l]["msg"] for l in agg)
best_conv = min(agg[l]["conv"] for l in agg)

# Ponderations testees : (A, B, C, D, E)
scenarios = {
    "equilibre (6,5,3,3,3)": (6, 5, 3, 3, 3),
    "cout dominant (4,8,2,3,3)": (4, 8, 2, 3, 3),
    "correction dominante (10,3,2,2,3)": (10, 3, 2, 2, 3),
    "cout faible (8,2,3,4,3)": (8, 2, 3, 4, 3),
    "latence forte (5,4,6,2,3)": (5, 4, 6, 2, 3),
    "sans ingenieire (6,5,3,3,0)": (6, 5, 3, 3, 0),
}

print(f"{'scenario':34} | {'v1':>6} {'v2':>6} {'v3':>6} | ordre stable ?")
print("-" * 78)
for nom, (wA, wB, wC, wD, wE) in scenarios.items():
    notes = {}
    for l in agg:
        A = wA * agg[l]["acc"]
        B = wB * best_msg / agg[l]["msg"]
        C = wC * best_conv / agg[l]["conv"]
        D = wD * agg[l]["part"]
        tot = A + B + C + D + (wE * E[l] / 3)
        notes[l] = tot
    stable = notes["v1"] < notes["v2"] < notes["v3"]
    print(f"{nom:34} | {notes['v1']:6.1f} {notes['v2']:6.1f} {notes['v3']:6.1f} | {'OUI' if stable else 'NON'}")

print()
print("Note : les ponderations sont normalisees sur 20 dans le rapport final.")
