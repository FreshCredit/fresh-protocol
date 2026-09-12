#!/usr/bin/env python3
"""Regenerate the p10-scan gate baseline from a fresh census.

The baseline is the committed contract the `--gate` mode enforces:
thresholds (hard caps + ratchets) and triage whitelists. Run this after a
deliberate triage session or a refactor that legitimately lowers a metric:

    python3 scripts/quality/p10-scan/update-baseline.py [p10-scan.json]

Review the diff of gate-baseline.json in the same PR — whitelist additions
must cite triage per .freshcredit/quality-audit/04-POWER-OF-10.md.
"""
import json
import sys

CENSUS = sys.argv[1] if len(sys.argv) > 1 else ".freshcredit/quality-audit/p10-scan.json"
OUT = "scripts/quality/p10-scan/gate-baseline.json"

d = json.load(open(CENSUS))
t = d["totals"]

recursion, no_exit, while_flagged = [], [], []
for f in d["files"]:
    for fn in f["functions"]:
        key = f"{f['path']}:{fn['name']}"
        if fn.get("recursive"):
            recursion.append(key)
        if fn.get("loop_no_exit"):
            no_exit.append(key)
        for w in fn.get("while_flagged", []):
            while_flagged.append(f"{f['path']}|{w['idents']}")

baseline = {
    "_comment": (
        "p10-scan --gate baseline. Thresholds are hard caps; *_whitelist "
        "entries encode the 2026-09-08 manual triage (04-POWER-OF-10.md, "
        "while_loop_census.py). New whitelist entries require human triage "
        "in the same PR. Regenerate with update-baseline.py after a "
        "deliberate ratchet change."
    ),
    "max_fns_over_200": t["fns_over_200_lines"],
    "max_unsafe_blocks": t["unsafe_blocks"],
    "min_files": t["files"],
    "prod_zero_asserts": t["prod_fns_with_zero_assert_or_ensure"],
    "recursion_whitelist": sorted(set(recursion)),
    "loop_no_exit_whitelist": sorted(set(no_exit)),
    "while_flagged_whitelist": sorted(set(while_flagged)),
}

with open(OUT, "w") as fh:
    json.dump(baseline, fh, indent=1)
    fh.write("\n")

print(f"wrote {OUT}")
print(f"  max_fns_over_200   = {baseline['max_fns_over_200']}")
print(f"  max_unsafe_blocks  = {baseline['max_unsafe_blocks']}")
print(f"  min_files          = {baseline['min_files']}")
print(f"  prod_zero_asserts  = {baseline['prod_zero_asserts']}")
print(f"  recursion          = {len(baseline['recursion_whitelist'])} entries")
print(f"  loop_no_exit       = {len(baseline['loop_no_exit_whitelist'])} entries")
print(f"  while_flagged      = {len(baseline['while_flagged_whitelist'])} entries")
