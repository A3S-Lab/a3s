#!/usr/bin/env python3
"""Materialize scripts/harbor/.env from .a3s/config.acl without printing secrets."""

from __future__ import annotations

import os
import re
from pathlib import Path


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def main() -> int:
    root = _repo_root()
    src_path = root / ".a3s" / "config.acl"
    if not src_path.exists():
        print(f"ERROR: missing {src_path}")
        return 1

    src = src_path.read_text(encoding="utf-8")
    m_key = re.search(r'api_key\s*=\s*"([^"]+)"', src)
    m_base = re.search(r'base_url\s*=\s*"([^"]+)"', src)
    m_model = re.search(r'default_model\s*=\s*"([^"]+)"', src)
    key = m_key.group(1) if m_key else ""
    base = m_base.group(1) if m_base else "https://api.deepseek.com"
    model = m_model.group(1) if m_model else "deepseek/deepseek-v4-pro"
    if not key:
        print("ERROR: no api_key in .a3s/config.acl")
        return 1

    env_path = root / "scripts" / "harbor" / ".env"
    env_path.write_text(
        "# Generated from .a3s/config.acl — do not commit\n"
        f"DEEPSEEK_API_KEY={key}\n"
        f"DEEPSEEK_BASE_URL={base}\n"
        f"A3S_TB_MODEL={model}\n",
        encoding="utf-8",
    )
    try:
        os.chmod(env_path, 0o600)
    except OSError:
        pass
    print(f"wrote={env_path}")
    print(f"model={model}")
    print(f"base_url={base}")
    print(f"key_set={bool(key)}")
    print(f"key_len={len(key)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
