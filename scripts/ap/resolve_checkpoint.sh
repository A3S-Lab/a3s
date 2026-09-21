#!/usr/bin/env bash
# Resolve an Apofasi-compatible System-1 checkpoint directory.
#
# Preference order:
#   1. APOFASI_CHECKPOINT (if it contains the published layout)
#   2. First HF hub snapshot under HF_HOME/hub with that layout
#   3. Download the reference hub bundle (same weights used by `just laya`)
#
# Prints the absolute checkpoint path on stdout.
# Brand-sensitive names stay in monorepo scripts only — not in crates/apofasi.

set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
hub_home="${HF_HOME:-${HOME}/.cache/huggingface}"
hub_dir="${hub_home}/hub"
# Reference bundle that publishes the Apofasi on-disk contract.
REF_REPO="${APOFASI_REF_REPO:-convaiinnovations/laya}"

is_checkpoint() {
  local dir="$1"
  [[ -f "${dir}/rl_agent_config.json" \
    && -f "${dir}/model.safetensors" \
    && -f "${dir}/tokenizer/tokenizer.json" \
    && -f "${dir}/encoder/config.json" ]]
}

if [[ -n "${APOFASI_CHECKPOINT:-}" ]]; then
  if is_checkpoint "${APOFASI_CHECKPOINT}"; then
    echo "${APOFASI_CHECKPOINT}"
    exit 0
  fi
  echo "APOFASI_CHECKPOINT is set but incomplete: ${APOFASI_CHECKPOINT}" >&2
  exit 1
fi

# Prefer an already-cached snapshot with the english root layout.
if [[ -d "${hub_dir}" ]]; then
  # shellcheck disable=SC2016
  found="$(
    find "${hub_dir}" -path '*/snapshots/*/rl_agent_config.json' 2>/dev/null \
      | while read -r cfg; do
          snap="$(dirname "${cfg}")"
          if is_checkpoint "${snap}"; then
            echo "${snap}"
            break
          fi
        done
  )"
  if [[ -n "${found}" ]]; then
    echo "${found}"
    exit 0
  fi
fi

echo "No local checkpoint found; downloading ${REF_REPO} into ${hub_dir}..." >&2
py="${root}/.scratch/laya-venv/bin/python"
if [[ ! -x "${py}" ]]; then
  echo "Creating helper venv at ${root}/.scratch/laya-venv" >&2
  uv venv -p 3.12 "${root}/.scratch/laya-venv"
  uv pip install --python "${py}" "huggingface_hub>=0.20"
fi
export APOFASI_REF_REPO="${REF_REPO}"
"${py}" - <<'PY'
from huggingface_hub import snapshot_download
import os
path = snapshot_download(os.environ["APOFASI_REF_REPO"])
print(path)
PY
