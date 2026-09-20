#!/usr/bin/env python3
"""Comparaison v1 / v2 / v3 sur les 9 tests, 1000 graines.

Lit les CSV produits par le CLI et verifie :
  - accord identique ou meilleur en v3
  - reduction du cout en messages
"""
import csv
import subprocess
import sys

TESTS = ["T1", "T2", "T3", "T4", "T5", "T5D", "T6", "T8", "T9"]
BIN = "./target/release/consensus_rs"


def run(flag, test):
    out = f"/tmp/_cmp_{test}_{flag or 'v1'}.csv"
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


print(f"{'test':5} | {'v1 accord':>10} {'msg':>9} | {'v2 accord':>10} {'msg':>9} | {'v3 accord':>10} {'msg':>9} | {'v3 vs v2':>9}")
print("-" * 92)
tot = {"v1": 0, "v2": 0, "v3": 0}
regressions = []
for t in TESTS:
    a1, m1, c1, n = run(None, t)
    a2, m2, c2, _ = run("--v2", t)
    a3, m3, c3, _ = run("--v3", t)
    tot["v1"] += m1
    tot["v2"] += m2
    tot["v3"] += m3
    if a3 < a2:
        regressions.append((t, a2, a3))
    delta = (m3 - m2) / m2 * 100 if m2 else 0
    print(f"{t:5} | {a1:5}/{n:<4} {m1:9} | {a2:5}/{n:<4} {m2:9} | {a3:5}/{n:<4} {m3:9} | {delta:+8.1f}%")

print()
print(f"TOTAL messages : v1={tot['v1']}  v2={tot['v2']}  v3={tot['v3']}")
print(f"v3 vs v2 : {(tot['v3']-tot['v2'])/tot['v2']*100:+.1f}%")
print(f"v3 vs v1 : {(tot['v3']-tot['v1'])/tot['v1']*100:+.1f}%")
print()
if regressions:
    print("REGRESSIONS D'ACCORD :", regressions)
    sys.exit(1)
print("Aucune regression d'accord : v3 >= v2 sur les 9 tests.")
