# Terminal-Bench 4.0 evaluation with A3S Code

Canonical runbook for evaluating **A3S Code** on **Terminal-Bench 4.0** via
**Harbor**. Keep this file updated when the adapter, wheel packaging, or Harbor
flags change.

> **Status:** Environment and adapter are ready. Full leaderboard-style runs
> should wait until the next A3S Code release wheel is available. Use smoke
> scripts only to validate install/config until then.

## Overview

| Piece | Role |
| --- | --- |
| Harbor | Official TB 4.0 harness (`harbor run -d terminal-bench/terminal-bench@4.0.0`) |
| `a3s_code_agent` | Custom `BaseInstalledAgent` that installs A3S Code in the task container and runs headlessly |
| Prefetched manylinux wheel | Avoids GitHub DNS failures inside task containers |
| `.a3s/config.acl` → `scripts/harbor/.env` | Model + API key source for Harbor (gitignored `.env`) |

A3S Code is **not** a built-in Harbor agent. Always pass:

```text
-a a3s_code_agent.agent:A3sCodeAgent
```

and set `PYTHONPATH` to `scripts/harbor` so Harbor can import the adapter.

## Host requirements

Run everything in **WSL2 Ubuntu** (not the Windows Docker host):

- Python ≥ 3.12 (for host tooling)
- [`uv`](https://github.com/astral-sh/uv)
- Harbor with Docker support: `uv tool install 'harbor[modal]'`
- Docker Engine working inside WSL (`docker ps` succeeds)
- NVIDIA GPU optional (some TB tasks need GPU; many do not)
- Network access to the model provider (for example DeepSeek)

Recommended PATH:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Persist that in `~/.bashrc` / `~/.profile` if needed.

## Layout

```text
scripts/
├── tb40_env_smoke.sh              # Host tools: python/uv/harbor/docker/gpu
├── tb40_a3s_install_only.sh       # Harbor --install-only (no model call)
├── tb40_a3s_agent_smoke.sh        # 1-task agent smoke with model
└── harbor/
    ├── README.md                  # Short index → this runbook
    ├── EVALUATION.md              # This file
    ├── .env.example
    ├── .env                       # gitignored; materialized from .a3s/config.acl
    ├── materialize_env_from_a3s_config.py
    ├── download_wheel.sh
    ├── wheels/                    # gitignored *.whl
    └── a3s_code_agent/
        ├── agent.py               # Harbor BaseInstalledAgent
        └── runner.py              # Headless a3s_code session (allow-all)

.harbor-smoke/jobs/                # Smoke job outputs (local)
```

## One-time setup

### 1. Install host tools (WSL)

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
# restart shell or: source "$HOME/.local/bin/env"
uv tool install 'harbor[modal]'
harbor --version
docker ps
```

### 2. Prefetch the A3S Code manylinux wheel

Task containers often cannot resolve GitHub. Download on the host and let the
adapter upload the wheel:

```bash
bash /mnt/d/code/a3s/scripts/harbor/download_wheel.sh 8.2.0
# → scripts/harbor/wheels/a3s_code-8.2.0-cp310-abi3-manylinux_2_28_x86_64.whl
```

When a **new A3S Code version** ships, download that version (or place the
matching `cp310-abi3-manylinux_2_28_x86_64` wheel into `scripts/harbor/wheels/`)
before full evaluation.

### 3. Materialize model credentials from `.a3s/config.acl`

```bash
python3 /mnt/d/code/a3s/scripts/harbor/materialize_env_from_a3s_config.py
```

Writes gitignored `scripts/harbor/.env`:

- `DEEPSEEK_API_KEY`
- `DEEPSEEK_BASE_URL`
- `A3S_TB_MODEL` (from `default_model`)

Never commit `.env` or paste keys into git-tracked files. Manual template:
`scripts/harbor/.env.example`.

Load for a shell session:

```bash
set -a
source /mnt/d/code/a3s/scripts/harbor/.env
set +a
```

Harbor discovers `DEEPSEEK_API_KEY` / `DEEPSEEK_BASE_URL` for model
`deepseek/<name>` via its provider registry (`passthrough` on the adapter).

## Adapter behavior (method)

1. **Install** (agent setup, timed; default Harbor setup budget ~360s, smoke uses multiplier 3):
   - Ensure light system deps (`curl`, `ca_certificates`) — avoid `build-essential` when using a binary wheel
   - Upload prefetched manylinux wheel into `/tmp/a3s-code-harbor/wheels/`
   - `uv` + Python 3.12 venv + `uv pip install` the wheel
   - Upload `runner.py`
2. **Run**:
   - Build a minimal ACL (`providers "<provider>" { api_key = env(...); base_url? }`)
   - `PermissionPolicy(default_decision="allow")` + `ConfirmationPolicy(enabled=False)` for unattended TB
   - Execute `session.send({prompt: instruction})` in `/app`
   - Stream logs to `/logs/agent/a3s-code.txt` and write `a3s-code-result.json`

Important implementation notes:

- Do **not** use `$HOME/...` paths for Harbor `upload_file` sources; use absolute host paths under `/mnt/d/code/a3s/...` or `/tmp/...`
- Prefer a host-downloaded manylinux wheel over `pip install` from GitHub inside the container
- Install into a **venv** (`uv venv`); do not `uv pip install` into uv-managed CPython

## Validation ladder (before full eval)

Always run from WSL with repo paths under `/mnt/d/code/a3s`.

### A. Environment smoke

```bash
bash /mnt/d/code/a3s/scripts/tb40_env_smoke.sh
```

Checks: `python3`, `uv`, `harbor`, `docker`, optional GPU.

### B. Install-only smoke (no API key required)

```bash
bash /mnt/d/code/a3s/scripts/tb40_a3s_install_only.sh
```

Confirms Harbor can import `A3sCodeAgent` and install the wheel inside a TB task
container (`--install-only`).

### C. Agent smoke (API key required)

```bash
python3 /mnt/d/code/a3s/scripts/harbor/materialize_env_from_a3s_config.py
bash /mnt/d/code/a3s/scripts/tb40_a3s_agent_smoke.sh
# optional model override:
# bash /mnt/d/code/a3s/scripts/tb40_a3s_agent_smoke.sh deepseek/deepseek-v4-flash
```

Runs **1 task × 1 trial**. Harbor may pick a hard task (for example
`layout-config-recreation2` with a multi-hour agent timeout). That is enough to
prove install + model wiring; it is **not** a substitute for a planned full
suite.

Stop a runaway smoke:

```bash
pkill -f 'harbor run -d terminal-bench/terminal-bench@4.0.0' || true
docker ps --format '{{.Names}}' | grep -E 'terminal-bench|layout-config' | xargs -r docker stop
```

## Full evaluation (after new A3S Code release)

### Checklist

1. Place the new manylinux wheel in `scripts/harbor/wheels/` (or `download_wheel.sh <version>`)
2. Refresh `.env` if `.a3s/config.acl` changed
3. Re-run install-only smoke once
4. Launch the Harbor job with explicit task filters / trial counts

### Example: leaderboard-style style parameters

Adjust concurrency and cost deliberately. Full TB 4.0 with high `-k` is
expensive and long-running.

```bash
export PATH="$HOME/.local/bin:$PATH"
export PYTHONPATH="/mnt/d/code/a3s/scripts/harbor${PYTHONPATH:+:$PYTHONPATH}"
set -a; source /mnt/d/code/a3s/scripts/harbor/.env; set +a

JOBS="/mnt/d/code/a3s/.harbor-eval/jobs"
mkdir -p "$JOBS"

harbor run \
  -d "terminal-bench/terminal-bench@4.0.0" \
  -a "a3s_code_agent.agent:A3sCodeAgent" \
  -m "${A3S_TB_MODEL}" \
  -e docker \
  -n 4 \
  -k 5 \
  --agent-setup-timeout-multiplier 3 \
  --jobs-dir "$JOBS"
```

Useful filters:

| Flag | Purpose |
| --- | --- |
| `-l N` / `--n-tasks N` | Cap number of tasks |
| `-i NAME` / `--include-task-name` | Include specific task(s) |
| `-x NAME` / `--exclude-task-name` | Exclude task(s) |
| `-k N` | Trials per task (leaderboard-style often uses 5) |
| `-n N` | Concurrent trials |
| `--agent-setup-timeout-multiplier` | Raise when install is slow on cold images |
| `--install-only` | Setup/install only |

Inspect results:

```bash
harbor view /mnt/d/code/a3s/.harbor-eval/jobs
# or open the job folder's result.json
```

## Jobs and artifacts

| Path | Contents |
| --- | --- |
| `.harbor-smoke/jobs/<timestamp>/` | Smoke job `result.json`, `job.log`, per-trial dirs |
| `<trial>/exception.txt` | Harbor/agent setup failures |
| `<trial>/trial.log` | Executed setup/run commands |
| Container `/logs/agent/a3s-code.txt` | Agent stdout stream |
| Container `/logs/agent/a3s-code-result.json` | Runner success/failure payload |
| Container `/tmp/a3s-code-harbor/agent.acl` | Generated ACL for the trial |

## Known pitfalls

1. **Windows Docker ≠ WSL Docker** — run Harbor inside Ubuntu WSL.
2. **AgentSetupTimeoutError (~360s)** — usually `apt-get` installing heavy
   build tools; adapter should stay on light deps + prefetched wheel; raise
   `--agent-setup-timeout-multiplier` if needed.
3. **GitHub DNS inside containers** — always prefetch the manylinux wheel.
4. **Missing API key** — install-only works; full agent runs need provider env
   vars (materialize from `.a3s/config.acl`).
5. **PowerShell `$VAR` expansion** — when invoking WSL from PowerShell, put
   bash logic in `.sh` files; do not rely on `$FOO` inside `wsl bash -c "..."`
   strings from PowerShell.
6. **Hard random smoke tasks** — a 1-task smoke may burn hours; stop it after
   install + first tool activity if you only needed wiring proof.

## Resume when the new A3S Code version is ready

```bash
# 1) New wheel
bash /mnt/d/code/a3s/scripts/harbor/download_wheel.sh <NEW_VERSION>

# 2) Model env
python3 /mnt/d/code/a3s/scripts/harbor/materialize_env_from_a3s_config.py

# 3) Quick gates
bash /mnt/d/code/a3s/scripts/tb40_env_smoke.sh
bash /mnt/d/code/a3s/scripts/tb40_a3s_install_only.sh
bash /mnt/d/code/a3s/scripts/tb40_a3s_agent_smoke.sh   # optional short wiring check

# 4) Full Harbor run (see "Full evaluation" above)
```
