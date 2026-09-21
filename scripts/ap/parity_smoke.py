#!/usr/bin/env python3
"""Compare Apofasi neural smoke to the Python reference on one billing case.

Uses the same on-disk checkpoint layout resolved by scripts/ap/resolve_checkpoint.sh
(and the same hub bundle as `just laya`). Decision thresholds must agree; float
scores may differ slightly across Metal vs MPS.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import torch
from laya import Router

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "crates" / "apofasi"

STATE = (
    "Hi, we were billed twice for March. Please refund the duplicate today."
)

QUESTIONS = {
    "department": {
        "type": "choice",
        "instructions": "Which department should handle this request?",
        "criteria": {
            "billing": "invoices, payments, refunds",
            "technical": "bugs, outages, system errors",
            "sales": "pricing, new contracts",
        },
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


def _ref_answers() -> dict:
    router = Router(device=_device())
    result = router.predict(STATE, QUESTIONS)
    answers = result["answers"]
    out = {}
    for key, value in answers.items():
        if isinstance(value, dict):
            out[key] = value
        else:
            slim = {"type": getattr(value, "type", None)}
            for field in ("choice", "score", "noul", "confidence"):
                if hasattr(value, field):
                    slim[field] = getattr(value, field)
            out[key] = slim
    return out


def _apofasi_answers() -> dict:
    ckpt = os.environ.get("APOFASI_CHECKPOINT")
    if not ckpt:
        raise SystemExit("APOFASI_CHECKPOINT must be set (use `just ap-parity`)")
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
        "smoke",
        "--json",
        "--checkpoint",
        ckpt,
    ]
    env = os.environ.copy()
    proc = subprocess.run(
        cmd,
        cwd=str(CRATE),
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
    # Cargo may print build lines; take the last JSON object line.
    payload = None
    for line in proc.stdout.splitlines():
        line = line.strip()
        if line.startswith("{") and line.endswith("}"):
            payload = json.loads(line)
    if payload is None:
        raise SystemExit(f"no JSON smoke output:\n{proc.stdout}\n{proc.stderr}")
    return payload["answers"]


def main() -> int:
    ref = _ref_answers()
    ap = _apofasi_answers()
    failures: list[str] = []

    ref_dept = ref["department"].get("choice") if isinstance(ref["department"], dict) else None
    ap_dept = ap["department"]["choice"]
    if ref_dept != ap_dept:
        failures.append(f"department mismatch: ref={ref_dept!r} apofasi={ap_dept!r}")

    ref_noul = float(ref["refund_requested"].get("noul", 0.0))
    ap_noul = float(ap["refund_requested"]["noul"])
    if (ref_noul > 0.5) != (ap_noul > 0.5):
        failures.append(
            f"refund_requested threshold mismatch: ref={ref_noul:.4f} apofasi={ap_noul:.4f}"
        )
    if abs(ref_noul - ap_noul) > 0.35:
        failures.append(
            f"refund_requested score drift too large: ref={ref_noul:.4f} apofasi={ap_noul:.4f}"
        )

    report = {
        "checkpoint": os.environ.get("APOFASI_CHECKPOINT"),
        "ref": {
            "department": ref_dept,
            "refund_requested": ref_noul,
        },
        "apofasi": {
            "department": ap_dept,
            "refund_requested": ap_noul,
        },
        "failures": failures,
    }
    print(json.dumps(report, indent=2))
    if failures:
        print("PARITY FAIL", file=sys.stderr)
        return 1
    print("PARITY OK", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
