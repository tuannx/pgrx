#!/usr/bin/env bash
# Rerun the pgrx perf benchmarks and compare against perf/baseline.json.
#
# Scope: pure-Rust benches in pgrx-sql-entity-graph (control-file parsing,
# entity graph build, SQL generation). The #[pg_bench] benches in
# pgrx-examples/benching need a live PostgreSQL server and are not run here.
#
# Verdict: regression >10% = warn, >25% = block, otherwise pass.
# Exit code: 0 pass, 1 warn, 2 block.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

cargo bench -p pgrx-sql-entity-graph --bench sql_entity_graph

python3 - "$ROOT" <<'PYEOF'
import json, os, sys

root = sys.argv[1]
with open(os.path.join(root, "perf", "baseline.json")) as f:
    baseline = json.load(f)

worst = 0  # 0 pass, 1 warn, 2 block
print(f"{'bench':38} {'baseline':>14} {'current':>14} {'delta':>9}  verdict")
for name, entry in sorted(baseline["benches"].items()):
    est_path = os.path.join(
        root, "target", "criterion", name, "new", "estimates.json"
    )
    if not os.path.exists(est_path):
        print(f"{name:38} {'?':>14} {'missing':>14} {'':>9}  block (no result)")
        worst = 2
        continue
    with open(est_path) as f:
        estimates = json.load(f)
    current = estimates["mean"]["point_estimate"]
    base = entry["mean_ns_per_iter"]
    delta = (current - base) / base * 100.0
    if delta > 25.0:
        verdict, level = "block", 2
    elif delta > 10.0:
        verdict, level = "warn", 1
    elif delta < 0:
        verdict, level = f"pass (improvement {delta:.1f}%)", 0
    else:
        verdict, level = "pass", 0
    worst = max(worst, level)
    print(f"{name:38} {base:>14.0f} {current:>14.0f} {delta:>+8.1f}%  {verdict}")

print({0: "OVERALL: pass", 1: "OVERALL: warn", 2: "OVERALL: block"}[worst])
sys.exit(worst)
PYEOF
