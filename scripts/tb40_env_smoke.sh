#!/usr/bin/env bash
# Host environment + oracle smoke for Terminal-Bench 4.0 / Harbor.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.local/bin:$PATH"
JOB_DIR="${A3S_HARBOR_JOBS_DIR:-$REPO_ROOT/.harbor-smoke}"

checks_json=$(python3 - <<'PY'
import json, subprocess
checks = {}
cmds = {
  "python": ["python3", "--version"],
  "uv": ["uv", "--version"],
  "harbor": ["harbor", "--version"],
  "docker": ["docker", "ps"],
  "gpu": ["nvidia-smi", "--query-gpu=name,memory.total", "--format=csv"],
}
for key, cmd in cmds.items():
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        checks[key] = {"rc": p.returncode, "out": (p.stdout or p.stderr).strip()[:400]}
    except Exception as exc:
        checks[key] = {"rc": -1, "out": str(exc)}
print(json.dumps(checks))
PY
)

echo "PRECHECK=$checks_json"

mkdir -p "$JOB_DIR"
cd "$JOB_DIR"

set +e
harbor run \
  -d "terminal-bench/terminal-bench@4.0.0" \
  -a oracle \
  -e docker \
  -n 1 \
  -l 1 \
  -k 1 \
  --jobs-dir "$JOB_DIR/jobs" 2>&1 | tee "$JOB_DIR/oracle-smoke.log"
RC=${PIPESTATUS[0]}
set -e

echo "ORACLE_RC=$RC"
exit "$RC"
