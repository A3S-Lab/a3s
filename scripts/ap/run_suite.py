#!/usr/bin/env python3
"""Run the reference decision suite through Apofasi.

Case inputs and pass/fail checks come from `scripts/laya/run_suite.py`, so
`just ap` and `just laya` see the same conditions. `--compare` times both
resident routers over repeated samples (p50) and prints the published TypeSafe
Jev figures from the Laya repository. Those Jev numbers were not measured here.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
LAYA_SUITE = ROOT / "scripts" / "laya" / "run_suite.py"
DEFAULT_LOG_DIR = ROOT / ".scratch" / "ap" / "logs"
COMPARE_WARMUP = 12
COMPARE_ITERS = 40

# Headline and typed-decisions rows from the Laya repo. Jev 1.13.0 is
# third-party published; the repo states those figures were never measured
# there. https://github.com/NandhaKishorM/laya/blob/main/BENCHMARKS.md
JEV_SOURCE = "https://github.com/NandhaKishorM/laya/blob/main/BENCHMARKS.md"
JEV_P50_1Q_MS = (236.0, 276.0)
LAYA_T4_P50_1Q_MS = 32.8
JEV_PUBLISHED_ROWS = (
    ("typed-decisions accuracy (2,000)", "0.766", "0.727"),
    ("typed-decisions soft accuracy", "0.471", "0.580"),
    ("typed-decisions Brier (lower better)", "0.061", "0.148"),
    ("typed-decisions ECE as shipped (lower better)", "0.213", "0.144"),
    ("typed-decisions score MAE (lower better)", "0.242", "0.391"),
    ("AG News (4 labels)", "0.953", "0.910"),
    ("DAIR Emotion (6 labels)", "0.600", "0.480"),
    ("Banking77 (Laya base 77 labels)", "0.425", "0.870"),
    ("ECE after temperature fitting (lower better)", "0.081", "0.246"),
    ("p50 latency, 1 question", "32.8 ms on T4", "236–276 ms"),
)


def _load_reference_suite():
    spec = importlib.util.spec_from_file_location("laya_reference_suite", LAYA_SUITE)
    if spec is None or spec.loader is None:
        raise SystemExit(f"cannot load {LAYA_SUITE}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _state_text(state: Any) -> Any:
    """Match the reference `serialize_state`: strings pass through, else JSON."""
    if isinstance(state, str):
        return state
    return json.dumps(state, ensure_ascii=False)


def _select(module, names: list[str] | None):
    selected = module.CASES
    if names:
        wanted = set(names)
        selected = [case for case in module.CASES if case.name in wanted]
        missing = wanted - {case.name for case in selected}
        if missing:
            print(f"Unknown case(s): {sorted(missing)}", file=sys.stderr)
            print(
                "Available:",
                ", ".join(case.name for case in module.CASES),
                file=sys.stderr,
            )
            raise SystemExit(2)
    return selected


def _case_payload(case) -> dict[str, Any]:
    state, questions, predict_kw = case.build(None)
    model = predict_kw.get("model")
    return {
        "name": case.name,
        "description": case.description,
        "state": _state_text(state),
        "questions": questions,
        "model": model,
        "predict_kwargs": predict_kw,
    }


def _p50(samples: list[float]) -> float:
    ordered = sorted(samples)
    return ordered[len(ordered) // 2]


def _run_apofasi(
    binary: Path,
    checkpoint: str,
    device: str | None,
    payloads: list[dict],
    warmup: int,
    iters: int,
) -> list[dict]:
    cases = [
        {
            "name": item["name"],
            "state": item["state"],
            "questions": item["questions"],
            "model": item["model"],
        }
        for item in payloads
    ]
    path = DEFAULT_LOG_DIR / "_suite_cases.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"cases": cases}, ensure_ascii=False), encoding="utf-8")
    cmd = [
        str(binary),
        "suite",
        "--cases",
        str(path),
        "--checkpoint",
        checkpoint,
        "--warmup",
        str(warmup),
        "--iters",
        str(iters),
    ]
    if device:
        cmd.extend(["--device", device])
    proc = subprocess.run(cmd, check=False, capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(proc.stderr.strip() or proc.stdout.strip() or "apofasi suite failed")
    try:
        rows = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"apofasi suite returned non-JSON:\n{proc.stdout}\n{exc}") from exc
    by_name = {row["name"]: row for row in rows}
    missing = [item["name"] for item in payloads if item["name"] not in by_name]
    if missing:
        raise SystemExit(f"apofasi suite missing cases: {missing}")
    return [by_name[item["name"]] for item in payloads]


def _reference_rows(
    module, payloads: list[dict], device: str | None, warmup: int, iters: int
) -> list[dict]:
    router = module.Router(device=device or module._device(), max_loaded=3)
    needed = {"english", "multilingual", "typed-decisions"}
    router.preload(sorted(needed))
    rows = []
    for item in payloads:
        state, questions, predict_kw = item["raw_state"], item["questions"], item["predict_kwargs"]
        for _ in range(warmup):
            router.predict(state, questions, **predict_kw)
        samples: list[float] = []
        result = None
        for _ in range(iters):
            t0 = time.perf_counter()
            result = router.predict(state, questions, **predict_kw)
            samples.append((time.perf_counter() - t0) * 1000.0)
        if result is None:
            raise SystemExit(f"{item['name']}: reference produced no samples")
        p50 = _p50(samples)
        rows.append(
            {
                "name": item["name"],
                "latency_ms": round(p50, 2),
                "p50_ms": round(p50, 2),
                "min_ms": round(min(samples), 2),
                "max_ms": round(max(samples), 2),
                "warmup": warmup,
                "iters": iters,
                "routing": {"model": (result.get("routing") or {}).get("model")},
                "answers": module._compact_answers(result.get("answers", {})),
                "result": result,
            }
        )
    return rows


def _checked(case, row: dict) -> list[str]:
    result = {
        "routing": row.get("routing") or {},
        "answers": row.get("answers") or {},
        "usage": row.get("usage"),
    }
    return list(case.check(result, float(row["latency_ms"])))


def _log(message: str = "") -> None:
    print(message, flush=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run the reference suite through Apofasi")
    parser.add_argument("--bin", type=Path, required=True, help="Path to a3s-apofasi")
    parser.add_argument("--case", action="append", dest="cases")
    parser.add_argument("--device", default=None, help="auto|metal|cuda|cpu for Apofasi; mps/cpu/cuda for the reference")
    parser.add_argument("--log-dir", type=Path, default=DEFAULT_LOG_DIR)
    parser.add_argument(
        "--compare",
        action="store_true",
        help="Also run the reference router, print per-case p50, and the published Jev table",
    )
    parser.add_argument("--warmup", type=int, default=None)
    parser.add_argument("--iters", type=int, default=None)
    args = parser.parse_args(argv)
    warmup = args.warmup if args.warmup is not None else (COMPARE_WARMUP if args.compare else 1)
    iters = args.iters if args.iters is not None else (COMPARE_ITERS if args.compare else 1)
    if warmup < 1 or iters < 1:
        raise SystemExit("--warmup and --iters must be positive")

    checkpoint = __import__("os").environ.get("APOFASI_CHECKPOINT")
    if not checkpoint:
        raise SystemExit("APOFASI_CHECKPOINT must be set")
    if not args.bin.is_file():
        raise SystemExit(f"missing binary {args.bin}")

    module = _load_reference_suite()
    selected = _select(module, args.cases)
    payloads = []
    for case in selected:
        item = _case_payload(case)
        state, questions, predict_kw = case.build(None)
        item["raw_state"] = state
        item["questions"] = questions
        item["predict_kwargs"] = predict_kw
        item["case"] = case
        payloads.append(item)

    ap_rows = _run_apofasi(args.bin, checkpoint, args.device, payloads, warmup, iters)
    ref_rows = (
        _reference_rows(module, payloads, _reference_device(args.device), warmup, iters)
        if args.compare
        else None
    )

    args.log_dir.mkdir(parents=True, exist_ok=True)
    failures_total = 0
    for index, item in enumerate(payloads):
        case = item["case"]
        row = ap_rows[index]
        failed = _checked(case, row)
        failures_total += 0 if not failed else 1
        _log(f"\n{'=' * 72}")
        _log(f"CASE: {case.name}")
        _log(f"DESC: {case.description}")
        _log("INPUT:")
        _log(
            json.dumps(
                {
                    "state": item["raw_state"],
                    "questions": item["questions"],
                    "predict_kwargs": item["predict_kwargs"],
                },
                ensure_ascii=False,
                indent=2,
                default=str,
            )
        )
        _log("OUTPUT:")
        _log(
            json.dumps(
                {
                    "routing": row.get("routing"),
                    "answers": row.get("answers"),
                    "usage": row.get("usage"),
                    "latency_ms": row.get("latency_ms"),
                    "p50_ms": row.get("p50_ms", row.get("latency_ms")),
                    "min_ms": row.get("min_ms"),
                    "max_ms": row.get("max_ms"),
                    "warmup": row.get("warmup", warmup),
                    "iters": row.get("iters", iters),
                    "checks_failed": failed,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        _log(f"RESULT: {'PASS' if not failed else 'FAIL'}")
        for failure in failed:
            _log(f"  - {failure}")

    passed = len(payloads) - failures_total
    _log(f"\n{'=' * 72}")
    _log(f"SUMMARY: {passed} passed, {failures_total} failed, {len(payloads)} total")
    for index, item in enumerate(payloads):
        row = ap_rows[index]
        failed = _checked(item["case"], row)
        mark = "PASS" if not failed else "FAIL"
        _log(f"  [{mark}] {item['name']} (p50 {float(row['latency_ms']):.1f} ms, n={iters})")

    if ref_rows is not None:
        _log(f"\n{'=' * 72}")
        _log(
            f"COMPARE (same cases, resident routers, warmup={warmup}, iters={iters}, p50)"
        )
        _log(f"{'case':<28} {'ap_p50':>8} {'laya_p50':>8} {'speedup':>8}  result")
        one_question = None
        for index, item in enumerate(payloads):
            ap = ap_rows[index]
            ref = ref_rows[index]
            ap_fail = _checked(item["case"], ap)
            ref_result = {
                "routing": ref["routing"],
                "answers": module._compact_answers(ref["result"].get("answers", {})),
            }
            ref_fail = list(item["case"].check(ref_result, float(ref["latency_ms"])))
            ap_ms = float(ap["latency_ms"])
            ref_ms = float(ref["latency_ms"])
            speedup = ref_ms / ap_ms if ap_ms > 0 else float("inf")
            both = "match" if not ap_fail and not ref_fail else (
                "ap-fail" if ap_fail else "ref-fail" if ref_fail else "differ"
            )
            if ap_fail and ref_fail:
                both = "both-fail"
            if item["name"] == "latency_budget_mps":
                one_question = (ap_ms, ref_ms)
            _log(
                f"{item['name']:<28} {ap_ms:8.2f} {ref_ms:8.2f} {speedup:7.2f}x  {both}"
            )
        _log_published_jev(one_question)

    return 0 if failures_total == 0 else 1


def _log_published_jev(one_question: tuple[float, float] | None) -> None:
    low, high = JEV_P50_1Q_MS
    _log(f"\n{'=' * 72}")
    _log("PUBLISHED JEV (not measured on this machine)")
    _log(f"Source: {JEV_SOURCE}")
    _log(
        "TypeSafe Jev 1.13.0 figures are third-party published. The Laya repo "
        "did not run them (no TypeSafe API). Sample sizes and prompts differ."
    )
    _log(f"{'metric':<48} {'laya':>16} {'jev':>16}")
    for metric, laya, jev in JEV_PUBLISHED_ROWS:
        _log(f"{metric:<48} {laya:>16} {jev:>16}")
    if one_question is None:
        return
    ap_ms, laya_ms = one_question
    _log("")
    _log(
        "Local 1-question p50 is latency_budget_mps on this machine. "
        "Jev 236–276 ms and Laya 32.8 ms are the published 1-question figures "
        "(Jev: third-party API p50; Laya: Tesla T4). Do not treat them as the same bench."
    )
    _log(
        f"  local apofasi p50 {ap_ms:.2f} ms | local laya p50 {laya_ms:.2f} ms | "
        f"published jev {low:.0f}–{high:.0f} ms | published laya T4 {LAYA_T4_P50_1Q_MS:.1f} ms"
    )


def _reference_device(device: str | None) -> str | None:
    if device in (None, "auto"):
        return None
    if device == "metal":
        return "mps"
    return device


if __name__ == "__main__":
    raise SystemExit(main())
