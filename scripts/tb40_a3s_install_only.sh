#!/usr/bin/env bash
# Harbor --install-only smoke for the A3S Code adapter (no model API key required).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.local/bin:$PATH"
export PYTHONPATH="$REPO_ROOT/scripts/harbor${PYTHONPATH:+:$PYTHONPATH}"
JOB_DIR="${A3S_HARBOR_JOBS_DIR:-$REPO_ROOT/.harbor-smoke}"
HARBOR_PY="${HARBOR_PY:-$HOME/.local/share/uv/tools/harbor/bin/python}"

"$HARBOR_PY" - <<PY
import sys
sys.path.insert(0, r"$REPO_ROOT/scripts/harbor")
from a3s_code_agent.agent import A3sCodeAgent
print("IMPORT_OK", A3sCodeAgent.name())
PY

mkdir -p "$JOB_DIR"
cd "$JOB_DIR"

set +e
harbor run \
  -d "terminal-bench/terminal-bench@4.0.0" \
  -a "a3s_code_agent.agent:A3sCodeAgent" \
  -e docker \
  -n 1 \
  -l 1 \
  -k 1 \
  --install-only \
  --jobs-dir "$JOB_DIR/jobs" 2>&1 | tee "$JOB_DIR/a3s-install-only.log"
RC=${PIPESTATUS[0]}
set -e

"$HARBOR_PY" - <<PY
import json
from pathlib import Path
result_dirs = sorted(Path(r"$JOB_DIR/jobs").glob("*"), key=lambda p: p.stat().st_mtime)
n_errors = None
if result_dirs:
    candidate = result_dirs[-1] / "result.json"
    if candidate.exists():
        payload = json.loads(candidate.read_text(encoding="utf-8"))
        n_errors = payload.get("stats", {}).get("n_errored_trials")
effective_rc = $RC
if n_errors:
    effective_rc = 1
print(f"INSTALL_ONLY_RC={effective_rc}")
raise SystemExit(effective_rc)
PY
