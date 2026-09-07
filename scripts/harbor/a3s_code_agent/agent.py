"""Harbor BaseInstalledAgent adapter for A3S Code."""

from __future__ import annotations

import json
import shlex
from pathlib import Path
from typing import Any, override

from harbor.agents.installed.base import BaseInstalledAgent, with_prompt_template
from harbor.agents.model_connection import ModelConnectionSpec
from harbor.environments.base import BaseEnvironment
from harbor.models.agent.context import AgentContext
from harbor.models.trial.paths import EnvironmentPaths

_PACKAGE_DIR = Path(__file__).resolve().parent
_HARBOR_DIR = _PACKAGE_DIR.parent
_WHEELS_DIR = _HARBOR_DIR / "wheels"
_RUNNER_HOST_PATH = _PACKAGE_DIR / "runner.py"
_CONTAINER_HOME = "/tmp/a3s-code-harbor"
_CONTAINER_RUNNER = f"{_CONTAINER_HOME}/runner.py"
_CONTAINER_ACL = f"{_CONTAINER_HOME}/agent.acl"
_CONTAINER_WHEEL_DIR = f"{_CONTAINER_HOME}/wheels"
_CONTAINER_INSTRUCTION = f"{EnvironmentPaths.agent_dir}/instruction.txt"
_DEFAULT_WHEEL_VERSION = "8.2.0"


def _resolve_host_wheel(version: str | None) -> Path | None:
    preferred = version or _DEFAULT_WHEEL_VERSION
    exact = (
        _WHEELS_DIR
        / f"a3s_code-{preferred}-cp310-abi3-manylinux_2_28_x86_64.whl"
    )
    if exact.exists():
        return exact
    matches = sorted(_WHEELS_DIR.glob("a3s_code-*-cp310-abi3-manylinux_2_28_x86_64.whl"))
    return matches[-1] if matches else None


class A3sCodeAgent(BaseInstalledAgent):
    """Install a3s-code into the task container and run headlessly."""

    MODEL_CONNECTION = ModelConnectionSpec(passthrough=True)

    def __init__(self, *args: Any, version: str | None = None, **kwargs: Any) -> None:
        super().__init__(*args, **kwargs)
        self._version = version

    @staticmethod
    @override
    def name() -> str:
        return "a3s-code"

    @override
    def version(self) -> str | None:
        return self._version

    @override
    def get_version_command(self) -> str | None:
        return (
            'export PATH="$HOME/.local/bin:$PATH"; '
            f'"{_CONTAINER_HOME}/venv/bin/python" -c '
            "\"import a3s_code; print(getattr(a3s_code, '__version__', 'unknown'))\""
        )

    @override
    def parse_version(self, stdout: str) -> str:
        return stdout.strip() or "unknown"

    @override
    async def install(self, environment: BaseEnvironment) -> None:
        # Prefer a prebuilt manylinux wheel: skip build-essential/git to keep
        # agent setup under Harbor's default 360s agent-setup timeout.
        await self.ensure_system_dependencies(
            environment,
            ("curl", "ca_certificates"),
        )

        version_spec = f"=={self._version}" if self._version else ""
        venv_dir = f"{_CONTAINER_HOME}/venv"
        host_wheel = _resolve_host_wheel(self._version)

        await self.exec_as_agent(
            environment,
            command=(
                "set -euo pipefail; "
                f'mkdir -p "{_CONTAINER_HOME}" "{_CONTAINER_WHEEL_DIR}" "$HOME/.local/bin"'
            ),
        )

        if host_wheel is not None:
            remote_wheel = f"{_CONTAINER_WHEEL_DIR}/{host_wheel.name}"
            await environment.upload_file(host_wheel, remote_wheel)
            pip_target = shlex.quote(remote_wheel)
        else:
            pip_target = shlex.quote(f"a3s-code{version_spec}")

        install_cmd = (
            "set -euo pipefail; "
            "if ! command -v uv >/dev/null 2>&1; then "
            "curl -LsSf https://astral.sh/uv/install.sh | sh; "
            "fi; "
            'if [ -f "$HOME/.local/bin/env" ]; then . "$HOME/.local/bin/env"; fi; '
            'export PATH="$HOME/.local/bin:$PATH"; '
            "uv python install 3.12; "
            f'uv venv "{venv_dir}" --python 3.12; '
            f'UV_PYTHON="{venv_dir}/bin/python"; '
            f"uv pip install --python \"$UV_PYTHON\" {pip_target}; "
            # Native wheel should import without GitHub bootstrap download.
            "\"$UV_PYTHON\" -c 'import a3s_code; print(\"a3s_code_import_ok\", getattr(a3s_code, \"__version__\", \"unknown\"))'; "
            'ln -sfn \"$UV_PYTHON\" \"$HOME/.local/bin/a3s-code-python\"'
        )

        await self.exec_as_agent(environment, command=install_cmd)
        await environment.upload_file(_RUNNER_HOST_PATH, _CONTAINER_RUNNER)

    @override
    def populate_context_post_run(self, context: AgentContext) -> None:
        result_path = self.logs_dir / "a3s-code-result.json"
        if not result_path.exists():
            return
        try:
            payload = json.loads(result_path.read_text(encoding="utf-8"))
        except Exception:
            return
        # Keep metadata only; Harbor already tracks reward separately.
        if isinstance(payload, dict):
            context.metadata = {
                **(context.metadata or {}),
                "a3s_code_result": payload,
            }

    @with_prompt_template
    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        if not self.model_name:
            raise ValueError("A3sCodeAgent requires --model provider/model")

        access = self.model_connection
        provider = (access.provider or self.model_name.split("/", 1)[0]).lower()
        api_key_env = {
            "anthropic": "ANTHROPIC_API_KEY",
            "openai": "OPENAI_API_KEY",
            "google": "GOOGLE_API_KEY",
            "gemini": "GEMINI_API_KEY",
            "deepseek": "DEEPSEEK_API_KEY",
            "openrouter": "OPENROUTER_API_KEY",
        }.get(provider, f"{provider.upper().replace('-', '_')}_API_KEY")

        if not access.api_key:
            raise ValueError(
                f"No API key found for provider {provider!r}. "
                f"Export {api_key_env} in the Harbor host environment "
                f"(or pass --ae {api_key_env}=...). "
                "Install-only succeeds without a key; full TB trials require one."
            )

        base_url = access.base_url or ""
        env = dict(access.env or {})
        escaped_instruction = shlex.quote(instruction)
        model = shlex.quote(self.model_name)
        api_key_env_q = shlex.quote(api_key_env)
        base_url_q = shlex.quote(base_url)
        command = (
            "set -euo pipefail; "
            'if [ -f "$HOME/.local/bin/env" ]; then . "$HOME/.local/bin/env"; fi; '
            'export PATH="$HOME/.local/bin:$PATH"; '
            f'PY="{_CONTAINER_HOME}/venv/bin/python"; '
            f"printf '%s' {escaped_instruction} > {_CONTAINER_INSTRUCTION}; "
            "\"$PY\" "
            f"{_CONTAINER_RUNNER} "
            "--workspace /app "
            f"--instruction-file {_CONTAINER_INSTRUCTION} "
            f"--model {model} "
            f"--acl-path {_CONTAINER_ACL} "
            f"--log-dir {EnvironmentPaths.agent_dir} "
            f"--api-key-env {api_key_env_q} "
            f"--base-url {base_url_q} "
            f"2>&1 | stdbuf -oL tee {EnvironmentPaths.agent_dir}/a3s-code.txt"
        )

        await self.exec_as_agent(environment, command=command, env=env)
