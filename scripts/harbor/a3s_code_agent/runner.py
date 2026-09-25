#!/usr/bin/env python3
"""Headless A3S Code runner for Harbor Terminal-Bench trials."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import traceback
from pathlib import Path

_MUTATION_DIGEST_RE = re.compile(
    r"workspace mutation ([a-f0-9]{64})",
    re.IGNORECASE,
)


def _write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def _build_acl(model: str, provider: str, api_key_env: str, base_url: str | None = None) -> str:
    # Keep ACL minimal: one provider block + default model.
    # Canonical form matches crates/code/agent.example.acl (`providers`).
    lines = [
        f'default_model = "{model}"',
        "",
        f'providers "{provider}" {{',
        f'  api_key = env("{api_key_env}")',
    ]
    if base_url:
        lines.append(f'  base_url = "{base_url}"')
    lines.extend(
        [
            f'  models "{model.split("/", 1)[-1]}" {{}}',
            "}",
            "",
        ]
    )
    return "\n".join(lines)


def _host_verification_report(digest: str) -> dict:
    """Bind a host Passed report so Harbor can own acceptance via its verifier."""
    return {
        "schema": "a3s.verification_report.v1",
        "subject": "harbor:terminal-bench",
        "status": "passed",
        "effect_digest": digest,
        "checks": [
            {
                "id": "check:harbor-host",
                "kind": "host",
                "description": (
                    "Harbor TB host accepts this workspace mutation digest; "
                    "Harbor's native verifier owns task acceptance."
                ),
                "status": "passed",
                "required": True,
            }
        ],
    }


def _run_with_host_completion_binding(session, instruction: str) -> object:
    """Drive the session until completion, binding host reports for open mutations.

    After 8.5.6, unmarked workspace mutations cannot complete a turn. Harbor's
    verifier is the TB acceptance authority, so the host binds Passed reports to
    whatever mutation digests the completion gate names.
    """
    prompt = instruction
    bound: set[str] = set()
    last_error: Exception | None = None
    for _ in range(16):
        try:
            return session.send({"prompt": prompt})
        except Exception as exc:  # noqa: BLE001 — surface via result JSON
            last_error = exc
            message = str(exc)
            if "completion gate:" not in message:
                raise
            match = _MUTATION_DIGEST_RE.search(message)
            if match is None:
                raise
            digest = match.group(1)
            if digest in bound:
                raise RuntimeError(
                    f"completion gate still open after host verification for {digest}"
                ) from exc
            session.record_verification_reports([_host_verification_report(digest)])
            bound.add(digest)
            prompt = (
                "Continue the task. The host bound a Passed verification report "
                f"for mutation digest {digest}. Harbor's verifier will judge the "
                "final artifacts."
            )
    assert last_error is not None
    raise last_error


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--instruction-file", required=True)
    parser.add_argument("--model", required=True, help="provider/model")
    parser.add_argument("--acl-path", required=True)
    parser.add_argument("--log-dir", required=True)
    parser.add_argument("--api-key-env", default="")
    parser.add_argument("--base-url", default="")
    args = parser.parse_args()

    log_dir = Path(args.log_dir)
    log_dir.mkdir(parents=True, exist_ok=True)
    result_path = log_dir / "a3s-code-result.json"
    error_path = log_dir / "a3s-code-error.txt"
    print("a3s-code runner: starting", flush=True)

    try:
        if "/" not in args.model:
            raise ValueError("model must be provider/model")
        provider, _model_name = args.model.split("/", 1)
        api_key_env = args.api_key_env or {
            "anthropic": "ANTHROPIC_API_KEY",
            "openai": "OPENAI_API_KEY",
            "google": "GOOGLE_API_KEY",
            "gemini": "GEMINI_API_KEY",
            "deepseek": "DEEPSEEK_API_KEY",
            "openrouter": "OPENROUTER_API_KEY",
        }.get(provider, f"{provider.upper()}_API_KEY")
        base_url = args.base_url or os.environ.get(f"{provider.upper()}_BASE_URL") or None

        acl_path = Path(args.acl_path)
        acl_path.parent.mkdir(parents=True, exist_ok=True)
        acl_path.write_text(
            _build_acl(args.model, provider, api_key_env, base_url=base_url),
            encoding="utf-8",
        )
        print(f"a3s-code runner: acl ready model={args.model}", flush=True)

        instruction = Path(args.instruction_file).read_text(encoding="utf-8")

        from a3s_code import Agent, ConfirmationPolicy, PermissionPolicy, SessionOptions

        print("a3s-code runner: creating Agent", flush=True)
        agent = Agent.create(str(acl_path))
        opts = SessionOptions()
        # Unattended TB execution: allow tools and auto-approve Ask.
        opts.permission_policy = PermissionPolicy(default_decision="allow")
        opts.confirmation_policy = ConfirmationPolicy(enabled=False)

        print(f"a3s-code runner: opening session workspace={args.workspace}", flush=True)
        session = agent.session(args.workspace, opts)
        print("a3s-code runner: sending instruction", flush=True)
        result = _run_with_host_completion_binding(session, instruction)
        text = getattr(result, "text", None) or str(result)
        _write_json(
            result_path,
            {
                "ok": True,
                "model": args.model,
                "text_preview": text[:4000],
                "text_len": len(text),
            },
        )
        print(text)
        return 0
    except Exception as exc:
        error_path.write_text(
            "".join(traceback.format_exception(type(exc), exc, exc.__traceback__)),
            encoding="utf-8",
        )
        _write_json(
            result_path,
            {
                "ok": False,
                "error": str(exc),
                "error_type": type(exc).__name__,
            },
        )
        print(f"a3s-code runner failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
