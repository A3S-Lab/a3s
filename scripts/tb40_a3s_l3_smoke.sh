#!/usr/bin/env bash
# Limited 3-task Harbor advancement (excludes multi-hour layout task).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.local/bin:$PATH"
export PYTHONPATH="$REPO_ROOT/scripts/harbor${PYTHONPATH:+:$PYTHONPATH}"
JOB_DIR="${A3S_HARBOR_JOBS_DIR:-$REPO_ROOT/.harbor-smoke}"
ENV_FILE="$REPO_ROOT/scripts/harbor/.env"

python3 "$REPO_ROOT/scripts/harbor/materialize_env_from_a3s_config.py"

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

MODEL="${A3S_TB_MODEL:-boyue/deepseek-v4-flash}"
echo "using_model=$MODEL"

harbor_env_args=()
for k in OPENAI_API_KEY ANTHROPIC_API_KEY DEEPSEEK_API_KEY OPENROUTER_API_KEY BOYUE_API_KEY \
  DEEPSEEK_BASE_URL BOYUE_BASE_URL; do
  if [[ -n "${!k:-}" ]]; then
    harbor_env_args+=(--ae "${k}=${!k}")
  fi
done

mkdir -p "$JOB_DIR"
cd "$JOB_DIR"

set +e
harbor run \
  -d "terminal-bench/terminal-bench@4.0.0" \
  -a "a3s_code_agent.agent:A3sCodeAgent" \
  -m "$MODEL" \
  -e docker \
  -n 1 \
  -l 3 \
  -k 1 \
  --agent-setup-timeout-multiplier 3 \
  -x "terminal-bench/layout-config-recreation2" \
  "${harbor_env_args[@]}" \
  --jobs-dir "$JOB_DIR/jobs-l3" 2>&1 | tee "$JOB_DIR/a3s-l3-smoke.log"
RC=${PIPESTATUS[0]}
set -e
echo "L3_RC=$RC"
exit "$RC"
