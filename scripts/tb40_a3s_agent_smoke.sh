#!/usr/bin/env bash
# 1-task Harbor agent smoke for A3S Code (requires model API key).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.local/bin:$PATH"
export PYTHONPATH="$REPO_ROOT/scripts/harbor${PYTHONPATH:+:$PYTHONPATH}"
JOB_DIR="${A3S_HARBOR_JOBS_DIR:-$REPO_ROOT/.harbor-smoke}"
HARBOR_PY="${HARBOR_PY:-$HOME/.local/share/uv/tools/harbor/bin/python}"
MODEL="${1:-}"
ENV_FILE="$REPO_ROOT/scripts/harbor/.env"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
  echo "loaded_env_file=$ENV_FILE"
else
  echo "no_env_file=$ENV_FILE"
fi

if [[ -z "$MODEL" ]]; then
  MODEL="${A3S_TB_MODEL:-deepseek/deepseek-v4-pro}"
fi
echo "using_model=$MODEL"

have_key=0
for k in OPENAI_API_KEY ANTHROPIC_API_KEY DEEPSEEK_API_KEY OPENROUTER_API_KEY; do
  if [[ -n "${!k:-}" ]]; then
    have_key=1
    echo "detected_key_env=$k"
  fi
done
if [[ "$have_key" -eq 0 ]]; then
  echo "No model API key detected. Create $ENV_FILE from .env.example and set one key." >&2
  echo "Or run: python3 $REPO_ROOT/scripts/harbor/materialize_env_from_a3s_config.py" >&2
  exit 2
fi

mkdir -p "$JOB_DIR"
cd "$JOB_DIR"

set +e
harbor run \
  -d "terminal-bench/terminal-bench@4.0.0" \
  -a "a3s_code_agent.agent:A3sCodeAgent" \
  -m "$MODEL" \
  -e docker \
  -n 1 \
  -l 1 \
  -k 1 \
  --agent-setup-timeout-multiplier 3 \
  --jobs-dir "$JOB_DIR/jobs" 2>&1 | tee "$JOB_DIR/a3s-agent-smoke.log"
RC=${PIPESTATUS[0]}
set -e

"$HARBOR_PY" - <<PY
import json
from pathlib import Path
result_dirs = sorted(Path(r"$JOB_DIR/jobs").glob("*"), key=lambda p: p.stat().st_mtime)
n_errors = None
result_path = None
trial_error = None
if result_dirs:
    candidate = result_dirs[-1] / "result.json"
    if candidate.exists():
        result_path = str(candidate)
        payload = json.loads(candidate.read_text(encoding="utf-8"))
        n_errors = payload.get("stats", {}).get("n_errored_trials")
    exceptions = sorted(result_dirs[-1].glob("*/exception.txt"), key=lambda p: p.stat().st_mtime)
    if exceptions:
        trial_error = exceptions[-1].read_text(encoding="utf-8")[-2500:]
effective_rc = $RC
if n_errors:
    effective_rc = 1
print(f"AGENT_SMOKE_RC={effective_rc}")
if trial_error:
    print(trial_error)
raise SystemExit(effective_rc)
PY
