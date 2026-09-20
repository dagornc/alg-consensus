#!/usr/bin/env python3
"""Compare v1 et v2 sur tous les tests du harnais."""
import subprocess, csv, io, os

RACINE = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(RACINE, "target", "release", "consensus_rs")
TESTS = ["T1", "T2", "T3", "T4", "T5", "T5D", "T6", "T8", "T9"]

def run(test, v2):
    args = [BIN, "--test", test, "--seeds", "1001-2000", "--out", "/tmp/_cmp.csv"]
    if v2:
        args.append("--v2")
    subprocess.run(args, capture_output=True, check=True)
    rows = list(csv.DictReader(open("/tmp/_cmp.csv")))
    acc = sum(1 for r in rows if r["accord"] == "1")
    msg = sum(int(r["messages"]) for r in rows)
    return acc, len(rows), msg

print(f"{'test':6} {'v1 accord':>12} {'v2 accord':>12} {'delta':>7} {'msg v1':>10} {'msg v2':>10} {'delta%':>8}")
print("-" * 72)
tot1 = tot2 = 0
for t in TESTS:
    a1, n, m1 = run(t, False)
    a2, _, m2 = run(t, True)
    tot1 += m1
    tot2 += m2
    d = a2 - a1
    dp = (m2 - m1) / m1 * 100 if m1 else 0
    print(f"{t:6} {a1:>7}/{n:<4} {a2:>7}/{n:<4} {d:>+7} {m1:>10} {m2:>10} {dp:>+7.1f}%")
print("-" * 72)
print(f"{'TOTAL':6} {'':>12} {'':>12} {'':>7} {tot1:>10} {tot2:>10} {(tot2-tot1)/tot1*100:>+7.1f}%")
