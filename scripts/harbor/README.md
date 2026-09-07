# A3S Code Harbor Adapter

Custom Harbor `BaseInstalledAgent` for **Terminal-Bench 4.0** evaluation of A3S Code.

**Full runbook (setup, smoke ladder, full eval, pitfalls):**
[`EVALUATION.md`](./EVALUATION.md)

## Quick start (WSL)

```bash
export PATH="$HOME/.local/bin:$PATH"
export PYTHONPATH="/mnt/d/code/a3s/scripts/harbor${PYTHONPATH:+:$PYTHONPATH}"

# Wheel (match your A3S Code release)
bash /mnt/d/code/a3s/scripts/harbor/download_wheel.sh 8.2.0

# Model + key from repo config (writes gitignored .env)
python3 /mnt/d/code/a3s/scripts/harbor/materialize_env_from_a3s_config.py

# Validation ladder
bash /mnt/d/code/a3s/scripts/tb40_env_smoke.sh
bash /mnt/d/code/a3s/scripts/tb40_a3s_install_only.sh
bash /mnt/d/code/a3s/scripts/tb40_a3s_agent_smoke.sh
```

Defer full TB 4.0 suites until the next A3S Code wheel is ready; see
[`EVALUATION.md`](./EVALUATION.md#resume-when-the-new-a3s-code-version-is-ready).
