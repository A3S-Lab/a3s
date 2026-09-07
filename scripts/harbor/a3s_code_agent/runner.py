#!/usr/bin/env python3
"""Headless A3S Code runner for Harbor Terminal-Bench trials."""

from __future__ import annotations

import argparse
import json
import os
import sys
import traceback
from pathlib import Path


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

        instruction = Path(args.instruction_file).read_text(encoding="utf-8")

        from a3s_code import Agent, ConfirmationPolicy, PermissionPolicy, SessionOptions

        agent = Agent.create(str(acl_path))
        opts = SessionOptions()
        # Unattended TB execution: allow tools and auto-approve Ask.
        opts.permission_policy = PermissionPolicy(default_decision="allow")
        opts.confirmation_policy = ConfirmationPolicy(enabled=False)

        session = agent.session(args.workspace, opts)
        result = session.send({"prompt": instruction})
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
