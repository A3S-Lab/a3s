#!/usr/bin/env python3
"""Warm latency shootout: Apofasi (release CLI) vs Python reference Router.

Compares both the 2-question smoke case and the 4-question triage case.
Exit non-zero unless Apofasi wins p50 on both.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

import torch
from laya import Router

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "crates" / "apofasi"

SMOKE_STATE = "Hi, we were billed twice for March. Please refund the duplicate today."
SMOKE_QS = {
    "department": {
        "type": "choice",
        "instructions": "Which department should handle this request?",
        "criteria": {
            "billing": "invoices payments refunds",
            "technical": "bugs outages errors",
            "sales": "pricing contracts",
        },
    },
    "refund_requested": {
        "type": "noul",
        "instructions": "Does the user explicitly request a refund?",
    },
}

TRIAGE_STATE = {
    "from": "user@acme.com",
    "subject": "Duplicate charge on invoice #4411",
    "body": (
        "Hi, we were billed twice for March. Please refund the duplicate today "
        "or we will cancel our plan."
    ),
}
TRIAGE_QS = {
    "department": {
        "type": "choice",
        "instructions": "Which department should handle this request?",
        "criteria": {
            "billing": "invoices, payments, refunds",
            "technical": "bugs, outages, system errors",
            "sales": "pricing, new contracts",
            "other": "everything else",
        },
    },
    "urgency": {
        "type": "score",
        "instructions": "How urgent is this request?",
        "criteria": ["not urgent", "soon", "critical deadline or blocking issue"],
    },
    "churn_risk": {
        "type": "noul",
        "instructions": "Does the user threaten to cancel or leave?",
    },
    "refund_requested": {
        "type": "noul",
        "instructions": "Does the user explicitly request a refund?",
    },
}


def _device() -> str:
    if torch.cuda.is_available():
        return "cuda"
    if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def _laya_p50(state, questions, iters: int = 40, warmup: int = 12) -> float:
    router = Router(device=_device())
    for _ in range(warmup):
        router.predict(state, questions)
    times: list[float] = []
    for _ in range(iters):
        t0 = time.perf_counter()
        router.predict(state, questions)
        times.append((time.perf_counter() - t0) * 1000.0)
    times.sort()
    return times[len(times) // 2]


def _apofasi_p50(case: str, iters: int = 40, warmup: int = 12) -> float:
    ckpt = os.environ["APOFASI_CHECKPOINT"]
    features = "cli,metal,mlx" if sys.platform == "darwin" else "cli"
    cmd = [
        "cargo",
        "run",
        "--release",
        "--features",
        features,
        "--bin",
        "a3s-apofasi",
        "--",
        "bench",
        "--checkpoint",
        ckpt,
        "--case",
        case,
        "--iters",
        str(iters),
        "--warmup",
        str(warmup),
    ]
    proc = subprocess.run(cmd, cwd=str(CRATE), check=True, capture_output=True, text=True)
    text = proc.stdout + "\n" + proc.stderr
    m = re.search(r"p50_ms=([0-9.]+)", text)
    if not m:
        raise SystemExit(f"missing p50 in apofasi output:\n{text}")
    return float(m.group(1))


def main() -> int:
    if "APOFASI_CHECKPOINT" not in os.environ:
        raise SystemExit("APOFASI_CHECKPOINT must be set")

    cases = [
        ("smoke", SMOKE_STATE, SMOKE_QS),
        ("triage", TRIAGE_STATE, TRIAGE_QS),
    ]
    report: dict = {"checkpoint": os.environ["APOFASI_CHECKPOINT"], "cases": {}}
    failures: list[str] = []
    for name, state, qs in cases:
        ap = _apofasi_p50(name)
        ref = _laya_p50(state, qs)
        faster = ap < ref
        report["cases"][name] = {
            "apofasi_p50_ms": ap,
            "laya_p50_ms": ref,
            "speedup": round(ref / ap, 3) if ap > 0 else None,
            "apofasi_faster": faster,
        }
        if not faster:
            failures.append(f"{name}: apofasi={ap:.2f}ms laya={ref:.2f}ms")

    print(json.dumps(report, indent=2))
    if failures:
        print("FAIL: " + "; ".join(failures), file=sys.stderr)
        return 1
    print("OK: Apofasi faster on smoke and triage", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
